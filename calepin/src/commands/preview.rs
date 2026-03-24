//! The `calepin preview` command: live-preview documents and collections.

use std::path::PathBuf;
use anyhow::Result;
use crate::cli::PreviewArgs;

pub fn handle_preview(args: PreviewArgs) -> Result<()> {
    // Directory: serve it over HTTP
    if args.input.is_dir() {
        return crate::collection::serve(&args.input, args.port);
    }
    // Project manifest: build, serve with live-reload, and watch for changes
    if crate::cli::is_collection_config(&args.input) {
        // For non-HTML targets, do a one-shot build and open the output.
        let is_html = {
            let target_name = args.target.as_deref().unwrap_or("html");
            let config = crate::project::load_project_config(&args.input)?;
            let target = crate::project::resolve_target(target_name, Some(&config))?;
            target.base == "html"
        };
        if !is_html {
            let output = PathBuf::from("output");
            crate::collection::build_collection(Some(args.input.as_path()), &output, true, false, args.target.as_deref())?;
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
    let ctx = crate::resolve_context(&args.input, args.target.as_deref())?;
    crate::preview::run(&args.input, &args, &ctx.target_name, &ctx.target)
}
