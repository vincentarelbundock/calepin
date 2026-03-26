//! The `calepin preview` command: live-preview documents and collections.

use std::path::PathBuf;
use anyhow::Result;
use crate::cli::PreviewArgs;

pub fn handle_preview(args: PreviewArgs) -> Result<()> {
    // Directory: check for project config inside, otherwise serve statically
    if args.input.is_dir() {
        if let Some(config) = crate::cli::find_project_config(&args.input) {
            let args = PreviewArgs { input: config, ..args };
            return handle_preview(args);
        }
        return crate::collection::serve(&args.input, args.port);
    }
    // Project manifest: build, serve with live-reload, and watch for changes
    if crate::cli::is_collection_config(&args.input) {
        // For non-HTML targets, do a one-shot build and open the output.
        let is_html = {
            let target_name = args.format.as_deref().unwrap_or("html");
            let meta = crate::config::load_project_metadata(&args.input)?;
            let target = crate::config::resolve_target(target_name, &meta.targets)?;
            target.engine == "html"
        };
        if !is_html {
            let output = PathBuf::from("output");
            crate::collection::build_collection(Some(args.input.as_path()), &output, true, false, args.format.as_deref())?;
            let pdf = output.join("book.pdf");
            if pdf.exists() {
                eprintln!("Opening {}", pdf.display());
                let _ = open::that(&pdf);
            }
            return Ok(());
        }

        return crate::preview::run_collection(&args.input, &args);
    }
    // Resolve target using the same path as render
    let ctx = crate::resolve_context(&args.input, args.format.as_deref())?;
    crate::preview::run(&args.input, &args, &ctx.target_name, &ctx.target)
}
