use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::cli::{is_quiet, CompileArgs};
use crate::typst::model::{ChunkSpec, EngineName};
use crate::typst::preprocess::{
    prepare_preprocess_plan, preprocess_plan_into_chunks, PreprocessOptions,
};

struct ScriptGroup {
    engine: EngineName,
    chunks: Vec<ChunkSpec>,
}

impl ScriptGroup {
    fn extension(&self) -> &'static str {
        match self.engine.as_str() {
            "r" => "R",
            "python" => "py",
            "julia" => "jl",
            "sh" | "bash" => "sh",
            _ => "txt",
        }
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
        fs::write(&output, render_script(&group.chunks))
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
        let engine = if chunk.engine.as_str() == "bash" {
            EngineName::from_name("sh")
        } else {
            chunk.engine.clone()
        };
        if let Some(group) = groups.iter_mut().find(|group| group.engine == engine) {
            group.chunks.push(chunk);
        } else {
            groups.push(ScriptGroup {
                engine,
                chunks: vec![chunk],
            });
        }
    }
    groups
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
    if groups.len() > 1 && !has_placeholder {
        return Err(anyhow!(
            "script output contains multiple languages; add `{{ext}}` or `{{engine}}` to the output path"
        ));
    }

    let mut seen = HashSet::new();
    let mut outputs = Vec::with_capacity(groups.len());
    for group in groups {
        let path = if has_placeholder {
            PathBuf::from(
                template_text
                    .replace("{ext}", group.extension())
                    .replace("{engine}", &group.filename_engine()),
            )
        } else {
            template.to_path_buf()
        };
        if !seen.insert(path.clone()) {
            return Err(anyhow!(
                "script output template maps multiple engines to {}; add `{{engine}}` to distinguish them",
                path.display()
            ));
        }
        outputs.push(path);
    }
    Ok(outputs)
}

fn render_script(chunks: &[ChunkSpec]) -> String {
    let mut output = String::new();
    for chunk in chunks {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("# ---- ");
        output.push_str(&chunk.label);
        output.push_str(" ----\n\n");
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
            engine: EngineName::from_name(engine),
            chunks: Vec::new(),
        }
    }

    #[test]
    fn default_paths_use_conventional_extensions() {
        let groups = vec![group("r"), group("python"), group("julia"), group("sh")];
        let paths = resolve_output_paths(Path::new("reports/paper.typ"), None, &groups).unwrap();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("reports/paper.R"),
                PathBuf::from("reports/paper.py"),
                PathBuf::from("reports/paper.jl"),
                PathBuf::from("reports/paper.sh"),
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
            &[group("ruby"), group("stata")],
        )
        .unwrap_err();
        assert!(error.to_string().contains("{engine}"));
    }
}
