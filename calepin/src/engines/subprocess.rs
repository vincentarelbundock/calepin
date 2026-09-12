// Persistent subprocess communication layer for R and Python engines.
//
// ## Sentinel protocol
//
// Each request/response pair is framed by a unique sentinel string generated from
// PID + an atomic counter (see make_sentinel() in mod.rs). This avoids practical
// collisions with user output.
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
// produce a DONE marker within the timeout, the subprocess is terminated and an
// error is returned. The default is unbounded (no timeout); pass `--timeout` on
// the calepin command line to set one. This is the one timeout rule shared by
// every engine (R, Python, Jupyter): whatever duration reaches SubprocessSession
// applies uniformly, so no engine may invent its own fallback.
//
// On timeout, the subprocess is asked to terminate gracefully first (SIGTERM on
// unix, so a Jupyter bridge gets a chance to shut down the kernels it started),
// then killed outright after a short grace period if it is still alive.
//
// ## Functions
//
// - SubprocessSession::spawn()   -- Start a subprocess with piped stdin/stdout.
// - spawn_script()               -- Write a bootstrap script to a temp file and run it.
// - SubprocessSession::execute() -- Send a sentinel-delimited code payload and read back
//                                   the sentinel-delimited result (with timeout).
// - Drop                         -- Close stdin and wait for the subprocess to exit.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::utils::process;
use crate::utils::tools::Tool;

/// A persistent subprocess that communicates via stdin/stdout.
/// Used by both R and Python engines.
pub struct SubprocessSession {
    child: Child,
    stdin: Option<BufWriter<std::process::ChildStdin>>,
    /// Reader thread sends lines via this channel.
    reader_rx: Option<std::sync::mpsc::Receiver<ReaderMsg>>,
    /// Handle for the detached reader thread.
    _reader_handle: Option<std::thread::JoinHandle<()>>,
    /// Chunk execution timeout.
    timeout: Option<Duration>,
    /// Set once the subprocess is known to be dead (timed out, exited, or the
    /// reader thread errored). A dead session must never be handed out for a
    /// new chunk; the caller should respawn instead.
    dead: bool,
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
    pub fn spawn(
        program: &Path,
        args: &[&str],
        env: &[(&str, &str)],
        cwd: Option<&std::path::Path>,
        timeout: Option<Duration>,
        tool: Option<&Tool>,
    ) -> Result<Self> {
        process::validate_executable(program, "start subprocess", tool)?;
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
            .map_err(|error| process::spawn_error(program, "start subprocess", error, tool))?;

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
                    Ok(0) => {
                        let _ = tx.send(ReaderMsg::Eof);
                        break;
                    }
                    Ok(_) => {
                        if tx.send(ReaderMsg::Line(line)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(ReaderMsg::Error(e));
                        break;
                    }
                }
            }
        });

        Ok(SubprocessSession {
            child,
            stdin: Some(stdin),
            reader_rx: Some(rx),
            _reader_handle: Some(handle),
            timeout,
            dead: false,
        })
    }

    /// Returns the configured execution timeout.
    pub fn timeout(&self) -> Option<std::time::Duration> {
        self.timeout
    }

    /// True once the subprocess is known dead (timed out, exited unexpectedly,
    /// or the reader thread failed). Callers must respawn rather than reuse.
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Send code to the subprocess and read back the sentinel-delimited result.
    /// Times out after the configured timeout (default: no timeout). On timeout,
    /// the subprocess is killed.
    pub fn execute(&mut self, sentinel: &str, payload: &str) -> Result<String> {
        if self.dead {
            anyhow::bail!("Subprocess is no longer running (a previous chunk timed out or it exited unexpectedly)");
        }
        let stdin = self.stdin.as_mut().context("Subprocess stdin closed")?;

        // Send: {sentinel}_BEGIN\n{payload}\n{sentinel}_END\n
        //
        // A subprocess that has already died leaves a closed pipe here, so the
        // write fails with BrokenPipe rather than the reader seeing EOF. That
        // is the same situation as an unexpected exit and has to report as one:
        // relaying the raw io error told the user their engine "failed to flush
        // stdin", which names a mechanism instead of the problem.
        let send = write!(stdin, "{}_BEGIN\n{}\n{}_END\n", sentinel, payload, sentinel)
            .and_then(|()| stdin.flush());
        if let Err(error) = send {
            if error.kind() == std::io::ErrorKind::BrokenPipe {
                self.dead = true;
                anyhow::bail!("Subprocess exited unexpectedly");
            }
            return Err(anyhow::Error::new(error).context("Failed to send code to subprocess"));
        }

        // Read lines until {sentinel}_DONE, with optional timeout
        let done_marker = format!("{}_DONE", sentinel);
        let mut output = String::new();
        let timeout = &self.timeout;
        let rx = self.reader_rx.as_ref().context("Reader channel closed")?;

        loop {
            let recv_result = match timeout {
                Some(dur) => rx.recv_timeout(*dur),
                None => rx
                    .recv()
                    .map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected),
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
                    self.dead = true;
                    anyhow::bail!("Subprocess exited unexpectedly");
                }
                Ok(ReaderMsg::Error(e)) => {
                    self.dead = true;
                    anyhow::bail!("Failed to read from subprocess: {}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Give the hung subprocess a chance to shut down gracefully,
                    // then kill it. Either way it must not be reused afterward.
                    terminate_child(&mut self.child);
                    self.dead = true;
                    anyhow::bail!(
                        "Code chunk timed out after {}s (pass --timeout to change this)",
                        timeout.unwrap().as_secs()
                    );
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.dead = true;
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

/// Ask a child process to exit gracefully, then give it a short grace period
/// before killing it outright. On unix this sends SIGTERM (via the `kill`
/// binary, so no extra dependency is needed) so a bridge process such as the
/// Jupyter bridge gets a chance to run its own shutdown/cleanup code instead
/// of being killed mid-cleanup. Platforms with no graceful-signal equivalent
/// just kill immediately.
fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let deadline = std::time::Instant::now() + Duration::from_millis(300);
        while std::time::Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let _ = child.kill();
}

pub fn spawn_script(
    program: &Path,
    args_before_script: &[&str],
    script: &str,
    context_name: &str,
    cwd: Option<&Path>,
    timeout: Option<Duration>,
    tool: Option<&Tool>,
) -> Result<(SubprocessSession, tempfile::NamedTempFile)> {
    let script_file = tempfile::NamedTempFile::new()
        .with_context(|| format!("Failed to create temp file for {context_name} bootstrap"))?;
    std::fs::write(script_file.path(), script)
        .with_context(|| format!("Failed to write {context_name} bootstrap"))?;
    let path_str = script_file.path().to_string_lossy().to_string();
    let mut args = Vec::with_capacity(args_before_script.len() + 1);
    args.extend(args_before_script.iter().copied());
    args.push(path_str.as_str());
    let proc = SubprocessSession::spawn(program, &args, &[], cwd, timeout, tool)
        .with_context(|| format!("Failed to start {context_name}"))?;
    Ok((proc, script_file))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str, timeout: Option<Duration>) -> SubprocessSession {
        SubprocessSession::spawn(Path::new("sh"), &["-c", script], &[], None, timeout, None)
            .expect("failed to spawn sh fixture")
    }

    // A tiny read-eval loop that mimics the real R/Python bootstraps closely
    // enough to exercise the framing + `_DONE` protocol: it waits for
    // `{sentinel}_BEGIN`, buffers lines until `{sentinel}_END`, then echoes
    // the buffered payload back tagged `_OUTPUT:` before the DONE marker.
    const ECHO_SCRIPT: &str = r#"
sentinel=""
payload=""
while IFS= read -r line; do
  case "$line" in
    *_BEGIN)
      sentinel="${line%_BEGIN}"
      payload=""
      ;;
    *)
      if [ "$line" = "${sentinel}_END" ]; then
        printf '%s_OUTPUT:%s\n' "$sentinel" "$payload"
        printf '%s_DONE\n' "$sentinel"
      elif [ -z "$payload" ]; then
        payload="$line"
      else
        payload="$payload $line"
      fi
      ;;
  esac
done
"#;

    #[test]
    fn execute_frames_the_request_and_reads_until_done() {
        let mut session = sh(ECHO_SCRIPT, Some(Duration::from_secs(5)));
        let raw = session.execute("__T1__", "hello-payload").unwrap();

        assert!(raw.starts_with("__T1__\n"), "{raw}");
        assert!(raw.contains("_OUTPUT:hello-payload"), "{raw}");
        // The DONE marker itself must never leak into the returned text.
        assert!(!raw.contains("_DONE"), "{raw}");
    }

    #[test]
    fn execute_treats_done_as_an_exact_line_match_not_a_substring() {
        // Emits a line that contains "_DONE" as a substring before the real
        // marker; a correct reader must not stop early on it.
        const SCRIPT: &str = r#"
sentinel=""
while IFS= read -r line; do
  case "$line" in
    *_BEGIN) sentinel="${line%_BEGIN}" ;;
    *_END)
      printf '%s_OUTPUT:not-the-marker\n' "$sentinel"
      printf '%s_DONE_DECOY\n' "$sentinel"
      printf '%s_DONE\n' "$sentinel"
      ;;
  esac
done
"#;
        let mut session = sh(SCRIPT, Some(Duration::from_secs(5)));
        let raw = session.execute("__T2__", "x").unwrap();

        assert!(raw.contains("_DONE_DECOY"), "{raw}");
        assert!(raw.contains("not-the-marker"), "{raw}");
    }

    #[test]
    fn execute_reports_eof_and_marks_the_session_dead() {
        // Exits without ever producing a DONE marker.
        let mut session = sh("exit 0", Some(Duration::from_secs(5)));
        let err = session.execute("__T3__", "x").unwrap_err();

        assert!(err.to_string().contains("exited unexpectedly"), "{err}");
        assert!(session.is_dead());

        // A dead session must be rejected up front rather than handed out
        // for another chunk (the bug: a killed/exited session was still used).
        let err2 = session.execute("__T3__", "x").unwrap_err();
        assert!(err2.to_string().contains("no longer running"), "{err2}");
    }

    #[test]
    fn execute_kills_a_hung_subprocess_on_timeout_and_marks_it_dead() {
        let mut session = sh("sleep 30", Some(Duration::from_millis(150)));
        let err = session.execute("__T4__", "x").unwrap_err();

        assert!(err.to_string().contains("timed out"), "{err}");
        assert!(session.is_dead());

        let err2 = session.execute("__T4__", "x").unwrap_err();
        assert!(err2.to_string().contains("no longer running"), "{err2}");
    }
}
