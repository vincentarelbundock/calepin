//! The `calepin info` command: display system capabilities (CSL styles, themes, completions).

use anyhow::Result;
use crate::cli::{Cli, InfoAction};

pub fn handle_info(action: InfoAction) -> Result<()> {
    match action {
        InfoAction::Csl => {
            use hayagriva::archive::ArchivedStyle;

            println!("Calepin uses CSL (Citation Style Language) for bibliography");
            println!("formatting. Over 2,600 styles are available from the Zotero");
            println!("style repository:");
            println!();
            println!("  https://www.zotero.org/styles");
            println!();
            println!("Download a .csl file and set csl: to its path in _calepin.toml");
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
        InfoAction::Themes => {
            println!("Built-in syntax highlighting themes:\n");
            if let Some(dir) = Some(&crate::render::elements::BUILTIN_HIGHLIGHTING) {
                let mut names: Vec<&str> = dir.files()
                    .filter_map(|f| {
                        if f.path().extension()?.to_str()? == "tmTheme" {
                            f.path().file_stem()?.to_str()
                        } else {
                            None
                        }
                    })
                    .collect();
                names.sort();
                for name in &names {
                    println!("  {}", name);
                }
                println!("\n{} themes available.", names.len());
            }
            println!("Custom themes: place a .tmTheme file in _calepin/assets/highlighting/");
            Ok(())
        }
        InfoAction::ThemeList => {
            let root = crate::paths::get_project_root();
            let themes = crate::theme_manifest::list_themes(&root);
            if themes.is_empty() {
                println!("No themes found in _calepin/themes/.");
                println!("Create one with: mkdir -p _calepin/themes/<name> && touch _calepin/themes/<name>/theme.toml");
            } else {
                for theme in &themes {
                    println!("{:<14} {:<10} {}",
                        theme.name,
                        theme.target,
                        theme.description.as_deref().unwrap_or(""),
                    );
                }
            }
            Ok(())
        }
        InfoAction::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = <Cli as CommandFactory>::command();
            clap_complete::generate(shell, &mut cmd, "calepin", &mut std::io::stdout());
            Ok(())
        }
    }
}
