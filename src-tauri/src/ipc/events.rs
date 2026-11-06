use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::state::DeviceStatus;

/// Test event payload proving the backend -> frontend event path works.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestEvent {
    pub message: String,
    pub timestamp_ms: u64,
}

pub const TEST_EVENT_NAME: &str = "backend-test-event";

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Emits a single test event shortly after startup.
///
/// Delayed slightly so the frontend has time to subscribe before it arrives.
pub fn emit_test_event(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = app.emit(
            TEST_EVENT_NAME,
            TestEvent {
                message: "Rust -> Angular event channel verified".to_string(),
                timestamp_ms: now_ms(),
            },
        );
    });
}

/// Emits the test event immediately, in response to a frontend request.
///
/// Used so the frontend can guarantee it is subscribed before emission.
pub fn emit_test_event_now(app: &AppHandle) {
    let _ = app.emit(
        TEST_EVENT_NAME,
        TestEvent {
            message: "Rust -> Angular event channel verified".to_string(),
            timestamp_ms: now_ms(),
        },
    );
}

pub const STREAM_STATUS_EVENT: &str = "stream-status";

/// Snapshot of derived stream state, emitted at ~10 Hz.
///
/// This is presentation state only: the frontend may skip frames, but the
/// underlying byte pipeline processes every received byte regardless.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamStatusEvent {
    pub device_status: DeviceStatus,
    pub source: String,
    pub total_bytes_received: u64,
    pub bytes_per_second: f64,
    /// Application-level drops only. This is NOT a claim about USB/FTDI/
    /// driver-level integrity upstream of the application.
    pub application_dropped_bytes: u64,
    pub chunks_processed: u64,
    pub reconnect_count: u64,
    pub last_error: Option<String>,
}

pub fn emit_stream_status(app: &AppHandle, event: &StreamStatusEvent) -> tauri::Result<()> {
    app.emit(STREAM_STATUS_EVENT, event)
}
