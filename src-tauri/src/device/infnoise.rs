//! `infnoise` subprocess source (WHITENED mode, binary stdout).
//!
//! The official `infnoise` executable is the device interface. Its stdout is
//! one continuous byte stream; chunk boundaries are an artifact of pipe
//! buffering and carry no meaning.

use std::io::{self, Read};
use std::process::{Child, ChildStdout, Command, Stdio};

use super::ByteSource;

pub const SOURCE_NAME: &str = "infnoise (whitened)";

pub const RAW_SOURCE_NAME: &str = "infnoise (raw source)";

/// Reads the Infinite Noise TRNG through the `infnoise` executable.
pub struct InfnoiseSource {
    child: Child,
    stdout: ChildStdout,
    name: &'static str,
}

impl InfnoiseSource {
    /// Spawns `infnoise` with binary whitened output on stdout.
    pub fn spawn() -> Result<Self, String> {
        Self::spawn_inner(false)
    }

    /// Spawns `infnoise --raw`: the unwhitened noise source. The raw source
    /// is NOT expected to be uniformly distributed — it is a separate
    /// analysis mode, not a drop-in replacement for whitened output.
    pub fn spawn_raw() -> Result<Self, String> {
        Self::spawn_inner(true)
    }

    fn spawn_inner(raw: bool) -> Result<Self, String> {
        let mut child = Command::new("infnoise")
            .args(if raw { vec!["--raw"] } else { vec![] })
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start `infnoise` ({e}) — is it installed and on PATH?"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to capture infnoise stdout".to_string())?;

        Ok(Self {
            child,
            stdout,
            name: if raw { RAW_SOURCE_NAME } else { SOURCE_NAME },
        })
    }
}

impl ByteSource for InfnoiseSource {
    fn read_chunk(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stdout.read(buf)
    }

    fn describe(&self) -> &str {
        self.name
    }
}

impl Drop for InfnoiseSource {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
