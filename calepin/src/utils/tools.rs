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

#[cfg(windows)]
const PYTHON_COMMAND: &str = "python";
#[cfg(not(windows))]
const PYTHON_COMMAND: &str = "python3";

// ---------------------------------------------------------------------------
// Tool registry
// ---------------------------------------------------------------------------

pub const RSCRIPT: Tool = Tool {
    cmd: "Rscript",
    install_hint: "install from https://cran.r-project.org/",
};

pub const PYTHON: Tool = Tool {
    cmd: PYTHON_COMMAND,
    install_hint: "install from https://www.python.org/downloads/",
};

pub const TYPST: Tool = Tool {
    cmd: "typst",
    install_hint: concat!(
        "install from https://github.com/typst/typst ",
        "or set `[executables] typst` to the Typst executable path"
    ),
};

pub const JUPYTER_CLIENT: Tool = Tool {
    cmd: PYTHON_COMMAND,
    install_hint: "jupyter_client not found — install with: pip install jupyter_client",
};

pub const MMDC: Tool = Tool {
    cmd: "mmdc",
    install_hint: "install with `npm install -g @mermaid-js/mermaid-cli`",
};

pub const DOT: Tool = Tool {
    cmd: "dot",
    install_hint: "install Graphviz from https://graphviz.org/download/",
};

pub const TECTONIC: Tool = Tool {
    cmd: "tectonic",
    install_hint: "install from https://tectonic-typesetting.github.io/",
};

pub const DVISVGM: Tool = Tool {
    cmd: "dvisvgm",
    install_hint: "install TeX Live or MacTeX from https://tug.org/texlive/",
};

pub const PDF2SVG: Tool = Tool {
    cmd: "pdf2svg",
    install_hint: "install with `brew install pdf2svg` or from your system package manager",
};

pub const D2: Tool = Tool {
    cmd: "d2",
    install_hint: "install from https://d2lang.com/tour/install/",
};
