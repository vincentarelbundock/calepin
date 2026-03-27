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


pub const MMDC: Tool = Tool {
    cmd: "mmdc",
    install_hint: "install with: npm install -g @mermaid-js/mermaid-cli",
};

pub const DOT: Tool = Tool {
    cmd: "dot",
    install_hint: "install from https://graphviz.org/download/",
};

pub const TECTONIC: Tool = Tool {
    cmd: "tectonic",
    install_hint: "install from https://tectonic-typesetting.github.io/",
};

pub const PDF2SVG: Tool = Tool {
    cmd: "pdf2svg",
    install_hint: "install from https://github.com/dawbarton/pdf2svg",
};

pub const D2: Tool = Tool {
    cmd: "d2",
    install_hint: "install from https://d2lang.com/",
};

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// Format a "not found" error message for a tool.
pub fn not_found_message(tool: &Tool) -> String {
    format!("{} not found on PATH. {}", tool.cmd, tool.install_hint)
}
