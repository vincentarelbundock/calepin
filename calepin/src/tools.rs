// External tool availability checks and error messages.
//
// Centralizes all knowledge about which CLI tools calepin depends on,
// how to detect them, and what to tell the user when they're missing.

use std::path::PathBuf;

/// An external tool that calepin may invoke.
pub struct Tool {
    /// Command name (looked up on PATH).
    pub cmd: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// Install instructions shown when the tool is missing.
    pub install_hint: &'static str,
}

// ---------------------------------------------------------------------------
// Tool registry
// ---------------------------------------------------------------------------

pub const RSCRIPT: Tool = Tool {
    cmd: "Rscript",
    name: "R",
    install_hint: "install from https://cran.r-project.org/",
};

pub const PYTHON: Tool = Tool {
    cmd: "python3",
    name: "Python",
    install_hint: "install from https://www.python.org/downloads/",
};

pub const SH: Tool = Tool {
    cmd: "/bin/sh",
    name: "Shell",
    install_hint: "/bin/sh should be available on any Unix system",
};

pub const PANDOC: Tool = Tool {
    cmd: "pandoc",
    name: "Pandoc",
    install_hint: "install from https://pandoc.org/installing.html",
};

pub const MMDC: Tool = Tool {
    cmd: "mmdc",
    name: "Mermaid CLI",
    install_hint: "install with: npm install -g @mermaid-js/mermaid-cli",
};

pub const DOT: Tool = Tool {
    cmd: "dot",
    name: "Graphviz",
    install_hint: "install from https://graphviz.org/download/",
};

pub const TECTONIC: Tool = Tool {
    cmd: "tectonic",
    name: "Tectonic",
    install_hint: "install from https://tectonic-typesetting.github.io/",
};

pub const PDF2SVG: Tool = Tool {
    cmd: "pdf2svg",
    name: "pdf2svg",
    install_hint: "install from https://github.com/dawbarton/pdf2svg",
};

pub const D2: Tool = Tool {
    cmd: "d2",
    name: "D2",
    install_hint: "install from https://d2lang.com/",
};

pub const PAGEFIND: Tool = Tool {
    cmd: "pagefind",
    name: "Pagefind",
    install_hint: "install with: npm install -g pagefind",
};

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// Check whether a tool is available on PATH.
/// Returns the path if found, None otherwise.
pub fn find_tool(tool: &Tool) -> Option<PathBuf> {
    which(tool.cmd)
}

/// Check whether a tool is available. Returns a user-friendly error if not.
pub fn require_tool(tool: &Tool) -> Result<PathBuf, String> {
    find_tool(tool).ok_or_else(|| not_found_message(tool))
}

/// Format a "not found" error message for a tool.
pub fn not_found_message(tool: &Tool) -> String {
    format!("{} not found on PATH. {}", tool.cmd, tool.install_hint)
}

/// Look up a command on PATH, returning its full path if found.
fn which(cmd: &str) -> Option<PathBuf> {
    // Absolute paths are checked directly
    if cmd.starts_with('/') {
        let p = PathBuf::from(cmd);
        return if p.exists() { Some(p) } else { None };
    }
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Classify an `io::Error` from `Command::new(cmd).spawn()` / `.output()`.
/// Returns a user-friendly message if the tool is not found, or re-wraps
/// other errors.
pub fn check_spawn_error(err: std::io::Error, tool: &Tool) -> anyhow::Error {
    if err.kind() == std::io::ErrorKind::NotFound {
        anyhow::anyhow!("{}", not_found_message(tool))
    } else {
        anyhow::anyhow!("failed to run {}: {}", tool.cmd, err)
    }
}
