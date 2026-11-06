//! Replay source: reads a previously captured `stream.bin` back through the
//! same stream pipeline (Rule 8 — one statistics engine for live and replay).
//!
//! Reads are paced (~64 KB/s) so rolling windows behave like a live capture
//! instead of being flooded instantaneously. EOF returns 0, which the reader
//! treats as a finished stream rather than an error.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;
use std::time::Duration;

use super::ByteSource;

/// Pause between chunk reads; keeps replay near live-acquisition pacing.
const REPLAY_PACE: Duration = Duration::from_millis(25);

/// Chunk size per paced read.
const REPLAY_CHUNK: usize = 16 * 1024;

pub struct FileReplaySource {
    reader: BufReader<File>,
    describe: String,
}

impl FileReplaySource {
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = File::open(path)
            .map_err(|e| format!("cannot open replay file {}: {e}", path.display()))?;
        let describe = format!("replay: {}", path.display());
        Ok(Self {
            reader: BufReader::with_capacity(REPLAY_CHUNK, file),
            describe,
        })
    }
}

impl ByteSource for FileReplaySource {
    fn read_chunk(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        std::thread::sleep(REPLAY_PACE);
        let n = buf.len().min(REPLAY_CHUNK);
        self.reader.read(&mut buf[..n])
    }

    fn describe(&self) -> &str {
        &self.describe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replays_exact_bytes_in_order_until_eof() {
        let dir = std::env::temp_dir().join(format!("trng-replay-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stream.bin");

        // Known content: two cycles of 00..FF plus a partial tail.
        let mut expected: Vec<u8> = (0..=255u16).map(|v| v as u8).collect();
        expected.extend((0..=255u16).map(|v| v as u8));
        expected.extend([0xDE, 0xAD, 0xBE, 0xEF]);
        std::fs::write(&path, &expected).unwrap();

        let mut source = FileReplaySource::open(&path).unwrap();
        let mut collected: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; 4096];
        loop {
            match source.read_chunk(&mut buf) {
                Ok(0) => break,
                Ok(n) => collected.extend_from_slice(&buf[..n]),
                Err(e) => panic!("replay read failed: {e}"),
            }
        }

        assert_eq!(collected, expected, "replay must be byte-identical to the capture");
        std::fs::remove_dir_all(&dir).ok();
    }
}
