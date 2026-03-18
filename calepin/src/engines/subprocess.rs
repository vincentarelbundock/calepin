// Persistent subprocess communication layer for R and Python engines.
//
// ## Sentinel protocol
//
// Each request/response pair is framed by a unique sentinel string generated from
// PID + nanosecond timestamp (see make_sentinel() in mod.rs). This avoids any
// possibility of collision with user output.
//
// Request: Rust writes `{sentinel}_BEGIN\n{payload}\n{sentinel}_END\n` to stdin.
// Response: the subprocess writes tagged output lines, then `{sentinel}_DONE\n`.
// execute() reads lines until it sees the DONE marker, then returns the raw text
// for process_results() in mod.rs to parse into ChunkResult variants.
//
// stderr is inherited (not piped), so library warnings from R/Python appear
// directly in the terminal — useful for a CLI tool.
//
// Known limitation: if a chunk hangs (infinite loop), execute() blocks forever.
// A read timeout would fix this but is not yet implemented.
//
// ## Functions
//
// - SubprocessSession::spawn()   — Start a subprocess with piped stdin/stdout.
// - SubprocessSession::execute() — Send a sentinel-delimited code payload and read back
//                                  the sentinel-delimited result.
// - Drop                         — Close stdin and wait for the subprocess to exit.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Command, Stdio};

/// A persistent subprocess that communicates via stdin/stdout.
/// Used by both R and Python engines.
pub struct SubprocessSession {
    child: Child,
    stdin: Option<BufWriter<std::process::ChildStdin>>,
    stdout: BufReader<std::process::ChildStdout>,
}

impl SubprocessSession {
    /// Spawn a subprocess with the given command and arguments.
    /// stdin/stdout are piped; stderr is inherited (warnings go to terminal).
    pub fn spawn(program: &str, args: &[&str]) -> Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to spawn {}. Is it installed and on PATH?", program))?;

        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let stdout = BufReader::new(child.stdout.take().unwrap());

        Ok(SubprocessSession {
            child,
            stdin: Some(stdin),
            stdout,
        })
    }

    /// Send code to the subprocess and read back the sentinel-delimited result.
    /// Returns the raw output in the same format as the old embedded capture:
    /// first line is the sentinel, followed by sentinel-tagged sections.
    pub fn execute(&mut self, sentinel: &str, payload: &str) -> Result<String> {
        let stdin = self.stdin.as_mut().context("Subprocess stdin closed")?;

        // Send: {sentinel}_BEGIN\n{payload}\n{sentinel}_END\n
        write!(stdin, "{}_BEGIN\n{}\n{}_END\n", sentinel, payload, sentinel)
            .context("Failed to send code to subprocess")?;
        stdin.flush().context("Failed to flush stdin")?;

        // Read lines until {sentinel}_DONE
        let done_marker = format!("{}_DONE", sentinel);
        let mut output = String::new();

        loop {
            let mut line = String::new();
            let bytes = self
                .stdout
                .read_line(&mut line)
                .context("Failed to read from subprocess (process may have crashed)")?;
            if bytes == 0 {
                anyhow::bail!("Subprocess exited unexpectedly");
            }
            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
            if trimmed == done_marker {
                break;
            }
            output.push_str(&line);
        }

        // Remove trailing newline if present
        if output.ends_with('\n') {
            output.pop();
        }

        Ok(format!("{}\n{}", sentinel, output))
    }
}

impl Drop for SubprocessSession {
    fn drop(&mut self) {
        // Drop the BufWriter to close the stdin pipe, signaling EOF to the subprocess
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}
