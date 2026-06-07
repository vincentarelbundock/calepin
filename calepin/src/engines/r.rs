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
// via the `dev` option. Raster devices get `units="in"` and the requested DPI.
//
// ## Functions
//
// - RSession::init(format)      — Spawn Rscript with the bootstrap read-eval loop.
// - RSession::evaluate_inline() — Evaluate a single R expression and return the formatted result.
// - RSession::capture()         — Execute an R code chunk with output/warning/message/plot capture
//                                 using the sentinel protocol.

use anyhow::Result;

use super::make_sentinel;
use super::subprocess::{spawn_script, SubprocessSession};

/// Format placeholder replaced at init time with the actual output format.
const FORMAT_PLACEHOLDER: &str = "__CALEPIN_FORMAT__";

/// Bootstrap R script sent once at startup.
/// Sets up a read-eval loop that reads sentinel-delimited code blocks from stdin,
/// executes them with output/warning/message/error/plot capture, and writes
/// sentinel-delimited results to stdout.
const R_BOOTSTRAP: &str = r#"
# Tell knitr-aware packages (tinytable, gt, etc.) what format we are rendering to.
# Warning: commented out because it corrupts #block[. This may be tinytable magic behavior for Quarto.
# local({
#   options(knitr.in.progress = TRUE)
#   tryCatch(
#     if (requireNamespace("knitr", quietly = TRUE)) {
#       try(knitr::opts_knit$set(rmarkdown.pandoc.to = "typst"), silent = TRUE)
#     },
#     error = function(e) NULL
#   )
# })

.calepin_has_knitr <- requireNamespace("knitr", quietly = TRUE)

# Preamble buffer: R code can call calepin.preamble() to inject content
# into the document preamble (e.g. \usepackage lines, HTML <head> elements).
.calepin_preamble_buf <- character(0)

calepin.preamble <- function(text) {
  .calepin_preamble_buf <<- c(.calepin_preamble_buf, text)
  invisible(NULL)
}

.calepin_loop <- function() {
  con <- file("stdin", "r")
  while (TRUE) {
    header <- readLines(con, n = 1, warn = FALSE)
    if (length(header) == 0) break
    sentinel <- sub("_BEGIN$", "", header)
    end_marker <- paste0(sentinel, "_END")

    lines <- list()
    .i <- 0L
    repeat {
      line <- readLines(con, n = 1, warn = FALSE)
      if (length(line) == 0 || line == end_marker) break
      .i <- .i + 1L
      lines[[.i]] <- line
    }
    lines <- unlist(lines)

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

    # Parse metadata: fig_path, dev, width, height, dpi
    meta <- list()
    for (item in strsplit(sub("^META:", "", meta_line), ";")[[1]]) {
      eq <- regexpr("=", item, fixed = TRUE)
      if (eq > 0) {
        key <- substr(item, 1L, eq - 1L)
        value <- substr(item, eq + 1L, nchar(item))
        meta[[key]] <- value
      }
    }
    fig_path <- meta[["fig_path"]]
    if (is.null(fig_path)) fig_path <- ""
    dev_name <- meta[["dev"]]
    if (is.null(dev_name)) dev_name <- ""
    width <- as.numeric(meta[["width"]])
    height <- as.numeric(meta[["height"]])
    dpi <- as.numeric(meta[["dpi"]])
    if (!is.finite(dpi)) dpi <- 150

    sep <- paste0(sentinel, "_SEP")
    parts <- character(0)
    warns <- character(0)
    msgs <- character(0)
    .calepin_env <- environment()
    err_out <- NULL
    device_id <- NA_integer_
    last_plot_state <- NULL
    plot_pending <- FALSE
    pending_plot_state <- NULL
    plot_index <- 1L
    device_path <- if (isTRUE(nzchar(fig_path))) paste0(fig_path, ".device") else ""

    .calepin_plot_threshold <- function(dev_name) {
      if (dev_name %in% c("pdf", "cairo_pdf")) {
        4000
      } else if (dev_name %in% c("svg")) {
        300
      } else {
        0
      }
    }

    .calepin_plot_state <- function() {
      open_devices <- dev.list()
      if (is.na(device_id) || is.null(open_devices) || !(device_id %in% open_devices)) {
        return(NULL)
      }
      tryCatch(recordPlot(), error = function(e) NULL)
    }

    .calepin_note_plot_change <- function() {
      current <- .calepin_plot_state()
      if (is.null(current)) {
        return(FALSE)
      }
      current_key <- tryCatch(serialize(current, NULL), error = function(e) NULL)
      if (is.null(current_key)) {
        return(FALSE)
      }
      changed <- !identical(current_key, last_plot_state)
      last_plot_state <<- current_key
      if (changed) {
        plot_pending <<- TRUE
        pending_plot_state <<- current
      }
      changed
    }

    .calepin_plot_path <- function(index) {
      if (index <= 1L) {
        return(fig_path)
      }
      ext <- tools::file_ext(fig_path)
      if (nzchar(ext)) {
        root <- substr(fig_path, 1L, nchar(fig_path) - nchar(ext) - 1L)
        root <- sub("-1$", "", root)
        paste0(root, "-", index, ".", ext)
      } else {
        root <- sub("-1$", "", fig_path)
        paste0(root, "-", index)
      }
    }

    .calepin_write_plot_state <- function(state, path) {
      if (is.null(state) || !isTRUE(nzchar(path))) {
        return(FALSE)
      }
      previous_device <- dev.cur()
      tryCatch({
        dir.create(dirname(path), recursive = TRUE, showWarnings = FALSE)
        if (file.exists(path)) suppressWarnings(file.remove(path))
        dev_fun <- match.fun(dev_name)
        if (dev_name %in% c("png", "jpeg", "bmp", "tiff")) {
          dev_fun(path, width = width, height = height, units = "in", res = dpi)
        } else {
          dev_fun(path, width = width, height = height)
        }
        replayPlot(state)
        dev.off()
        if (!is.null(dev.list()) && previous_device %in% dev.list()) {
          dev.set(previous_device)
        }
        if (!file.exists(path)) {
          return(FALSE)
        }
        sz <- file.info(path)$size
        is.finite(sz) && sz > .calepin_plot_threshold(dev_name)
      }, error = function(e) {
        if (!is.null(dev.list()) && dev.cur() != previous_device) {
          try(dev.off(), silent = TRUE)
        }
        if (!is.null(dev.list()) && previous_device %in% dev.list()) {
          try(dev.set(previous_device), silent = TRUE)
        }
        warns <<- c(warns, paste0("Failed to save figure: ", conditionMessage(e)))
        FALSE
      })
    }

    .calepin_emit_plot_pending <- function() {
      if (plot_pending) {
        path <- .calepin_plot_path(plot_index)
        if (.calepin_write_plot_state(pending_plot_state, path)) {
          parts <<- c(parts, paste0(sentinel, "_PLOT:", path))
          plot_index <<- plot_index + 1L
        }
        plot_pending <<- FALSE
        pending_plot_state <<- NULL
      }
    }

    # Open graphics device
    has_plot <- FALSE
    if (isTRUE(nzchar(device_path)) && isTRUE(nzchar(dev_name))) {
      tryCatch({
        dir.create(dirname(device_path), recursive = TRUE, showWarnings = FALSE)
        if (file.exists(device_path)) suppressWarnings(file.remove(device_path))
        dev_fun <- match.fun(dev_name)
        # Raster devices (png, jpeg, etc.) need units and resolution
        if (dev_name %in% c("png", "jpeg", "bmp", "tiff")) {
          dev_fun(device_path, width = width, height = height, units = "in", res = dpi)
        } else {
          dev_fun(device_path, width = width, height = height)
        }
        device_id <- dev.cur()
        dev.control(displaylist = "enable")
        initial_plot <- .calepin_plot_state()
        last_plot_state <- if (is.null(initial_plot)) NULL else serialize(initial_plot, NULL)
      }, error = function(e) {
        err_out <<- conditionMessage(e)
      })
    }

    if (is.null(err_out)) {
      tryCatch(
        withCallingHandlers(
          {
            exprs <- parse(text = code, keep.source = TRUE)
            srcs <- attr(exprs, "srcref")
            code_lines <- strsplit(code, "\n", fixed = TRUE)[[1]]
            prev_end <- 0L
            src_buf <- character(0)
            for (i in seq_along(exprs)) {
              if (plot_pending) .calepin_emit_plot_pending()

              # Determine source line range (include gap lines: comments, blanks)
              if (!is.null(srcs) && i <= length(srcs)) {
                last_line <- srcs[[i]][3L]
              } else {
                last_line <- length(code_lines)
              }
              src_buf <- c(src_buf, code_lines[(prev_end + 1L):last_line])
              prev_end <- last_line

              # Capture stdout and direct stderr during eval
              plot_pending_before <- plot_pending
              .err_out <- capture.output(
                .cat_out <- capture.output(
                  .val <- withVisible(eval(exprs[[i]], envir = globalenv()))
                ),
                type = "message"
              )
              .calepin_note_plot_change()

              has_output <- FALSE

              # Emit cat() output first
              if (length(.cat_out) > 0) {
                if (plot_pending_before) .calepin_emit_plot_pending()
                parts <- c(parts, paste0(sentinel, "_SOURCE:", paste(src_buf, collapse = "\n")))
                src_buf <- character(0)
                has_output <- TRUE
                parts <- c(parts, paste0(sentinel, "_OUTPUT:", paste(.cat_out, collapse = "\n")))
              }

              if (length(.err_out) > 0) {
                if (plot_pending_before) .calepin_emit_plot_pending()
                if (!has_output) {
                  parts <- c(parts, paste0(sentinel, "_SOURCE:", paste(src_buf, collapse = "\n")))
                  src_buf <- character(0)
                  has_output <- TRUE
                }
                parts <- c(parts, paste0(sentinel, "_MESSAGE:", paste(.err_out, collapse = "\n")))
              }

              # Then emit visible return value
              if (.val$visible) {
                is_asis <- FALSE
                r <- character(0)
                if (.calepin_has_knitr) {
                  r <- capture.output(assign("kp_val", knitr::knit_print(.val$value), envir = .calepin_env))
                  if (inherits(kp_val, "knit_asis")) {
                    is_asis <- TRUE
                    r <- as.character(kp_val)
                  } else if (length(r) == 0) {
                    r <- capture.output(print(.val$value))
                  }
                } else if (inherits(.val$value, "knit_asis")) {
                  is_asis <- TRUE
                  r <- as.character(.val$value)
                } else {
                  r <- capture.output(print(.val$value))
                }
                if (length(r) > 0) {
                  if (plot_pending_before) .calepin_emit_plot_pending()
                  if (!has_output) {
                    parts <- c(parts, paste0(sentinel, "_SOURCE:", paste(src_buf, collapse = "\n")))
                    src_buf <- character(0)
                  }
                  tag <- if (is_asis) "_ASIS:" else "_OUTPUT:"
                  parts <- c(parts, paste0(sentinel, tag, paste(r, collapse = "\n")))
                }
              }
            }
            # Flush remaining source (trailing expressions + comments)
            remaining <- if (prev_end < length(code_lines)) {
              c(src_buf, code_lines[(prev_end + 1L):length(code_lines)])
            } else {
              src_buf
            }
            if (length(remaining) > 0 && nzchar(trimws(paste(remaining, collapse = "\n")))) {
              parts <- c(parts, paste0(sentinel, "_SOURCE:", paste(remaining, collapse = "\n")))
            }
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
          err_out <<- conditionMessage(e)
        }
      )
    }

    open_devices <- dev.list()
    if (!is.na(device_id) && !is.null(open_devices) && device_id %in% open_devices) {
      dev.off(device_id)
    }

    if (isTRUE(nzchar(device_path)) && file.exists(device_path)) {
      # Empty device files are small but format-specific: empty PDFs are larger
      # than empty SVGs, while empty raster devices often write no file at all.
      sz <- file.info(device_path)$size
      has_plot <- is.finite(sz) && sz > .calepin_plot_threshold(dev_name)
      suppressWarnings(file.remove(device_path))
    }

    # Collect knitr::knit_meta (used by tinytable, gt, etc. for LaTeX dependencies)
    if (.calepin_has_knitr) {
      .km <- knitr::knit_meta("get")
      if (length(.km) > 0) {
        for (m in .km) {
          if (inherits(m, "latex_dependency")) {
            pkg <- m$name
            opts <- m$options
            extra <- m$extra_lines
            if (isTRUE(nzchar(pkg))) {
              if (!is.null(opts) && length(opts) > 0) {
                .calepin_preamble_buf <<- c(.calepin_preamble_buf,
                  paste0("\\usepackage[", paste(opts, collapse = ","), "]{", pkg, "}"))
              } else {
                .calepin_preamble_buf <<- c(.calepin_preamble_buf,
                  paste0("\\usepackage{", pkg, "}"))
              }
              if (!is.null(extra) && length(extra) > 0) {
                .calepin_preamble_buf <<- c(.calepin_preamble_buf, extra)
              }
            }
          } else if (inherits(m, "html_dependency")) {
            if (!is.null(m$head) && nzchar(m$head)) {
              .calepin_preamble_buf <<- c(.calepin_preamble_buf, m$head)
            }
          }
        }
        knitr::knit_meta("clear")
      }
    }

    if (has_plot) {
      if (plot_pending) {
        .calepin_emit_plot_pending()
      }
    }
    if (!is.null(err_out)) {
      parts <- c(parts, paste0(sentinel, "_ERROR:", err_out))
    }
    if (length(warns) > 0) parts <- c(parts, paste0(sentinel, "_WARNING:", paste(warns, collapse = "\n")))
    if (length(msgs) > 0) parts <- c(parts, paste0(sentinel, "_MESSAGE:", paste(msgs, collapse = "\n")))

    if (length(.calepin_preamble_buf) > 0) {
      for (p in .calepin_preamble_buf) {
        parts <- c(parts, paste0(sentinel, "_PREAMBLE:", p))
      }
      .calepin_preamble_buf <<- character(0)
    }

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
    pub fn init_with_program(
        program: &str,
        format: &str,
        cwd: Option<&std::path::Path>,
        timeout: Option<std::time::Duration>,
    ) -> Result<Self> {
        let bootstrap = R_BOOTSTRAP.replace(FORMAT_PLACEHOLDER, format);
        let (proc, bootstrap_file) = spawn_script(
            program,
            &["--no-save", "--no-restore"],
            &bootstrap,
            "R",
            cwd,
            timeout,
        )?;
        Ok(RSession {
            proc,
            _bootstrap_file: bootstrap_file,
        })
    }

    /// Capture R code output using the sentinel protocol.
    pub fn capture(
        &mut self,
        code: &str,
        fig_path: &str,
        dev: &str,
        width: f64,
        height: f64,
        dpi: f64,
    ) -> Result<String> {
        let sentinel = make_sentinel();
        let meta = format!(
            "META:fig_path={};dev={};width={};height={};dpi={}",
            fig_path, dev, width, height, dpi
        );
        let payload = format!("{}\n{}", meta, code);
        self.proc.execute(&sentinel, &payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    fn has_rscript() -> bool {
        Command::new("Rscript")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn session() -> RSession {
        RSession::init_with_program("Rscript", "typst", None, Some(Duration::from_secs(10)))
            .unwrap()
    }

    #[test]
    fn r_session_reports_invalid_figure_device_without_exiting() {
        if !has_rscript() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("bad.svg");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let mut session = session();
        let raw = session
            .capture(
                "cat('should not run')",
                &fig_path,
                "baddev",
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        assert!(raw.contains("_ERROR:"), "{raw}");
        assert!(raw.contains("baddev"), "{raw}");

        let raw = session
            .capture("cat(42)", "", "svg", 6.0, 3.708, 150.0)
            .unwrap();
        assert!(raw.contains("_OUTPUT:42"), "{raw}");
    }

    #[test]
    fn r_session_does_not_report_empty_pdf_as_plot() {
        if !has_rscript() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("empty.pdf");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let mut session = session();
        let raw = session
            .capture(
                "cat('text only')",
                &fig_path,
                "cairo_pdf",
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        assert!(raw.contains("_OUTPUT:text only"), "{raw}");
        assert!(!raw.contains("_PLOT:"), "{raw}");
        assert!(!std::path::Path::new(&fig_path).exists());
    }

    #[test]
    fn r_session_uses_requested_raster_dpi() {
        if !has_rscript() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("plot.png");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let mut session = session();
        let raw = session
            .capture("plot(1:3)", &fig_path, "png", 2.0, 2.0, 77.0)
            .unwrap();

        assert!(raw.contains("_PLOT:"), "{raw}");
        let bytes = std::fs::read(&fig_path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        assert_eq!(width, 154);
        assert_eq!(height, 154);
    }

    #[test]
    fn r_session_reports_plot_before_later_text_output() {
        if !has_rscript() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("plot.svg");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let mut session = session();
        let raw = session
            .capture(
                r#"m <- lm(mpg ~ wt, data = mtcars)
plot(hp ~ qsec, data = mtcars, col = "red", pch = 19)
summary(m)"#,
                &fig_path,
                "svg",
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        let plot = raw.find("_PLOT:").expect(&raw);
        let summary = raw.find("Residuals:").expect(&raw);
        assert!(plot < summary, "{raw}");
    }

    #[test]
    fn r_session_captures_direct_stderr_as_message() {
        if !has_rscript() {
            return;
        }

        let mut session = session();
        let raw = session
            .capture(
                "cat('stderr text', file = stderr())",
                "",
                "svg",
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        assert!(raw.contains("_MESSAGE:stderr text"), "{raw}");
        assert!(
            raw.contains("_SOURCE:cat('stderr text', file = stderr())"),
            "{raw}"
        );
    }

    #[test]
    fn r_session_accepts_equals_in_figure_path() {
        if !has_rscript() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("plot=equals.svg");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let mut session = session();
        let raw = session
            .capture("plot(1:3)", &fig_path, "svg", 6.0, 3.708, 150.0)
            .unwrap();

        assert!(std::path::Path::new(&fig_path).exists());
        assert!(raw.contains("_PLOT:"), "{raw}");
        assert!(raw.contains(&fig_path), "{raw}");
    }
}
