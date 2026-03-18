// Shell engine session via a persistent /bin/sh subprocess.
//
// A single sh process runs for the lifetime of the document render. Variables,
// cd, and other shell state persist across chunks — same semantics as R/Python.
//
// No figure support: shell chunks produce only text output and errors.
//
// ## Functions
//
// - ShSession::init()            — Spawn /bin/sh with the bootstrap read-eval loop.
// - ShSession::capture()         — Execute a shell code chunk and capture output.
// - ShSession::evaluate_inline() — Evaluate a single shell expression and return trimmed output.

use anyhow::{Context, Result};

use super::make_sentinel;
use super::subprocess::SubprocessSession;

/// Bootstrap shell script sent once at startup.
/// Reads sentinel-delimited code blocks from stdin, executes them with eval,
/// and writes sentinel-tagged results to stdout.
const SH_BOOTSTRAP: &str = r#"
_calepin_tmpfile=$(mktemp)
trap 'rm -f "$_calepin_tmpfile"' EXIT
while IFS= read -r _line; do
    case "$_line" in
        *_BEGIN)
            _sentinel="${_line%_BEGIN}"
            _code=""
            while IFS= read -r _line; do
                case "$_line" in
                    "${_sentinel}_END") break ;;
                    *) _code="${_code}${_line}
" ;;
                esac
            done
            _exit=0
            eval "$_code" > "$_calepin_tmpfile" 2>&1 || _exit=$?
            _output=$(cat "$_calepin_tmpfile")
            if [ "$_exit" -ne 0 ]; then
                printf '%s' "${_sentinel}_ERROR:${_output}"
                printf '\n'
            elif [ -n "$_output" ]; then
                printf '%s' "${_sentinel}_OUTPUT:${_output}"
                printf '\n'
            fi
            printf '%s' "${_sentinel}_DONE"
            printf '\n'
            ;;
    esac
done
"#;

pub struct ShSession {
    session: SubprocessSession,
    _bootstrap_file: tempfile::NamedTempFile,
}

impl ShSession {
    pub fn init() -> Result<Self> {
        let bootstrap_file = tempfile::NamedTempFile::new()
            .context("Failed to create temp file for sh bootstrap")?;
        std::fs::write(bootstrap_file.path(), SH_BOOTSTRAP)
            .context("Failed to write sh bootstrap")?;
        let path_str = bootstrap_file.path().to_string_lossy().to_string();
        let session = SubprocessSession::spawn("/bin/sh", &[&path_str])
            .context("Failed to start /bin/sh")?;
        Ok(ShSession { session, _bootstrap_file: bootstrap_file })
    }

    /// Execute a shell code chunk and return sentinel-tagged output.
    pub fn capture(&mut self, code: &str) -> Result<String> {
        let sentinel = make_sentinel();
        self.session.execute(&sentinel, code)
    }

    /// Evaluate a single shell expression and return trimmed output.
    pub fn evaluate_inline(&mut self, expr: &str) -> Result<String> {
        let sentinel = make_sentinel();
        let raw = self.session.execute(&sentinel, expr)?;

        // Extract just the output text from sentinel-tagged lines
        let output_prefix = format!("{}_OUTPUT:", sentinel);
        let mut result = String::new();
        for line in raw.lines() {
            if let Some(text) = line.strip_prefix(&output_prefix) {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(text);
            }
        }
        Ok(result.trim().to_string())
    }
}
