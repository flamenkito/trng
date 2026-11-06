//! Bucketed rolling histogram over the byte stream.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::Serialize;

/// Aggregation bucket duration.
pub const BUCKET_DURATION: Duration = Duration::from_millis(100);

/// Configured rolling windows, shortest first. New windows can be added here.
pub const WINDOW_DURATIONS_MS: [u64; 5] = [1_000, 5_000, 30_000, 60_000, 300_000];

/// Keep a little headroom beyond the longest window so a window query at
/// time T can see every bucket that starts within [T-5m, T].
const MAX_BUCKETS: usize = (WINDOW_DURATIONS_MS[4] / 100) as usize + 2;

/// One timestamped aggregation bucket.
#[derive(Clone)]
pub struct Bucket {
    pub start: Instant,
    pub counts: [u32; 256],
}

impl Bucket {
    fn new(start: Instant) -> Self {
        Self {
            start,
            counts: [0; 256],
        }
    }

    fn total(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }
}

/// Aggregated counts for one rolling window.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowStats {
    pub window_ms: u64,
    /// Total bytes contributing to `counts`.
    pub samples: u64,
    /// Exact per-value counts over the covered (bucket-aligned) span.
    pub counts: Vec<u64>,
}

/// Rolling histogram: completed buckets plus the bucket currently being
/// filled. Owned by the processor thread only (single consumer); snapshots
/// are copied out for the ticker under a short mutex.
pub struct RollingHistogram {
    buckets: VecDeque<Bucket>,
    current: Bucket,
}

impl RollingHistogram {
    pub fn new(now: Instant) -> Self {
        Self {
            buckets: VecDeque::with_capacity(MAX_BUCKETS),
            current: Bucket::new(now),
        }
    }

    /// Records bytes into the current bucket, rotating buckets as wall time
    /// crosses bucket boundaries. Called by the processor for every chunk
    /// exactly once.
    pub fn record(&mut self, now: Instant, data: &[u8]) {
        if now.duration_since(self.current.start) >= BUCKET_DURATION {
            self.rotate(now);
        }
        for &b in data {
            // Safe: byte value indexes the 256-bin table directly.
            self.current.counts[b as usize] += 1;
        }
    }

    fn rotate(&mut self, now: Instant) {
        let finished = std::mem::replace(&mut self.current, Bucket::new(now));
        if finished.total() > 0 {
            self.buckets.push_back(finished);
        }
        while self.buckets.len() > MAX_BUCKETS {
            self.buckets.pop_front();
        }
    }

    /// Reconstructs exact counts for the window: buckets with start in
    /// `(now - W, now]`, plus the in-progress bucket.
    pub fn window(&self, now: Instant, window_ms: u64) -> WindowStats {
        let cutoff = now - Duration::from_millis(window_ms);
        let mut counts = [0u64; 256];
        let mut samples = 0u64;

        let mut accumulate = |bucket: &Bucket| {
            for (acc, &c) in counts.iter_mut().zip(bucket.counts.iter()) {
                *acc += c as u64;
            }
            samples += bucket.total();
        };

        for bucket in &self.buckets {
            if bucket.start > cutoff {
                accumulate(bucket);
            }
        }
        if self.current.start > cutoff {
            accumulate(&self.current);
        }

        WindowStats {
            window_ms,
            samples,
            counts: counts.to_vec(),
        }
    }

    /// Snapshots all configured windows.
    pub fn all_windows(&self, now: Instant) -> Vec<WindowStats> {
        WINDOW_DURATIONS_MS
            .iter()
            .map(|&ms| self.window(now, ms))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advance(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn every_byte_increments_exactly_one_bin() {
        let t0 = Instant::now();
        let mut h = RollingHistogram::new(t0);
        let data: Vec<u8> = (0..=255u16).map(|v| v as u8).collect();
        h.record(t0, &data);
        h.record(t0, &data);
        let stats = h.window(t0, 1000);
        assert_eq!(stats.samples, 512);
        for (i, &c) in stats.counts.iter().enumerate() {
            assert_eq!(c, 2, "bin {i} must contain exactly 2 occurrences");
        }
    }

    #[test]
    fn window_evicts_old_buckets() {
        let t0 = Instant::now();
        let mut h = RollingHistogram::new(t0);
        // Fill bucket [0,100) with byte 0x01 only.
        h.record(t0, &[0x01; 100]);
        // 1 s later: bucket [1000,1100) with byte 0x02 only.
        let t1 = advance(t0, 1000);
        h.record(t1, &[0x02; 100]);

        let wide = h.window(t1, 60_000);
        assert_eq!(wide.samples, 200);
        assert_eq!(wide.counts[0x01], 100);
        assert_eq!(wide.counts[0x02], 100);

        // A 1 s window queried at t1 covers bucket starts in (t0, t1]: the
        // 0x01 bucket (started exactly at t0 = cutoff) is excluded; the 0x02
        // bucket (started at t1, now the in-progress bucket) is included.
        let one_sec = h.window(t1, 1000);
        assert_eq!(one_sec.counts[0x01], 0, "bucket starting exactly at cutoff is excluded");
        assert_eq!(one_sec.counts[0x02], 100);
    }

    #[test]
    fn chunk_boundaries_do_not_affect_counts() {
        let t0 = Instant::now();
        let mut whole = RollingHistogram::new(t0);
        let mut split = RollingHistogram::new(t0);
        let data: Vec<u8> = (0..=255).cycle().take(4096).map(|v| v as u8).collect();
        whole.record(t0, &data);
        for chunk in data.chunks(7) {
            split.record(t0, chunk);
        }
        let a = whole.window(t0, 1000);
        let b = split.window(t0, 1000);
        assert_eq!(a.samples, b.samples);
        assert_eq!(a.counts, b.counts);
    }

    #[test]
    fn empty_stream_produces_zeroed_counts() {
        let t0 = Instant::now();
        let h = RollingHistogram::new(t0);
        let stats = h.window(t0, 5000);
        assert_eq!(stats.samples, 0);
        assert!(stats.counts.iter().all(|&c| c == 0));
    }

    #[test]
    fn all_windows_returns_five_configured_windows() {
        let t0 = Instant::now();
        let h = RollingHistogram::new(t0);
        let windows = h.all_windows(t0);
        assert_eq!(windows.len(), 5);
        assert_eq!(
            windows.iter().map(|w| w.window_ms).collect::<Vec<_>>(),
            vec![1000, 5000, 30000, 60000, 300000]
        );
    }

    #[test]
    fn bucket_rotation_drops_ancient_buckets() {
        let t0 = Instant::now();
        let mut h = RollingHistogram::new(t0);
        // Feed 400 buckets' worth of time (40 s, beyond the 1 s window but
        // within capacity) to ensure rotation does not corrupt state.
        for i in 0..400u64 {
            h.record(advance(t0, i * 100), &[0xAB]);
        }
        let stats = h.window(advance(t0, 399 * 100), 1000);
        // Only the buckets within the last second remain counted.
        assert_eq!(stats.counts[0xAB], stats.samples);
        assert!(stats.samples >= 1);
        assert!(stats.samples < 400);
    }
}
