use anyhow::Result;
use indexmap::IndexMap;
use std::path::Path;

use crate::typst::io::write_if_changed;
use crate::typst::model::{ChunkResultDocument, ChunkSpec, ResultsDocument, RESULT_SCHEMA_VERSION};
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
    let json = serde_json::to_string_pretty(document)?;
    let json = format!("{}\n", json);
    write_if_changed(path, json)
}

pub fn refresh_results_metadata(document: &mut ResultsDocument, chunks: &[ChunkSpec]) {
    for chunk in chunks {
        if let Some(result) = document.chunks.get_mut(&chunk.label) {
            result.display_options = chunk.display_options.clone();
            result.crossref_labels = chunk.crossref_labels.clone();
        }
    }
}

pub fn refresh_cached_results_metadata(path: &Path, chunks: &[ChunkSpec]) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let mut document: ResultsDocument = serde_json::from_str(&text)?;
    refresh_results_metadata(&mut document, chunks);
    write_results(path, &document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{ChunkStatus, EngineName, ResultItem, ResultsMode};
    use crate::typst::testfixtures::{chunk, display_options};

    #[test]
    fn builds_results_document_keyed_by_label() {
        let doc = build_results_document(
            Path::new("chapters/intro.typ"),
            vec![ChunkResultDocument {
                label: "setup".to_string(),
                engine: EngineName::Python,
                status: ChunkStatus::Ok,
                display_options: display_options(ResultsMode::Render),
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

    #[test]
    fn refreshes_cached_result_display_options_without_replacing_items() {
        let mut stale_options = display_options(ResultsMode::Render);
        stale_options.fig_width = Some(serde_json::json!("70%"));
        let items = vec![ResultItem {
            text: Some("cached output".to_string()),
            ..ResultItem::default()
        }];
        let mut doc = build_results_document(
            Path::new("paper.typ"),
            vec![ChunkResultDocument {
                label: "fig-demo".to_string(),
                engine: EngineName::Python,
                status: ChunkStatus::Ok,
                display_options: stale_options,
                items: items.clone(),
                crossref_labels: vec![],
            }],
        );
        let mut current = chunk("fig-demo", "print(1)", ResultsMode::Render);
        current.display_options.fig_width = Some(serde_json::json!("10%"));
        current.display_options.echo = false;

        refresh_results_metadata(&mut doc, &[current]);

        let updated = doc.chunks.get("fig-demo").unwrap();
        assert_eq!(
            updated.display_options.fig_width,
            Some(serde_json::json!("10%"))
        );
        assert!(!updated.display_options.echo);
        assert_eq!(updated.items, items);
    }
}
