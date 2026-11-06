//! `infnoise` subprocess source (WHITENED mode, binary stdout).
//!
//! The official `infnoise` executable is the device interface. Its stdout is
//! one continuous byte stream; chunk boundaries are an artifact of pipe
//! buffering and carry no meaning.

use std::io::{self, Read};
use std::process::{Child, ChildStdout, Command, Stdio};

use super::ByteSource;

pub const SOURCE_NAME: &str = "infnoise (whitened)";

/// Reads the Infinite Noise TRNG through the `infnoise` executable.
pub struct InfnoiseSource {
    child: Child,
    stdout: ChildStdout,
}

impl InfnoiseSource {
    /// Spawns `infnoise` with binary whitened output on stdout.
    pub fn spawn() -> Result<Self, String> {
        let mut child = Command::new("infnoise")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start `infnoise` ({e}) — is it installed and on PATH?"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to capture infnoise stdout".to_string())?;

        Ok(Self { child, stdout })
    }
}

impl ByteSource for InfnoiseSource {
    fn read_chunk(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stdout.read(buf)
    }

    fn describe(&self) -> &str {
        SOURCE_NAME
    }
}

impl Drop for InfnoiseSource {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
