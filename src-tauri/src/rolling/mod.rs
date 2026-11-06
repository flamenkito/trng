//! Rolling-window byte distribution statistics.
//!
//! The processor aggregates the stream into small timestamped buckets:
//!
//! ```text
//! bucket duration: 100 ms
//! each bucket: { start: Instant, bytes: u32, counts: [u32; 256] }
//! ```
//!
//! Documented semantics: a window of duration W queried at time T is the sum
//! of all buckets whose start time falls in the half-open interval
//! `(T - W, T]`, plus the bucket currently being filled. Because bucket
//! boundaries are fixed, the covered span can exceed W by up to one bucket
//! duration (~100 ms) at the trailing edge. Every received byte lands in
//! exactly one bucket, so counts are exact for the covered span; only the
//! span edges are bucket-aligned (bucket approximation per spec §11).
//!
//! All 256 bins of every configured window are recomputed on demand from the
//! buckets (not re-sampled), so a 5-minute window contains every byte
//! received during it.

pub mod histogram;
