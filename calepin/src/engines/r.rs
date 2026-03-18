// R engine session via a persistent Rscript subprocess.
//
// ## Design
//
// A single Rscript process runs for the lifetime of the document render. On init,
// a bootstrap script is written to a temp file and executed with --no-save
// --no-restore. The bootstrap sets up a read-eval loop over stdin/stdout using a
// sentinel-delimited protocol (see subprocess.rs). All chunks execute in the
// global environment, so variables persist across chunks — notebook semantics.
//
// Two execution modes:
// - **Block** (`capture`): each expression is eval'd individually via
//   capture.output(), with warnings and messages intercepted by
//   withCallingHandlers(). A graphics device is opened before execution and
//   closed after, so any plots are saved to the requested path.
// - **Inline** (`evaluate_inline`): eval() a single expression. Numeric scalars
//   are formatted with `format(digits=3, big.mark=",")` for readable output.
//
// The graphics device type (png, svg, cairo_pdf, etc.) is configurable per chunk
// via the `dev` option. Raster devices get `units="in"` and `res=150`.
//
// ## Functions
//
// - RSession::init()            — Spawn Rscript with the bootstrap read-eval loop.
// - RSession::evaluate_inline() — Evaluate a single R expression and return the formatted result.
// - RSession::capture()         — Execute an R code chunk with output/warning/message/plot capture
//                                 using the sentinel protocol.

use anyhow::{Context, Result};

use super::make_sentinel;
use super::subprocess::SubprocessSession;

/// Bootstrap R script sent once at startup.
/// Sets up a read-eval loop that reads sentinel-delimited code blocks from stdin,
/// executes them with output/warning/message/error/plot capture, and writes
/// sentinel-delimited results to stdout.
const R_BOOTSTRAP: &str = r#"
.calepin_loop <- function() {
  con <- file("stdin", "r")
  while (TRUE) {
    header <- readLines(con, n = 1, warn = FALSE)
    if (length(header) == 0) break
    sentinel <- sub("_BEGIN$", "", header)
    end_marker <- paste0(sentinel, "_END")

    lines <- character(0)
    repeat {
      line <- readLines(con, n = 1, warn = FALSE)
      if (length(line) == 0 || line == end_marker) break
      lines <- c(lines, line)
    }

    # First line is metadata: MODE:..., rest is code
    meta_line <- lines[1]
    code <- paste(lines[-1], collapse = "\n")

    if (startsWith(meta_line, "INLINE:")) {
      # Inline eval mode
      expr_text <- sub("^INLINE:", "", meta_line)
      result <- tryCatch({
        .val <- eval(parse(text = expr_text), envir = globalenv())
        if (is.numeric(.val) && length(.val) == 1) {
          format(.val, digits = 3, big.mark = ",")
        } else {
          paste(as.character(.val), collapse = ", ")
        }
      }, error = function(e) {
        paste0(sentinel, "_ERROR:", conditionMessage(e))
      })
      cat(result, "\n", sep = "")
      cat(sentinel, "_DONE\n", sep = "")
      flush(stdout())
      next
    }

    # Parse metadata: fig_path, dev, width, height
    meta <- list()
    for (item in strsplit(sub("^META:", "", meta_line), ";")[[1]]) {
      kv <- strsplit(item, "=", fixed = TRUE)[[1]]
      if (length(kv) == 2) meta[[kv[1]]] <- kv[2]
    }
    fig_path <- meta[["fig_path"]]
    dev_name <- meta[["dev"]]
    width <- as.numeric(meta[["width"]])
    height <- as.numeric(meta[["height"]])

    sep <- paste0(sentinel, "_SEP")

    # Open graphics device
    has_plot <- FALSE
    if (nzchar(fig_path) && nzchar(dev_name)) {
      dev_fun <- match.fun(dev_name)
      # Raster devices (png, jpeg, etc.) need units and resolution
      if (dev_name %in% c("png", "jpeg", "bmp", "tiff")) {
        dev_fun(fig_path, width = width, height = height, units = "in", res = 150)
      } else {
        dev_fun(fig_path, width = width, height = height)
      }
      on.exit({ if (dev.cur() > 1) dev.off() }, add = TRUE)
    }

    warns <- character(0)
    msgs <- character(0)
    out <- tryCatch(
      withCallingHandlers(
        {
          exprs <- parse(text = code)
          res <- character(0)
          for (expr in exprs) {
            r <- capture.output(eval(expr, envir = globalenv()))
            if (length(r) > 0) res <- c(res, r)
          }
          paste(res, collapse = "\n")
        },
        warning = function(w) {
          warns <<- c(warns, conditionMessage(w))
          invokeRestart("muffleWarning")
        },
        message = function(m) {
          msgs <<- c(msgs, conditionMessage(m))
          invokeRestart("muffleMessage")
        }
      ),
      error = function(e) {
        paste0(sentinel, "_ERROR:", conditionMessage(e))
      }
    )

    if (dev.cur() > 1) dev.off()
    on.exit(NULL)

    if (nzchar(fig_path)) {
      has_plot <- file.exists(fig_path) && file.info(fig_path)$size > 0
    }

    parts <- character(0)
    if (nzchar(out)) parts <- c(parts, paste0(sentinel, "_OUTPUT:", out))
    if (length(warns) > 0) parts <- c(parts, paste0(sentinel, "_WARNING:", paste(warns, collapse = "\n")))
    if (length(msgs) > 0) parts <- c(parts, paste0(sentinel, "_MESSAGE:", paste(msgs, collapse = "\n")))
    if (has_plot) parts <- c(parts, paste0(sentinel, "_PLOT:", fig_path))

    result <- paste(parts, collapse = paste0("\n", sep, "\n"))
    cat(result, "\n", sep = "")
    cat(sentinel, "_DONE\n", sep = "")
    flush(stdout())
  }
}
.calepin_loop()
"#;

/// RAII guard for the R subprocess.
pub struct RSession {
    proc: SubprocessSession,
    _bootstrap_file: tempfile::NamedTempFile,
}

impl RSession {
    /// Spawn an Rscript subprocess running the bootstrap script.
    pub fn init() -> Result<Self> {
        let bootstrap_file = tempfile::NamedTempFile::new()
            .context("Failed to create temp file for R bootstrap")?;
        std::fs::write(bootstrap_file.path(), R_BOOTSTRAP)
            .context("Failed to write R bootstrap")?;
        let path_str = bootstrap_file.path().to_string_lossy().to_string();
        let proc = SubprocessSession::spawn("Rscript", &["--no-save", "--no-restore", &path_str])
            .context("Failed to start R")?;
        Ok(RSession { proc, _bootstrap_file: bootstrap_file })
    }

    /// Evaluate an inline R expression and return the formatted result.
    pub fn evaluate_inline(&mut self, expr: &str) -> Result<String> {
        let sentinel = make_sentinel();
        let payload = format!("INLINE:{}", expr);
        let raw = self.proc.execute(&sentinel, &payload)?;
        // Result is: {sentinel}\n{result}
        let (_, result) = raw.split_once('\n').unwrap_or(("", ""));
        Ok(result.to_string())
    }

    /// Capture R code output using the sentinel protocol.
    pub fn capture(
        &mut self,
        code: &str,
        fig_path: &str,
        dev: &str,
        width: f64,
        height: f64,
    ) -> Result<String> {
        let sentinel = make_sentinel();
        let meta = format!(
            "META:fig_path={};dev={};width={};height={}",
            fig_path, dev, width, height
        );
        let payload = format!("{}\n{}", meta, code);
        self.proc.execute(&sentinel, &payload)
    }
}
