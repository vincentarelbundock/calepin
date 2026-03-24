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
// directly in the terminal -- useful for a CLI tool.
//
// ## Timeout
//
// execute() uses a reader thread + channel with recv_timeout. If a chunk doesn't
// produce a DONE marker within the timeout, the subprocess is killed and an error
// is returned. Default timeout is 30 seconds; set CALEPIN_TIMEOUT=N to override.
//
// ## Functions
//
// - SubprocessSession::spawn()   -- Start a subprocess with piped stdin/stdout.
// - SubprocessSession::execute() -- Send a sentinel-delimited code payload and read back
//                                   the sentinel-delimited result (with timeout).
// - Drop                         -- Close stdin and wait for the subprocess to exit.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Command, Stdio};
use std::sync::LazyLock;
use std::time::Duration;

/// Chunk execution timeout. None means no timeout (wait forever).
/// Override with CALEPIN_TIMEOUT env var or `timeout` in _calepin.toml [defaults].
/// Absent/null in TOML means infinite; a numeric value sets the timeout in seconds.
static CHUNK_TIMEOUT: LazyLock<Option<Duration>> = LazyLock::new(|| {
    if let Some(secs) = std::env::var("CALEPIN_TIMEOUT").ok().and_then(|s| s.parse::<u64>().ok()) {
        return Some(Duration::from_secs(secs));
    }
    crate::project::get_defaults().timeout.map(|s| Duration::from_secs(s))
});

/// A persistent subprocess that communicates via stdin/stdout.
/// Used by both R and Python engines.
pub struct SubprocessSession {
    child: Child,
    stdin: Option<BufWriter<std::process::ChildStdin>>,
    /// Reader thread sends lines via this channel.
    reader_rx: Option<std::sync::mpsc::Receiver<ReaderMsg>>,
    /// Handle for the reader thread (joined on drop).
    _reader_handle: Option<std::thread::JoinHandle<()>>,
}

enum ReaderMsg {
    Line(String),
    Eof,
    Error(std::io::Error),
}

impl SubprocessSession {
    /// Spawn a subprocess with piped stdin/stdout, optional env vars and working directory.
    /// stderr is inherited (warnings go to terminal).
    /// A reader thread is spawned to enable timeout-based reads.
    pub fn spawn(program: &str, args: &[&str], env: &[(&str, &str)], cwd: Option<&std::path::Path>) -> Result<Self> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (k, v) in env {
            cmd.env(k, v);
        }
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow::anyhow!("{} not found on PATH", program)
                } else {
                    anyhow::anyhow!("Failed to spawn {}: {}", program, e)
                }
            })?;

        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let stdout = child.stdout.take().unwrap();

        // Spawn a reader thread that sends lines over a channel.
        // This allows execute() to use recv_timeout for chunk timeouts.
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => { let _ = tx.send(ReaderMsg::Eof); break; }
                    Ok(_) => { if tx.send(ReaderMsg::Line(line)).is_err() { break; } }
                    Err(e) => { let _ = tx.send(ReaderMsg::Error(e)); break; }
                }
            }
        });

        Ok(SubprocessSession {
            child,
            stdin: Some(stdin),
            reader_rx: Some(rx),
            _reader_handle: Some(handle),
        })
    }


    /// Send code to the subprocess and read back the sentinel-delimited result.
    /// Times out after the configured timeout (default: no timeout). On timeout,
    /// the subprocess is killed.
    pub fn execute(&mut self, sentinel: &str, payload: &str) -> Result<String> {
        let stdin = self.stdin.as_mut().context("Subprocess stdin closed")?;

        // Send: {sentinel}_BEGIN\n{payload}\n{sentinel}_END\n
        write!(stdin, "{}_BEGIN\n{}\n{}_END\n", sentinel, payload, sentinel)
            .context("Failed to send code to subprocess")?;
        stdin.flush().context("Failed to flush stdin")?;

        // Read lines until {sentinel}_DONE, with optional timeout
        let done_marker = format!("{}_DONE", sentinel);
        let mut output = String::new();
        let timeout = &*CHUNK_TIMEOUT;
        let rx = self.reader_rx.as_ref().context("Reader channel closed")?;

        loop {
            let recv_result = match timeout {
                Some(dur) => rx.recv_timeout(*dur),
                None => rx.recv().map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected),
            };
            match recv_result {
                Ok(ReaderMsg::Line(line)) => {
                    let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
                    if trimmed == done_marker {
                        break;
                    }
                    output.push_str(&line);
                }
                Ok(ReaderMsg::Eof) => {
                    anyhow::bail!("Subprocess exited unexpectedly");
                }
                Ok(ReaderMsg::Error(e)) => {
                    anyhow::bail!("Failed to read from subprocess: {}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Kill the hung subprocess
                    let _ = self.child.kill();
                    anyhow::bail!(
                        "Code chunk timed out after {}s (set timeout in _calepin.toml or CALEPIN_TIMEOUT env var)",
                        timeout.unwrap().as_secs()
                    );
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("Subprocess reader thread terminated unexpectedly");
                }
            }
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
        // Drop the receiver so the reader thread's send will fail and it exits
        drop(self.reader_rx.take());
        let _ = self.child.wait();
    }
}
