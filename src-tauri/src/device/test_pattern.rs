//! Deterministic test source: `00 01 02 ... FE FF` repeating forever.
//!
//! Used for development and integrity tests when the physical TRNG is
//! unavailable. Enable with the `TRNG_SOURCE=testpattern` environment
//! variable. Feeds the exact same pipeline as the real device.

use std::io;

use super::ByteSource;

pub const SOURCE_NAME: &str = "testpattern (00..FF loop)";

/// Infinite repeating sequence 0x00..=0xFF with varied chunk sizes so tests
/// exercise partial reads and chunk-boundary handling.
pub struct TestPatternSource {
    next_value: u8,
    reads: u64,
}

impl TestPatternSource {
    pub fn new() -> Self {
        Self {
            next_value: 0,
            reads: 0,
        }
    }
}

impl ByteSource for TestPatternSource {
    fn read_chunk(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Vary delivered length: cycles through 17..=64 byte chunks.
        let want = 17 + (self.reads % 48) as usize;
        let n = buf.len().min(want.max(1));
        for slot in buf.iter_mut().take(n) {
            *slot = self.next_value;
            self.next_value = self.next_value.wrapping_add(1);
        }
        self.reads += 1;
        // Simulate a device that never ends.
        std::thread::sleep(std::time::Duration::from_millis(2));
        Ok(n)
    }

    fn describe(&self) -> &str {
        SOURCE_NAME
    }
}
