// External tool availability checks and error messages.
//
// Centralizes all knowledge about which CLI tools calepin depends on,
// how to detect them, and what to tell the user when they're missing.

/// An external tool that calepin may invoke.
pub struct Tool {
    /// Command name (looked up on PATH).
    pub cmd: &'static str,
    /// Install instructions shown when the tool is missing.
    pub install_hint: &'static str,
}

// ---------------------------------------------------------------------------
// Tool registry
// ---------------------------------------------------------------------------

pub const RSCRIPT: Tool = Tool {
    cmd: "Rscript",
    install_hint: "install from https://cran.r-project.org/",
};

pub const PYTHON: Tool = Tool {
    cmd: "python3",
    install_hint: "install from https://www.python.org/downloads/",
};

pub const SH: Tool = Tool {
    cmd: "/bin/sh",
    install_hint: "/bin/sh should be available on any Unix system",
};

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// Format a "not found" error message for a tool.
pub fn not_found_message(tool: &Tool) -> String {
    format!("{} not found on PATH. {}", tool.cmd, tool.install_hint)
}
