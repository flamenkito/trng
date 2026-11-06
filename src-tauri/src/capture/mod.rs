//! Lossless binary capture of the TRNG stream.
//!
//! ```text
//! processor (every chunk exactly once)
//!     │  bounded channel
//!     ▼
//! capture writer thread ──▶ captures/<timestamp>/stream.bin  (byte-for-byte)
//!                              captures/<timestamp>/metadata.json
//! ```
//!
//! Invariants (Rules 4 and 6): `stream.bin` receives the chunk bytes with no
//! transformation, no headers, no text encoding; any write failure becomes a
//! visible error state and the capture is marked incomplete.

pub mod metadata;
pub mod writer;
