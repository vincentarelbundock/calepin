use anyhow::{Context, Result};
use indexmap::IndexMap;
use std::path::Path;

use crate::typst::model::{ChunkResultDocument, ResultsDocument, RESULT_SCHEMA_VERSION};
use crate::typst::paths::slash_path;

pub fn build_results_document(
    input_rel: &Path,
    chunks: Vec<ChunkResultDocument>,
) -> ResultsDocument {
    let mut map = IndexMap::new();
    for chunk in chunks {
        map.insert(chunk.label.clone(), chunk);
    }
    ResultsDocument {
        schema: RESULT_SCHEMA_VERSION,
        calepin_version: env!("CARGO_PKG_VERSION").to_string(),
        input: slash_path(input_rel),
        chunks: map,
    }
}

pub fn write_results(path: &Path, document: &ResultsDocument) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(document)?;
    let json = format!("{}\n", json);
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == json) {
        return Ok(());
    }
    std::fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{ChunkStatus, DisplayOptions, EngineName, ResultsMode};

    fn display_options() -> DisplayOptions {
        DisplayOptions {
            echo: true,
            output: true,
            results: ResultsMode::Render,
            warning: true,
            message: true,
            placeholder: true,
            fig_width: None,
            fig_height: None,
            fig_align: None,
            fig_responsive: None,
            fig_link: None,
            fig_caption: None,
            fig_cap_location: None,
            fig_alt_text: None,
            fig_subcaptions: None,
            fig_layout_columns: None,
            fig_layout_rows: None,
            kind: None,
        }
    }

    #[test]
    fn builds_results_document_keyed_by_label() {
        let doc = build_results_document(
            Path::new("chapters/intro.typ"),
            vec![ChunkResultDocument {
                label: "setup".to_string(),
                engine: EngineName::Python,
                status: ChunkStatus::Ok,
                display_options: display_options(),
                items: Vec::new(),
                crossref_labels: vec![],
            }],
        );

        assert_eq!(doc.schema, 1);
        assert_eq!(doc.input, "chapters/intro.typ");
        assert!(doc.chunks.contains_key("setup"));
    }

    #[test]
    fn writes_pretty_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".calepin/paper/results.json");
        let doc = build_results_document(Path::new("paper.typ"), Vec::new());

        write_results(&path, &doc).unwrap();

        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"schema\": 1"));
        assert!(text.ends_with('\n'));
    }
}
