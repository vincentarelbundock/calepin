//! The `calepin extra` command: display system capabilities (CSL styles, themes, completions).

use anyhow::Result;
use crate::cli::ExtraAction;

pub fn handle_extra(action: ExtraAction) -> Result<()> {
    match action {
        ExtraAction::Csl => {
            use hayagriva::archive::ArchivedStyle;

            println!("Calepin uses CSL (Citation Style Language) for bibliography");
            println!("formatting. Over 2,600 styles are available from the Zotero");
            println!("style repository:");
            println!();
            println!("  https://www.zotero.org/styles");
            println!();
            println!("Download a .csl file and set 'csl' to its path in _calepin/config.toml");
            println!("or in document front matter.");
            println!();
            println!("The following shortcuts are also available as built-in names");
            println!("(no download required):");
            println!();

            let mut names: Vec<&str> = ArchivedStyle::all().iter()
                .map(|s| s.names()[0])
                .collect();
            names.sort();

            // Print comma-separated, wrapped at 79 characters
            let joined = names.join(", ");
            let mut line = String::from("  ");
            for word in joined.split(' ') {
                if line.len() + 1 + word.len() > 79 && line.len() > 2 {
                    println!("{}", line);
                    line = format!("  {}", word);
                } else {
                    if line.len() > 2 { line.push(' '); }
                    line.push_str(word);
                }
            }
            if !line.trim().is_empty() {
                println!("{}", line);
            }
            Ok(())
        }
        ExtraAction::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = <crate::cli::Cli as CommandFactory>::command();
            clap_complete::generate(shell, &mut cmd, "calepin", &mut std::io::stdout());
            Ok(())
        }
        ExtraAction::Highlight => {
            println!("Built-in syntax highlighting themes:\n");
            let names = crate::modules::list_builtin_themes();
            for name in &names {
                println!("  {}", name);
            }
            println!("\n{} themes available.", names.len());
            println!("Custom themes: place a .tmTheme file in _calepin/assets/highlighting/");
            Ok(())
        }
        ExtraAction::Partials { target } => {
            show_partial_resolution(&target);
            Ok(())
        }
    }
}

/// Show where each partial is resolved from for a given target.
fn show_partial_resolution(target_name: &str) {
    use crate::render::elements::{BUILTIN_PARTIALS, resolve_builtin_partial};

    // Determine writer from target
    let empty = std::collections::HashMap::new();
    let target = match crate::config::resolve_target(target_name, &empty) {
        Ok(t) => Some(t),
        Err(_) => {
            eprintln!("Note: target '{}' not found, showing writer-level partials", target_name);
            None
        }
    };
    let writer = target.as_ref().map(|t| t.writer.as_str()).unwrap_or(target_name);
    let ext = crate::paths::resolve_extension(writer);

    // Set active target for resolution
    crate::paths::set_active_target(Some(target_name));

    // Collect all known partial names from built-in
    let mut names: Vec<String> = Vec::new();
    if let Some(dir) = BUILTIN_PARTIALS.get_dir(writer) {
        for file in dir.files() {
            if let Some(stem) = file.path().file_stem().and_then(|s| s.to_str()) {
                if !names.contains(&stem.to_string()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    // Also check target-specific partials
    if target_name != writer {
        if let Some(dir) = BUILTIN_PARTIALS.get_dir(target_name) {
            for file in dir.files() {
                if let Some(stem) = file.path().file_stem().and_then(|s| s.to_str()) {
                    if !names.contains(&stem.to_string()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
    }
    names.sort();

    println!("Partial resolution for target '{}' (writer: {}):\n", target_name, writer);

    let project_root = crate::paths::get_project_root();
    let partials_dir = crate::paths::partials_dir(&project_root);

    for name in &names {
        // Try filesystem first
        if let Some(path) = crate::paths::resolve_partial(name, writer) {
            let rel = path.strip_prefix(&project_root).unwrap_or(&path);
            println!("  {}.{}  \x1b[32m{}\x1b[0m", name, ext, rel.display());
        } else if resolve_builtin_partial(name, writer).is_some() {
            println!("  {}.{}  \x1b[36mbuilt-in: {}\x1b[0m", name, ext, writer);
        } else {
            println!("  {}.{}  \x1b[31mnot found\x1b[0m", name, ext);
        }
    }

    // Check if user partials directory exists
    if partials_dir.is_dir() {
        println!("\nUser partials: {}", partials_dir.display());
    } else {
        println!("\nNo user partials directory. All partials come from built-in defaults.");
    }

    let ext_dirs = crate::paths::get_extension_partial_dirs();
    if !ext_dirs.is_empty() {
        println!("Extension partials:");
        for dir in &ext_dirs {
            println!("  {}", dir.display());
        }
    }
}
