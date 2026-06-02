// Julia engine session via a persistent julia subprocess.
//
// A single Julia process runs for the lifetime of the document render. The
// bootstrap script reads sentinel-delimited code blocks from stdin, executes
// them in Main, and writes sentinel-tagged results to stdout.

use anyhow::Result;

use super::make_sentinel;
use super::subprocess::{spawn_script, SubprocessSession};

const JULIA_BOOTSTRAP: &str = r#"
module Calepin
    const fig_path = Ref{String}("")
    const fig_format = Ref{String}("")
    const fig_width = Ref{Float64}(0.0)
    const fig_height = Ref{Float64}(0.0)
    const fig_dpi = Ref{Float64}(0.0)
end

function _calepin_parse_meta(line::AbstractString)
    meta = Dict{String,String}()
    startswith(line, "META:") || return meta
    for item in split(line[6:end], ';')
        isempty(item) && continue
        kv = split(item, '=', limit=2)
        length(kv) == 2 && (meta[kv[1]] = kv[2])
    end
    return meta
end

function _calepin_rstrip_newlines(text::String)
    while endswith(text, "\n") || endswith(text, "\r")
        text = chop(text)
    end
    return text
end

function _calepin_push!(parts::Vector{String}, sentinel::String, tag::String, text)
    value = string(text)
    isempty(value) && return
    push!(parts, string(sentinel, "_", tag, ":", value))
end

function _calepin_emit_parts(sentinel::String, parts::Vector{String})
    isempty(parts) && return
    sep = string("\n", sentinel, "_SEP\n")
    print(join(parts, sep), "\n")
end

function _calepin_file_has_plot(path::String)
    !isempty(path) && isfile(path) && stat(path).size > 0
end

function _calepin_set_meta!(meta::Dict{String,String})
    Calepin.fig_path[] = get(meta, "fig_path", "")
    Calepin.fig_format[] = get(meta, "dev", "")
    Calepin.fig_width[] = parse(Float64, get(meta, "width", "0"))
    Calepin.fig_height[] = parse(Float64, get(meta, "height", "0"))
    Calepin.fig_dpi[] = parse(Float64, get(meta, "dpi", "0"))
end

function _calepin_exprs(code::String)
    parsed = Meta.parse("begin\n" * code * "\nend")
    if parsed isa Expr && parsed.head == :block
        return Any[arg for arg in parsed.args if !(arg isa LineNumberNode)]
    end
    return Any[parsed]
end

function _calepin_quiet_expr(expr)
    expr isa Expr || return false
    expr.head in (:(=), :function, :macro, :struct, :module, :using, :import, :global, :local, :const)
end

function _calepin_show_value(value)
    try
        return _calepin_rstrip_newlines(sprint(show, MIME("text/plain"), value))
    catch
        return _calepin_rstrip_newlines(sprint(show, value))
    end
end

function _calepin_try_save_plot(value, fig_path::String)
    isempty(fig_path) && return false
    if value !== nothing
        if isdefined(Main, :savefig)
            try
                Main.savefig(value, fig_path)
                _calepin_file_has_plot(fig_path) && return true
            catch
            end
        end
        if isdefined(Main, :save)
            try
                Main.save(fig_path, value)
                _calepin_file_has_plot(fig_path) && return true
            catch
            end
        end
    end
    return _calepin_file_has_plot(fig_path)
end

function _calepin_try_save_current_plot(fig_path::String)
    isempty(fig_path) && return false
    if isdefined(Main, :savefig)
        try
            Main.savefig(fig_path)
            _calepin_file_has_plot(fig_path) && return true
        catch
        end
    end
    return _calepin_file_has_plot(fig_path)
end

function _calepin_capture_expr(expr)
    out_path, out_handle = mktemp()
    err_path, err_handle = mktemp()
    close(out_handle)
    close(err_handle)
    err = nothing
    value = Ref{Any}(nothing)
    has_value = Ref(false)

    try
        open(out_path, "w") do out_file
            open(err_path, "w") do err_file
                redirect_stdout(out_file) do
                    redirect_stderr(err_file) do
                        value[] = Main.eval(expr)
                        has_value[] = true
                    end
                end
            end
        end
    catch e
        err = sprint(showerror, e, catch_backtrace())
    end

    output = isfile(out_path) ? _calepin_rstrip_newlines(read(out_path, String)) : ""
    diagnostics = isfile(err_path) ? _calepin_rstrip_newlines(read(err_path, String)) : ""
    rm(out_path, force = true)
    rm(err_path, force = true)

    return output, diagnostics, err, value[], has_value[]
end

function _calepin_run_block(sentinel::String, meta_line::String, code::String)
    meta = _calepin_parse_meta(meta_line)
    _calepin_set_meta!(meta)
    fig_path = Calepin.fig_path[]
    if !isempty(fig_path)
        rm(fig_path, force = true)
    end
    parts = String[]
    plot_saved = false
    exprs = Any[]

    try
        exprs = _calepin_exprs(code)
    catch e
        _calepin_push!(parts, sentinel, "ERROR", sprint(showerror, e, catch_backtrace()))
        _calepin_emit_parts(sentinel, parts)
        return
    end

    for expr in exprs
        output, diagnostics, err, value, has_value = _calepin_capture_expr(expr)
        _calepin_push!(parts, sentinel, "OUTPUT", output)
        _calepin_push!(parts, sentinel, "WARNING", diagnostics)

        if err !== nothing
            _calepin_push!(parts, sentinel, "ERROR", err)
            break
        end

        saved_value = has_value && _calepin_try_save_plot(value, fig_path)
        plot_saved = plot_saved || saved_value
        if has_value && !saved_value && value !== nothing && !_calepin_quiet_expr(expr)
            _calepin_push!(parts, sentinel, "OUTPUT", _calepin_show_value(value))
        end
    end

    plot_saved = plot_saved || _calepin_try_save_current_plot(fig_path)
    if plot_saved
        _calepin_push!(parts, sentinel, "PLOT", fig_path)
    end
    _calepin_emit_parts(sentinel, parts)
end

function _calepin_loop()
    while !eof(stdin)
        header = readline(stdin)
        isempty(header) && continue
        endswith(header, "_BEGIN") || continue

        sentinel = replace(header, r"_BEGIN$" => "")
        end_marker = sentinel * "_END"
        lines = String[]

        while !eof(stdin)
            line = readline(stdin)
            line == end_marker && break
            push!(lines, line)
        end

        meta_line = isempty(lines) ? "" : lines[1]
        code = length(lines) <= 1 ? "" : join(lines[2:end], "\n")

        _calepin_run_block(sentinel, meta_line, code)
        println(sentinel, "_DONE")
        flush(stdout)
    end
end

_calepin_loop()
"#;

pub struct JuliaSession {
    proc: SubprocessSession,
    _bootstrap_file: tempfile::NamedTempFile,
}

impl JuliaSession {
    pub fn init_with_program(
        program: &str,
        cwd: Option<&std::path::Path>,
        timeout: Option<std::time::Duration>,
    ) -> Result<Self> {
        let (proc, bootstrap_file) = spawn_script(
            program,
            &["--startup-file=no", "--history-file=no", "--quiet"],
            JULIA_BOOTSTRAP,
            "Julia",
            cwd,
            timeout,
        )?;
        Ok(Self {
            proc,
            _bootstrap_file: bootstrap_file,
        })
    }

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

    fn has_julia() -> bool {
        Command::new("julia")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn julia_session_captures_stdout_and_persistent_state() {
        if !has_julia() {
            return;
        }

        let mut session =
            JuliaSession::init_with_program("julia", None, Some(Duration::from_secs(15))).unwrap();

        let first = session
            .capture("x = 40\nprintln(x + 2)", "", "svg", 6.0, 3.708, 150.0)
            .unwrap();
        assert!(first.contains("_OUTPUT:42"), "{first}");

        let second = session
            .capture("println(x + 3)", "", "svg", 6.0, 3.708, 150.0)
            .unwrap();
        assert!(second.contains("_OUTPUT:43"), "{second}");
    }

    #[test]
    fn julia_session_reports_errors() {
        if !has_julia() {
            return;
        }

        let mut session =
            JuliaSession::init_with_program("julia", None, Some(Duration::from_secs(15))).unwrap();
        let raw = session
            .capture("error(\"boom\")", "", "svg", 6.0, 3.708, 150.0)
            .unwrap();
        assert!(raw.contains("_ERROR:"), "{raw}");
        assert!(raw.contains("boom"), "{raw}");
    }

    #[test]
    fn julia_session_separates_stdout_and_error_parts() {
        if !has_julia() {
            return;
        }

        let mut session =
            JuliaSession::init_with_program("julia", None, Some(Duration::from_secs(15))).unwrap();
        let raw = session
            .capture(
                "println(\"before\")\nerror(\"boom\")",
                "",
                "svg",
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        assert!(raw.contains("_OUTPUT:before"), "{raw}");
        assert!(raw.contains("_SEP"), "{raw}");
        assert!(raw.contains("_ERROR:"), "{raw}");
        assert!(raw.contains("boom"), "{raw}");
    }

    #[test]
    fn julia_session_captures_visible_expression_values() {
        if !has_julia() {
            return;
        }

        let mut session =
            JuliaSession::init_with_program("julia", None, Some(Duration::from_secs(15))).unwrap();
        let raw = session
            .capture("x = 40\nx + 2", "", "svg", 6.0, 3.708, 150.0)
            .unwrap();

        assert!(raw.contains("_OUTPUT:42"), "{raw}");
        assert!(!raw.contains("_OUTPUT:40"), "{raw}");
    }

    #[test]
    fn julia_session_reports_plot_artifacts() {
        if !has_julia() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("julia-plot.svg");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let mut session =
            JuliaSession::init_with_program("julia", None, Some(Duration::from_secs(15))).unwrap();
        let raw = session
            .capture(
                r#"open(Calepin.fig_path[], "w") do io
    write(io, """<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0L1 1"/></svg>""")
end"#,
                &fig_path,
                "svg",
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        assert!(std::path::Path::new(&fig_path).exists());
        assert!(raw.contains("_PLOT:"), "{raw}");
        assert!(raw.contains(&fig_path), "{raw}");
    }
}
