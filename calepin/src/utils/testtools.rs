// Support for the tool-skip pattern used throughout calepin's test suite: a
// test that needs an external tool (`typst`, `Rscript`, `python3`, a Python
// module, ...) checks for it and returns early when it is missing, rather
// than failing. That is convenient locally, on a machine that only has some
// of the engines installed, but it is silent in CI: a runner missing every
// tool reports a fully green suite that asserted nothing.
//
// Set `CALEPIN_TEST_REQUIRE_TOOLS=1` to turn every such skip into a panic
// naming the missing tool, so a CI job that installs every tool the suite
// needs fails loudly on a regression instead of quietly skipping it.
//
// This file is compiled twice: once as part of the crate (for unit tests,
// gated behind `#[cfg(test)]` in `utils/mod.rs`), and once included
// directly into the integration test binary via `#[path = ...]`, since
// `calepin` has no library target for that binary to depend on.

use std::process::Command;

fn tools_are_mandatory() -> bool {
    std::env::var_os("CALEPIN_TEST_REQUIRE_TOOLS").is_some()
}

/// Wraps a plain availability check: `available` is what the caller found
/// (a command that ran, a module that imported, ...), and `what` names it
/// for the panic message. Returns `available` unchanged unless
/// `CALEPIN_TEST_REQUIRE_TOOLS` is set and `available` is false, in which
/// case it panics naming `what`.
pub fn require(available: bool, what: &str) -> bool {
    if !available && tools_are_mandatory() {
        panic!(
            "required test tool `{what}` is not available, but \
             CALEPIN_TEST_REQUIRE_TOOLS=1 is set; install `{what}` or unset \
             the variable to allow this test to skip"
        );
    }
    available
}

/// Runs `command --version` and reports whether it succeeded, honoring
/// `CALEPIN_TEST_REQUIRE_TOOLS` via [`require`].
pub fn command_available(command: &str) -> bool {
    let available = Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    require(available, command)
}
