use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::cli::{is_quiet, CompileArgs};
use crate::typst::model::{ChunkSpec, EngineName, ScriptDestination};
use crate::typst::preprocess::{
    prepare_preprocess_plan, preprocess_plan_into_chunks, PreprocessOptions,
};

struct ScriptGroup {
    destination: GroupDestination,
    engine: EngineName,
    chunks: Vec<ChunkSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScriptLanguage {
    extension: &'static str,
    comment: Option<CommentSyntax>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentSyntax {
    Line(&'static str),
    Block {
        open: &'static str,
        close: &'static str,
    },
}

const UNKNOWN_LANGUAGE: ScriptLanguage = ScriptLanguage {
    extension: "txt",
    comment: None,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum GroupDestination {
    Default,
    Explicit(PathBuf),
}

impl ScriptGroup {
    fn extension(&self) -> &'static str {
        script_language_for_engine(self.engine.as_str())
            .unwrap_or(UNKNOWN_LANGUAGE)
            .extension
    }

    fn filename_engine(&self) -> String {
        sanitize_filename_component(self.engine.as_str())
    }
}

pub fn extract_scripts(args: CompileArgs) -> Result<()> {
    let input = args.input.clone();
    let output_template = args.output.clone();
    let plan = prepare_preprocess_plan(PreprocessOptions {
        input: args.input,
        root: None,
        config: args.common.config,
        display_root: None,
        quiet: args.common.quiet,
        status: false,
        progress: false,
        timeout: args.common.timeout,
        sync_pages: false,
        theme: None,
        fallback_theme: crate::theme::ThemeSelection::Default,
        html_syntax_theme: None,
        asset_dir: None,
        config_overrides: args.common.sets,
    })?;
    let groups = group_script_chunks(preprocess_plan_into_chunks(plan));

    if groups.is_empty() {
        if !is_quiet() {
            eprintln!("No executable code chunks found in {}", input.display());
        }
        return Ok(());
    }

    let outputs = resolve_output_paths(&input, output_template.as_deref(), &groups)?;
    for (group, output) in groups.iter().zip(outputs) {
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let language = script_language_for_group(group, &output);
        fs::write(&output, render_script(&group.chunks, language.comment))
            .with_context(|| format!("failed to write {}", output.display()))?;
        if !is_quiet() {
            eprintln!("Created {}", output.display());
        }
    }

    Ok(())
}

fn group_script_chunks(chunks: Vec<ChunkSpec>) -> Vec<ScriptGroup> {
    let mut groups: Vec<ScriptGroup> = Vec::new();
    for chunk in chunks {
        if chunk.engine.is_diagram() {
            continue;
        }
        let destination = match &chunk.script {
            ScriptDestination::Enabled(false) => continue,
            ScriptDestination::Enabled(true) => GroupDestination::Default,
            ScriptDestination::Path(path) => GroupDestination::Explicit(PathBuf::from(path)),
        };
        let engine = if chunk.engine.as_str() == "bash" {
            EngineName::from_name("sh")
        } else {
            chunk.engine.clone()
        };
        if let Some(group) =
            groups
                .iter_mut()
                .find(|group| match (&group.destination, &destination) {
                    (GroupDestination::Default, GroupDestination::Default) => {
                        group.engine == engine
                    }
                    (GroupDestination::Explicit(left), GroupDestination::Explicit(right)) => {
                        left == right
                    }
                    _ => false,
                })
        {
            // Unknown/Jupyter fences can be reported twice by Typst (as a raw
            // block and as Calepin metadata) with consecutive generated labels.
            // Do not duplicate that one source block in the extracted script.
            if group.chunks.last().is_some_and(|existing| {
                consecutive_generated_labels(&existing.label, &chunk.label)
                    && existing.code.trim_end() == chunk.code.trim_end()
                    && existing.script == chunk.script
            }) {
                continue;
            }
            group.chunks.push(chunk);
        } else {
            groups.push(ScriptGroup {
                destination,
                engine,
                chunks: vec![chunk],
            });
        }
    }
    groups
}

fn consecutive_generated_labels(left: &str, right: &str) -> bool {
    let Some(left) = left.strip_prefix("chunk-") else {
        return false;
    };
    let Some(right) = right.strip_prefix("chunk-") else {
        return false;
    };
    left.parse::<usize>()
        .ok()
        .zip(right.parse::<usize>().ok())
        .is_some_and(|(left, right)| left.checked_add(1) == Some(right))
}

fn resolve_output_paths(
    input: &Path,
    template: Option<&Path>,
    groups: &[ScriptGroup],
) -> Result<Vec<PathBuf>> {
    let default_template;
    let template = match template {
        Some(template) => template,
        None => {
            let stem = input
                .file_stem()
                .ok_or_else(|| anyhow!("input path must have a file name: {}", input.display()))?;
            default_template = input.with_file_name(format!("{}.{{ext}}", stem.to_string_lossy()));
            &default_template
        }
    };
    let template_text = template.to_str().ok_or_else(|| {
        anyhow!(
            "script output template must be valid UTF-8: {}",
            template.display()
        )
    })?;
    let has_placeholder = template_text.contains("{ext}") || template_text.contains("{engine}");
    let default_group_count = groups
        .iter()
        .filter(|group| group.destination == GroupDestination::Default)
        .count();
    if default_group_count > 1 && !has_placeholder {
        return Err(anyhow!(
            "script output contains multiple languages; add `{{ext}}` or `{{engine}}` to the output path"
        ));
    }

    let mut seen = HashSet::new();
    let mut outputs = Vec::with_capacity(groups.len());
    for group in groups {
        let path = match &group.destination {
            GroupDestination::Explicit(path) => resolve_explicit_path(input, path)?,
            GroupDestination::Default if has_placeholder => PathBuf::from(
                template_text
                    .replace("{ext}", group.extension())
                    .replace("{engine}", &group.filename_engine()),
            ),
            GroupDestination::Default => template.to_path_buf(),
        };
        if !seen.insert(path.clone()) {
            return Err(anyhow!(
                "multiple script destinations resolve to {}; choose distinct `script` paths or add `{{engine}}` to the output template",
                path.display()
            ));
        }
        outputs.push(path);
    }
    Ok(outputs)
}

fn resolve_explicit_path(input: &Path, path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(anyhow!("`script` path must not be empty"));
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!(
                    "`script` path must be relative and stay within the document directory: {}",
                    path.display()
                ));
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(anyhow!("`script` path must name a file"));
    }
    let parent = input.parent().unwrap_or_else(|| Path::new(""));
    Ok(parent.join(relative))
}

fn script_language_for_group(group: &ScriptGroup, output: &Path) -> ScriptLanguage {
    script_language_for_engine(group.engine.as_str())
        .or_else(|| {
            output
                .extension()
                .and_then(|extension| extension.to_str())
                .and_then(script_language_for_extension)
        })
        .unwrap_or(UNKNOWN_LANGUAGE)
}

fn script_language_for_engine(engine: &str) -> Option<ScriptLanguage> {
    script_language(engine)
}

fn script_language_for_extension(extension: &str) -> Option<ScriptLanguage> {
    let extension = extension.strip_prefix('.').unwrap_or(extension);
    script_language(extension)
}

fn script_language(name: &str) -> Option<ScriptLanguage> {
    let name = name.to_ascii_lowercase();
    let (extension, comment) = match name.as_str() {
        "r" => ("R", Some(CommentSyntax::Line("#"))),
        "python" | "python3" | "py" => ("py", Some(CommentSyntax::Line("#"))),
        "julia" | "jl" => ("jl", Some(CommentSyntax::Line("#"))),
        "sh" | "bash" | "zsh" => ("sh", Some(CommentSyntax::Line("#"))),
        "fish" => ("fish", Some(CommentSyntax::Line("#"))),
        "powershell" | "pwsh" | "ps1" => ("ps1", Some(CommentSyntax::Line("#"))),
        "ruby" | "rb" => ("rb", Some(CommentSyntax::Line("#"))),
        "perl" | "pl" => ("pl", Some(CommentSyntax::Line("#"))),
        "yaml" | "yml" => ("yaml", Some(CommentSyntax::Line("#"))),
        "toml" => ("toml", Some(CommentSyntax::Line("#"))),
        "rust" | "rs" => ("rs", Some(CommentSyntax::Line("//"))),
        "javascript" | "js" | "node" | "nodejs" => ("js", Some(CommentSyntax::Line("//"))),
        "typescript" | "ts" => ("ts", Some(CommentSyntax::Line("//"))),
        "c" => ("c", Some(CommentSyntax::Line("//"))),
        "cpp" | "c++" | "cxx" | "cc" => ("cpp", Some(CommentSyntax::Line("//"))),
        "java" => ("java", Some(CommentSyntax::Line("//"))),
        "go" => ("go", Some(CommentSyntax::Line("//"))),
        "kotlin" | "kt" => ("kt", Some(CommentSyntax::Line("//"))),
        "swift" => ("swift", Some(CommentSyntax::Line("//"))),
        "scala" => ("scala", Some(CommentSyntax::Line("//"))),
        "dart" => ("dart", Some(CommentSyntax::Line("//"))),
        "groovy" => ("groovy", Some(CommentSyntax::Line("//"))),
        "php" => ("php", Some(CommentSyntax::Line("//"))),
        "scss" => ("scss", Some(CommentSyntax::Line("//"))),
        "less" => ("less", Some(CommentSyntax::Line("//"))),
        "sql" => ("sql", Some(CommentSyntax::Line("--"))),
        "lua" => ("lua", Some(CommentSyntax::Line("--"))),
        "matlab" => ("m", Some(CommentSyntax::Line("%"))),
        "octave" => ("m", Some(CommentSyntax::Line("%"))),
        "html" | "htm" => (
            "html",
            Some(CommentSyntax::Block {
                open: "<!--",
                close: "-->",
            }),
        ),
        "xml" => (
            "xml",
            Some(CommentSyntax::Block {
                open: "<!--",
                close: "-->",
            }),
        ),
        "css" => (
            "css",
            Some(CommentSyntax::Block {
                open: "/*",
                close: "*/",
            }),
        ),
        "json" => ("json", None),
        "jsonc" => ("jsonc", Some(CommentSyntax::Line("//"))),
        _ => return None,
    };
    Some(ScriptLanguage { extension, comment })
}

fn render_script(chunks: &[ChunkSpec], comment: Option<CommentSyntax>) -> String {
    let mut output = String::new();
    for chunk in chunks {
        if !output.is_empty() {
            output.push('\n');
        }
        match comment {
            Some(CommentSyntax::Line(prefix)) => {
                output.push_str(prefix);
                output.push_str(" ---- ");
                output.push_str(&chunk.label);
                output.push_str(" ----\n\n");
            }
            Some(CommentSyntax::Block { open, close }) => {
                output.push_str(open);
                output.push_str(" ---- ");
                output.push_str(&chunk.label);
                output.push_str(" ---- ");
                output.push_str(close);
                output.push_str("\n\n");
            }
            None => {}
        }
        output.push_str(chunk.code.trim_end());
        output.push('\n');
    }
    output
}

fn sanitize_filename_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches(['-', '.']);
    if sanitized.is_empty() {
        "kernel".to_string()
    } else {
        sanitized.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(engine: &str) -> ScriptGroup {
        ScriptGroup {
            destination: GroupDestination::Default,
            engine: EngineName::from_name(engine),
            chunks: Vec::new(),
        }
    }

    fn chunk(label: &str, code: &str) -> ChunkSpec {
        crate::typst::testfixtures::chunk(label, code, crate::typst::model::ResultsMode::Render)
    }

    #[test]
    fn default_paths_use_conventional_extensions() {
        let groups = vec![
            group("r"),
            group("python"),
            group("julia"),
            group("sh"),
            group("rust"),
            group("yaml"),
            group("javascript"),
            group("json"),
        ];
        let paths = resolve_output_paths(Path::new("reports/paper.typ"), None, &groups).unwrap();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("reports/paper.R"),
                PathBuf::from("reports/paper.py"),
                PathBuf::from("reports/paper.jl"),
                PathBuf::from("reports/paper.sh"),
                PathBuf::from("reports/paper.rs"),
                PathBuf::from("reports/paper.yaml"),
                PathBuf::from("reports/paper.js"),
                PathBuf::from("reports/paper.json"),
            ]
        );
    }

    #[test]
    fn expands_engine_and_extension_placeholders() {
        let groups = vec![group("python"), group("custom/kernel")];
        let paths = resolve_output_paths(
            Path::new("paper.typ"),
            Some(Path::new("scripts/{engine}.{ext}")),
            &groups,
        )
        .unwrap();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("scripts/python.py"),
                PathBuf::from("scripts/custom-kernel.txt"),
            ]
        );
    }

    #[test]
    fn rejects_ambiguous_output_for_multiple_languages() {
        let error = resolve_output_paths(
            Path::new("paper.typ"),
            Some(Path::new("script.txt")),
            &[group("r"), group("python")],
        )
        .unwrap_err();
        assert!(error.to_string().contains("multiple languages"));
    }

    #[test]
    fn detects_extension_collisions_for_unknown_kernels() {
        let error = resolve_output_paths(
            Path::new("paper.typ"),
            Some(Path::new("paper.{ext}")),
            &[group("stata"), group("sas")],
        )
        .unwrap_err();
        assert!(error.to_string().contains("{engine}"));
    }

    #[test]
    fn explicit_paths_are_relative_to_the_input() {
        let groups = vec![ScriptGroup {
            destination: GroupDestination::Explicit(PathBuf::from("scripts/main.py")),
            engine: EngineName::from_name("python"),
            chunks: Vec::new(),
        }];
        let paths = resolve_output_paths(Path::new("reports/paper.typ"), None, &groups).unwrap();
        assert_eq!(paths, vec![PathBuf::from("reports/scripts/main.py")]);
    }

    #[test]
    fn explicit_paths_cannot_escape_the_document_directory() {
        let groups = vec![ScriptGroup {
            destination: GroupDestination::Explicit(PathBuf::from("../main.py")),
            engine: EngineName::from_name("python"),
            chunks: Vec::new(),
        }];
        let error =
            resolve_output_paths(Path::new("reports/paper.typ"), None, &groups).unwrap_err();
        assert!(error.to_string().contains("stay within"));
    }

    #[test]
    fn renders_language_appropriate_separators() {
        let chunks = vec![chunk("setup", "fn main() {}")];
        assert!(render_script(
            &chunks,
            script_language("rust").and_then(|language| language.comment)
        )
        .starts_with("// ---- setup ----"));
        assert!(render_script(
            &chunks,
            script_language("sql").and_then(|language| language.comment)
        )
        .starts_with("-- ---- setup ----"));
        assert!(render_script(
            &chunks,
            script_language("html").and_then(|language| language.comment)
        )
        .starts_with("<!-- ---- setup ---- -->"));
    }

    #[test]
    fn omits_separators_when_comments_are_not_supported_or_unknown() {
        let chunks = vec![chunk("data", "{\"answer\": 42}")];
        assert_eq!(render_script(&chunks, None), "{\"answer\": 42}\n");
        assert_eq!(script_language_for_engine("unknown-kernel"), None);
    }

    #[test]
    fn explicit_extension_is_a_fallback_after_engine_name() {
        let unknown = ScriptGroup {
            destination: GroupDestination::Explicit(PathBuf::from("main.rs")),
            engine: EngineName::from_name("custom-kernel"),
            chunks: Vec::new(),
        };
        assert_eq!(
            script_language_for_group(&unknown, Path::new("main.rs")),
            script_language("rust").unwrap()
        );

        let python = group("python");
        assert_eq!(
            script_language_for_group(&python, Path::new("data.json")),
            script_language("python").unwrap()
        );
    }
}
