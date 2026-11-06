//! Device input layer.
//!
//! A [`ByteSource`] is one authoritative consumer of a random byte stream.
//! The stream pipeline (see `stream::processor`) is the only component that
//! ever reads from a source; everything downstream is fed from the internal
//! bounded channel.

pub mod infnoise;
pub mod test_pattern;

use std::io;

/// One continuous byte stream.
///
/// Implementations must treat reads as arbitrary chunk boundaries of a single
/// logical stream: one `read_chunk` call is never one "unit" of random data.
pub trait ByteSource: Send {
    /// Reads up to `buf.len()` bytes from the stream.
    ///
    /// Returns the number of bytes read. Returns `0` when the stream has ended
    /// (subprocess exited, finite test pattern consumed, device unplugged).
    fn read_chunk(&mut self, buf: &mut [u8]) -> io::Result<usize>;

    /// Short human-readable description of the source, shown in the UI.
    fn describe(&self) -> &str;
}
