//! Capture session management: buffered `stream.bin` writing with an
//! explicit finalization contract.
//!
//! `stream.bin` only ever contains chunk bytes in arrival order. The file is
//! considered complete only after [`stop`](CaptureWriter::stop) returns
//! `Ok`; any write failure moves the session into an error state that is
//! surfaced to the UI.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::metadata::{CaptureMetadata, capture_dir_name};

/// Capture lifecycle state exposed to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureStatus {
    /// No capture running.
    Idle,
    /// Capturing; `stream.bin` is being appended in arrival order.
    Active,
    /// The capture ended with an error. `stream.bin` may be incomplete and
    /// `metadata.json` is never marked complete.
    Error,
}

impl CaptureStatus {
    fn as_u8(self) -> u8 {
        match self {
            CaptureStatus::Idle => 0,
            CaptureStatus::Active => 1,
            CaptureStatus::Error => 2,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => CaptureStatus::Active,
            2 => CaptureStatus::Error,
            _ => CaptureStatus::Idle,
        }
    }
}

/// One open capture session.
struct CaptureSession {
    dir: PathBuf,
    file: BufWriter<File>,
    started_at: SystemTime,
    bytes_written: u64,
}

/// Shared capture state. Owned by the stream pipeline; driven by Tauri
/// commands and fed chunks by the processor thread.
pub struct CaptureWriter {
    status: std::sync::atomic::AtomicU8,
    session: std::sync::Mutex<Option<CaptureSession>>,
    error: std::sync::Mutex<Option<String>>,
}

impl CaptureWriter {
    pub fn new() -> Self {
        Self {
            status: std::sync::atomic::AtomicU8::new(CaptureStatus::Idle.as_u8()),
            session: std::sync::Mutex::new(None),
            error: std::sync::Mutex::new(None),
        }
    }

    pub fn status(&self) -> CaptureStatus {
        CaptureStatus::from_u8(self.status.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn bytes_written(&self) -> u64 {
        self.session
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.bytes_written))
            .unwrap_or(0)
    }

    pub fn dir(&self) -> Option<PathBuf> {
        self.session
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.dir.clone()))
    }

    pub fn last_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|g| g.clone())
    }

    /// Starts a capture in `captures_root/<timestamp>/`.
    ///
    /// Creates `stream.bin` and an initial `metadata.json`; both are replaced
    /// atomically on finalization.
    pub fn start(&self, captures_root: &Path) -> Result<PathBuf, String> {
        if self.status() == CaptureStatus::Active {
            return Err("a capture is already active".to_string());
        }
        *self.error.lock().unwrap() = None;

        let now = SystemTime::now();
        let dir = captures_root.join(capture_dir_name(now));
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create capture dir {}: {e}", dir.display()))?;

        let file = File::create(dir.join("stream.bin"))
            .map_err(|e| format!("cannot create stream.bin: {e}"))?;

        let session = CaptureSession {
            dir: dir.clone(),
            file: BufWriter::with_capacity(256 * 1024, file),
            started_at: now,
            bytes_written: 0,
        };
        *self.session.lock().unwrap() = Some(session);
        self.status
            .store(CaptureStatus::Active.as_u8(), std::sync::atomic::Ordering::Relaxed);

        // Initial metadata; rewritten on stop with final counters.
        let meta = CaptureMetadata::started(now);
        meta.write_to(&dir)
            .map_err(|e| format!("cannot write metadata.json: {e}"))?;

        Ok(dir)
    }

    /// Appends one chunk to `stream.bin`. Called by the processor for every
    /// chunk exactly once, in arrival order, regardless of whether a capture
    /// is active (no-op when idle).
    pub fn feed(&self, data: &[u8]) {
        let mut guard = match self.session.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(session) = guard.as_mut() else {
            return;
        };
        if let Err(e) = session.file.write_all(data) {
            let message = format!("stream.bin write failed: {e}");
            drop(guard);
            self.fail(&message);
        } else {
            session.bytes_written += data.len() as u64;
        }
    }

    /// Stops the capture: flushes, writes final metadata with
    /// `bytesReceived` and `complete: true`, and closes the file.
    pub fn stop(&self) -> Result<(PathBuf, u64), String> {
        let mut guard = self.session.lock().unwrap();
        let Some(mut session) = guard.take() else {
            return Err("no capture is active".to_string());
        };

        let result = (|| -> io::Result<(PathBuf, u64)> {
            session.file.flush()?;
            let mut meta = CaptureMetadata::started(session.started_at);
            meta.ended_at = Some(super::metadata::format_timestamp_rfc3339(SystemTime::now()));
            meta.bytes_received = session.bytes_written;
            meta.complete = Some(true);
            meta.write_to(&session.dir)?;
            Ok((session.dir.clone(), session.bytes_written))
        })();

        match result {
            Ok(summary) => {
                self.status
                    .store(CaptureStatus::Idle.as_u8(), std::sync::atomic::Ordering::Relaxed);
                Ok(summary)
            }
            Err(e) => {
                drop(guard);
                self.fail(&format!("capture finalization failed: {e}"));
                Err(format!("capture finalization failed: {e}"))
            }
        }
    }

    fn fail(&self, message: &str) {
        *self.error.lock().unwrap() = Some(message.to_string());
        self.status
            .store(CaptureStatus::Error.as_u8(), std::sync::atomic::Ordering::Relaxed);
        // Drop the session so later feeds are no-ops; stream.bin stays as-is
        // and metadata.json is never marked complete.
        drop(self.session.lock().unwrap().take());
    }
}

impl Default for CaptureWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let unique = format!(
            "trng-capture-test-{}-{}",
            tag,
            std::process::id()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn capture_preserves_exact_byte_order_and_content() {
        // Known-answer integrity test: the pattern fed through feed() must
        // appear in stream.bin byte-for-byte, in order, with nothing added.
        let root = temp_root("known-answer");
        let writer = CaptureWriter::new();
        writer.start(&root).unwrap();

        let mut expected: Vec<u8> = Vec::new();
        for round in 0..100u32 {
            let mut chunk = Vec::new();
            for v in 0..=255u8 {
                chunk.push(v.wrapping_add(round as u8));
            }
            writer.feed(&chunk);
            expected.extend_from_slice(&chunk);
        }

        let (dir, bytes) = writer.stop().expect("stop must succeed");
        assert_eq!(bytes, expected.len() as u64);
        assert_eq!(writer.status(), CaptureStatus::Idle);

        let captured = std::fs::read(dir.join("stream.bin")).unwrap();
        assert_eq!(captured, expected, "stream.bin must be byte-identical");

        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("metadata.json")).unwrap()).unwrap();
        assert_eq!(meta["bytesReceived"], expected.len() as u64);
        assert_eq!(meta["complete"], true);
        assert!(meta["endedAt"].is_string());

        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn feed_while_idle_is_a_no_op() {
        let writer = CaptureWriter::new();
        writer.feed(&[1, 2, 3]);
        assert_eq!(writer.status(), CaptureStatus::Idle);
        assert_eq!(writer.bytes_written(), 0);
        assert!(writer.stop().is_err());
    }

    #[test]
    fn double_start_is_rejected() {
        let root = temp_root("double-start");
        let writer = CaptureWriter::new();
        writer.start(&root).unwrap();
        assert!(writer.start(&root).is_err());
        writer.stop().unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn partial_chunks_and_empty_chunks_are_handled() {
        let root = temp_root("partial");
        let writer = CaptureWriter::new();
        writer.start(&root).unwrap();
        writer.feed(&[]);
        writer.feed(&[0x00, 0xFF, 0x01, 0x01, 0x20]);
        writer.feed(&[]);
        let (dir, bytes) = writer.stop().unwrap();
        assert_eq!(bytes, 5);
        let captured = std::fs::read(dir.join("stream.bin")).unwrap();
        assert_eq!(captured, vec![0x00, 0xFF, 0x01, 0x01, 0x20]);
        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&dir).ok();
    }
}
