//! Exact byte-stream counters and offset-integrity verification.
//!
//! Invariant: every chunk delivered to the processor is counted exactly once.
//! Any detected gap or overlap in stream offsets is an application-level
//! integrity error and must be surfaced — never silently continue.

use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonically increasing stream statistics.
#[derive(Default)]
pub struct StreamCounters {
    /// Total bytes received from the source. Never decreases.
    total_bytes_received: AtomicU64,
    /// Bytes added since the last status tick (swapped by the ticker to
    /// derive an exact bytes/sec rate — never a sampled estimate).
    bytes_since_tick: AtomicU64,
    /// Number of chunks processed.
    chunks_processed: AtomicU64,
    /// Bytes the application itself failed to process. Must always be 0; any
    /// non-zero value indicates an internal pipeline overflow and is shown
    /// prominently as APPLICATION-DROPPED.
    application_dropped_bytes: AtomicU64,
}

impl StreamCounters {
    /// Records one processed chunk of `len` bytes. Called exactly once per
    /// chunk by the master stream processor.
    pub fn record_chunk(&self, len: usize) {
        let len = len as u64;
        self.total_bytes_received.fetch_add(len, Ordering::Relaxed);
        self.bytes_since_tick.fetch_add(len, Ordering::Relaxed);
        self.chunks_processed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn total_bytes_received(&self) -> u64 {
        self.total_bytes_received.load(Ordering::Relaxed)
    }

    pub fn chunks_processed(&self) -> u64 {
        self.chunks_processed.load(Ordering::Relaxed)
    }

    pub fn application_dropped_bytes(&self) -> u64 {
        self.application_dropped_bytes.load(Ordering::Relaxed)
    }

    /// Records an explicit application-level drop. Per project Rule 3 this
    /// should never happen; if it does it becomes a visible error state.
    pub fn record_application_drop(&self, len: usize) {
        self.application_dropped_bytes
            .fetch_add(len as u64, Ordering::Relaxed);
    }

    /// Swaps out the bytes accumulated since the previous tick.
    pub fn swap_bytes_since_tick(&self) -> u64 {
        self.bytes_since_tick.swap(0, Ordering::Relaxed)
    }
}

/// Verifies that consecutive chunks cover the stream without gaps or
/// overlaps. Offsets are logical positions in the continuous stream,
/// independent of chunk boundaries.
#[derive(Default)]
pub struct OffsetVerifier {
    next_expected: u64,
    seen_first: bool,
}

impl OffsetVerifier {
    /// Checks `chunk` for continuity. Returns the number of bytes that were
    /// out of order (0 = perfectly contiguous). The verifier resynchronizes
    /// to the end of the given chunk so one reported gap is not repeated.
    pub fn check(&mut self, offset: u64, len: u64) -> u64 {
        if !self.seen_first {
            self.seen_first = true;
            self.next_expected = offset + len;
            return 0;
        }
        let lost = offset.abs_diff(self.next_expected);
        self.next_expected = offset + len;
        lost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_is_counted_exactly_once() {
        let counters = StreamCounters::default();
        counters.record_chunk(4096);
        counters.record_chunk(17);
        counters.record_chunk(1);
        assert_eq!(counters.total_bytes_received(), 4114);
        assert_eq!(counters.chunks_processed(), 3);
        assert_eq!(counters.application_dropped_bytes(), 0);
    }

    #[test]
    fn bytes_since_tick_is_swapped_exactly_once() {
        let counters = StreamCounters::default();
        counters.record_chunk(100);
        counters.record_chunk(50);
        assert_eq!(counters.swap_bytes_since_tick(), 150);
        assert_eq!(counters.swap_bytes_since_tick(), 0);
        assert_eq!(counters.total_bytes_received(), 150);
    }

    #[test]
    fn chunk_boundaries_do_not_affect_totals() {
        let a = StreamCounters::default();
        a.record_chunk(10_000);
        let b = StreamCounters::default();
        for _ in 0..10_000 {
            b.record_chunk(1);
        }
        assert_eq!(a.total_bytes_received(), b.total_bytes_received());
    }

    #[test]
    fn offset_verifier_accepts_contiguous_chunks() {
        let mut v = OffsetVerifier::default();
        assert_eq!(v.check(0, 4096), 0);
        assert_eq!(v.check(4096, 4096), 0);
        assert_eq!(v.check(8192, 17), 0);
    }

    #[test]
    fn offset_verifier_detects_gap_and_resyncs() {
        let mut v = OffsetVerifier::default();
        assert_eq!(v.check(0, 4096), 0);
        // Next chunk claims to start 100 bytes later than expected.
        assert_eq!(v.check(4196, 4096), 100);
        // Resynced: a contiguous chunk after the gap reports no further loss.
        assert_eq!(v.check(8292, 4096), 0);
    }

    #[test]
    fn offset_verifier_detects_overlap() {
        let mut v = OffsetVerifier::default();
        assert_eq!(v.check(0, 4096), 0);
        // Re-delivering part of the same range is a (negative) gap.
        assert_eq!(v.check(4000, 4096), 96);
    }
}
