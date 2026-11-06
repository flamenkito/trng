//! Master stream pipeline: the single authoritative consumer of the device
//! stream.
//!
//! ```text
//! ByteSource (reader thread, blocking reads)
//!     │  bounded channel (StreamChunk { offset, data })
//!     ▼
//! Processor thread  ── every chunk exactly once ──▶ counters (+ stats/capture later)
//!     │
//!     ▼
//! Status ticker (10 Hz) ──▶ `stream-status` Tauri event ──▶ Angular snapshot
//! ```
//!
//! Backpressure policy (Rule 3): the reader thread *blocks* on the bounded
//! channel instead of dropping bytes. With the current consumer (counters
//! only) this never engages; capture/statistics consumers added later must
//! keep the channel drained or surface a critical buffer state.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::mpsc::sync_channel;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::device::ByteSource;
use crate::device::infnoise::InfnoiseSource;
use crate::device::test_pattern::TestPatternSource;
use crate::ipc::events::{StreamStatusEvent, emit_stream_status};
use crate::state::{DeviceStatus, StreamState};
use crate::stream::counters::OffsetVerifier;

/// Logical chunk of the continuous stream.
struct StreamChunk {
    /// Offset of `data[0]` in the continuous stream.
    offset: u64,
    data: Vec<u8>,
}

/// Bounded capacity, in chunks, of the reader → processor channel.
const CHANNEL_CAPACITY: usize = 4096;

const READ_BUFFER_SIZE: usize = 64 * 1024;
const RETRY_DELAY: Duration = Duration::from_secs(2);
const TICK_INTERVAL: Duration = Duration::from_millis(100);

/// Handle to the running pipeline; shut down on application exit.
pub struct StreamPipeline {
    shutdown: Arc<AtomicBool>,
    /// Currently open source. `stop()` drops it, which terminates the
    /// subprocess and unblocks a reader thread sitting in `read_chunk`.
    source_slot: Arc<Mutex<Option<Box<dyn ByteSource>>>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl StreamPipeline {
    /// Requests shutdown. The reader stops retrying, the current source is
    /// dropped (killing the subprocess), and the ticker exits.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        // Drop the active source: kills the infnoise child process, which
        // makes a blocked read return and lets the reader thread exit.
        drop(self.source_slot.lock().unwrap().take());
    }

    /// Waits for pipeline threads to finish. Call after [`stop`](Self::stop).
    pub fn join(&self) {
        let mut threads = self.threads.lock().unwrap();
        for thread in threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// Opens the configured byte source.
///
/// `TRNG_SOURCE=testpattern` selects the deterministic test source so the
/// full pipeline can run without the physical TRNG attached.
fn open_source() -> Result<Box<dyn ByteSource>, String> {
    match std::env::var("TRNG_SOURCE").as_deref() {
        Ok("testpattern") => Ok(Box::new(TestPatternSource::new())),
        _ => Ok(Box::new(InfnoiseSource::spawn()?)),
    }
}

/// Starts reader, processor, and status-ticker threads.
pub fn start(app: AppHandle, state: Arc<StreamState>) -> StreamPipeline {
    let shutdown = Arc::new(AtomicBool::new(false));
    let source_slot: Arc<Mutex<Option<Box<dyn ByteSource>>>> = Arc::new(Mutex::new(None));
    let (tx, rx) = sync_channel::<StreamChunk>(CHANNEL_CAPACITY);
    let mut threads = Vec::new();

    // Reader thread: owns the device source, applies backpressure by blocking.
    {
        let state = state.clone();
        let shutdown = shutdown.clone();
        let source_slot = source_slot.clone();
        threads.push(std::thread::Builder::new()
            .name("trng-reader".into())
            .spawn(move || {
                let mut offset: u64 = 0;
                while !shutdown.load(Ordering::Relaxed) {
                    state.set_status(DeviceStatus::Connecting);
                    let source = match open_source() {
                        Ok(source) => {
                            state.set_source(source.describe());
                            state.clear_error();
                            state.set_status(DeviceStatus::Streaming);
                            source
                        }
                        Err(err) => {
                            state.set_status(DeviceStatus::Error);
                            state.set_error(err);
                            state.record_reconnect();
                            std::thread::sleep(RETRY_DELAY);
                            continue;
                        }
                    };
                    *source_slot.lock().unwrap() = Some(source);

                    let mut buf = vec![0u8; READ_BUFFER_SIZE];
                    loop {
                        if shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                        // Read through the shared slot so `stop()` can drop
                        // the source and unblock this read during shutdown.
                        let read = {
                            let mut guard = source_slot.lock().unwrap();
                            match guard.as_mut() {
                                Some(source) => source
                                    .read_chunk(&mut buf)
                                    .map_err(|e| (e.to_string(), source.describe().to_string())),
                                None => break,
                            }
                        };
                        match read {
                            Ok(0) => {
                                let describe = source_slot
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|s| s.describe().to_string())
                                    .unwrap_or_else(|| "stream".to_string());
                                *source_slot.lock().unwrap() = None;
                                // Subprocess exited or finite test source done.
                                state.set_status(DeviceStatus::Error);
                                state.set_error(format!("stream from {describe} ended unexpectedly"));
                                break;
                            }
                            Ok(n) => {
                                let chunk = StreamChunk {
                                    offset,
                                    data: buf[..n].to_vec(),
                                };
                                offset += n as u64;
                                // Bounded channel: blocks under backpressure
                                // rather than ever dropping a byte.
                                if tx.send(chunk).is_err() {
                                    break;
                                }
                            }
                            Err(err) => {
                                let (read_err, describe) = err;
                                *source_slot.lock().unwrap() = None;
                                state.set_status(DeviceStatus::Error);
                                state.set_error(format!("read failed on {describe}: {read_err}"));
                                break;
                            }
                        }
                    }
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    state.record_reconnect();
                    std::thread::sleep(RETRY_DELAY);
                }
            })
            .expect("spawn trng-reader"));
    }

    // Processor thread: the only place chunks are consumed. Every byte is
    // processed exactly once here.
    {
        let state = state.clone();
        threads.push(std::thread::Builder::new()
            .name("trng-processor".into())
            .spawn(move || {
                let mut verifier = OffsetVerifier::default();
                while let Ok(chunk) = rx.recv() {
                    let len = chunk.data.len() as u64;
                    let lost = verifier.check(chunk.offset, len);
                    if lost > 0 {
                        // Application-level integrity failure: visible error.
                        state.set_error(format!(
                            "stream offset discontinuity: {lost} bytes out of order"
                        ));
                        state.set_status(DeviceStatus::Error);
                        state.counters.record_application_drop(lost as usize);
                    }
                    // The chunk is deliberately kept alive until all future
                    // consumers (statistics, capture) have seen it.
                    state.counters.record_chunk(chunk.data.len());
                }
            })
            .expect("spawn trng-processor"));
    }

    // Status ticker: emits frontend snapshots at 10 Hz. Snapshots are derived
    // state only — they may be skipped by the frontend without affecting the
    // byte pipeline.
    {
        let state = state.clone();
        let shutdown = shutdown.clone();
        threads.push(std::thread::Builder::new()
            .name("trng-ticker".into())
            .spawn(move || {
                let mut last_tick = Instant::now();
                while !shutdown.load(Ordering::Relaxed) {
                    std::thread::sleep(TICK_INTERVAL);
                    let now = Instant::now();
                    let elapsed = now.duration_since(last_tick).as_secs_f64();
                    last_tick = now;
                    let bytes = state.counters.swap_bytes_since_tick();
                    let event = StreamStatusEvent {
                        device_status: state.status(),
                        source: state.source(),
                        total_bytes_received: state.counters.total_bytes_received(),
                        bytes_per_second: bytes as f64 / elapsed,
                        application_dropped_bytes: state.counters.application_dropped_bytes(),
                        chunks_processed: state.counters.chunks_processed(),
                        reconnect_count: state.reconnect_count(),
                        last_error: state.last_error(),
                    };
                    if emit_stream_status(&app, &event).is_err() {
                        // Frontend not ready yet; next tick will carry the state.
                    }
                }
            })
            .expect("spawn trng-ticker"));
    }

    StreamPipeline {
        shutdown,
        source_slot,
        threads: Mutex::new(threads),
    }
}

