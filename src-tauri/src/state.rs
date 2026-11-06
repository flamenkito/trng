//! Shared application state between the stream pipeline, IPC, and lifecycle.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use serde::Serialize;

use crate::capture::writer::CaptureWriter;
use crate::stream::counters::StreamCounters;

/// Device/stream state exposed to the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceStatus {
    Disconnected,
    Connecting,
    Connected,
    Streaming,
    Error,
}

impl DeviceStatus {
    fn as_u8(self) -> u8 {
        match self {
            DeviceStatus::Disconnected => 0,
            DeviceStatus::Connecting => 1,
            DeviceStatus::Connected => 2,
            DeviceStatus::Streaming => 3,
            DeviceStatus::Error => 4,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => DeviceStatus::Connecting,
            2 => DeviceStatus::Connected,
            3 => DeviceStatus::Streaming,
            4 => DeviceStatus::Error,
            _ => DeviceStatus::Disconnected,
        }
    }
}

/// Process-wide stream state. Written by the pipeline threads, read by the
/// status ticker that emits frontend snapshots.
pub struct StreamState {
    status: AtomicU8,
    pub counters: StreamCounters,
    pub capture: CaptureWriter,
    reconnect_count: AtomicU64,
    source: Mutex<String>,
    last_error: Mutex<Option<String>>,
}

impl StreamState {
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(DeviceStatus::Disconnected.as_u8()),
            counters: StreamCounters::default(),
            capture: CaptureWriter::new(),
            reconnect_count: AtomicU64::new(0),
            source: Mutex::new("none".to_string()),
            last_error: Mutex::new(None),
        }
    }

    pub fn status(&self) -> DeviceStatus {
        DeviceStatus::from_u8(self.status.load(Ordering::Relaxed))
    }

    pub fn set_status(&self, status: DeviceStatus) {
        self.status.store(status.as_u8(), Ordering::Relaxed);
    }

    pub fn reconnect_count(&self) -> u64 {
        self.reconnect_count.load(Ordering::Relaxed)
    }

    pub fn record_reconnect(&self) {
        self.reconnect_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_source(&self, source: impl Into<String>) {
        if let Ok(mut guard) = self.source.lock() {
            *guard = source.into();
        }
    }

    pub fn source(&self) -> String {
        self.source.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn set_error(&self, message: impl Into<String>) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(message.into());
        }
    }

    pub fn clear_error(&self) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|g| g.clone())
    }
}

impl Default for StreamState {
    fn default() -> Self {
        Self::new()
    }
}
