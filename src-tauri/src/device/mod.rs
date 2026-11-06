//! Device input layer.
//!
//! A [`ByteSource`] is one authoritative consumer of a random byte stream.
//! The stream pipeline (see `stream::processor`) is the only component that
//! ever reads from a source; everything downstream is fed from the internal
//! bounded channel.

pub mod infnoise;
pub mod replay;
pub mod test_pattern;

use std::io;
use std::path::PathBuf;

/// Which byte source the reader should open. Switchable at runtime via the
/// `set_source` Tauri command so LIVE and REPLAY feed the same pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSpec {
    /// The physical TRNG via the `infnoise` subprocess (WHITENED).
    Infnoise,
    /// The unwhitened noise source (`infnoise --raw`). Experimental: the raw
    /// source is not uniformly distributed and must not be read as one.
    InfnoiseRaw,
    /// Deterministic `00..FF` loop for development/tests.
    TestPattern,
    /// Replay a previously captured `stream.bin` through the same
    /// statistics pipeline.
    Replay(PathBuf),
}

impl SourceSpec {
    /// Opens a source for this spec.
    pub fn open(&self) -> Result<Box<dyn ByteSource>, String> {
        match self {
            SourceSpec::Infnoise => Ok(Box::new(infnoise::InfnoiseSource::spawn()?)),
            SourceSpec::InfnoiseRaw => Ok(Box::new(infnoise::InfnoiseSource::spawn_raw()?)),
            SourceSpec::TestPattern => Ok(Box::new(test_pattern::TestPatternSource::new())),
            SourceSpec::Replay(path) => Ok(Box::new(replay::FileReplaySource::open(path)?)),
        }
    }

    /// Conceptual data mode recorded in capture metadata.
    pub fn mode_label(&self) -> &'static str {
        match self {
            SourceSpec::Infnoise => "whitened",
            SourceSpec::InfnoiseRaw => "raw",
            SourceSpec::TestPattern => "testpattern",
            SourceSpec::Replay(_) => "replay",
        }
    }
}

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
