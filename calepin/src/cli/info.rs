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
    }
}
