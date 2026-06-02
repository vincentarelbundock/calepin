use anyhow::Result;

use crate::cli::{set_quiet, CompileArgs, PreprocessArgs};
use crate::typst::preprocess::{
    compile_with_typst, preprocess, PreprocessOptions,
};

pub fn handle_preprocess(args: PreprocessArgs) -> Result<()> {
    set_quiet(args.quiet);
    let cache_override = args.cache_override();
    let execute_override = args.execute_override();
    preprocess(PreprocessOptions {
        input: args.input,
        root: args.root,
        cwd: args.cwd,
        results: args.results,
        cache_override,
        execute_override,
        clean: args.clean,
        quiet: args.quiet,
        typst: args.typst,
        rscript: args.rscript,
        python: args.python,
        shell: args.shell,
        timeout: args.timeout,
    })?;
    Ok(())
}

pub fn handle_compile(args: CompileArgs) -> Result<()> {
    set_quiet(args.common.quiet);
    let typst = args.common.typst.clone();
    let cache_override = args.common.cache_override();
    let execute_override = args.common.execute_override();
    let output = preprocess(PreprocessOptions {
        input: args.input,
        root: args.common.root,
        cwd: args.common.cwd,
        results: args.common.results,
        cache_override,
        execute_override,
        clean: args.common.clean,
        quiet: args.common.quiet,
        typst: args.common.typst,
        rscript: args.common.rscript,
        python: args.common.python,
        shell: args.common.shell,
        timeout: args.common.timeout,
    })?;
    compile_with_typst(&typst, &output.layout, args.output, &args.typst_args)?;
    Ok(())
}
