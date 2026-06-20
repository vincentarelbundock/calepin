use anyhow::{anyhow, Context, Result};
use indexmap::IndexMap;
use std::path::Path;

use crate::typst::io::write_if_changed;
use crate::typst::model::{ChunkResultDocument, ChunkSpec, ResultsDocument, RESULT_SCHEMA_VERSION};
use crate::typst::paths::slash_path;

pub fn build_results_document(
    input_rel: &Path,
    chunks: Vec<ChunkResultDocument>,
) -> Result<ResultsDocument> {
    let mut map = IndexMap::with_capacity(chunks.len());
    for chunk in chunks {
        let label = chunk.label.clone();
        if map.insert(label.clone(), chunk).is_some() {
            return Err(anyhow!(
                "duplicate chunk label `{label}` in results document"
            ));
        }
    }
    Ok(ResultsDocument {
        schema: RESULT_SCHEMA_VERSION,
        calepin_version: env!("CARGO_PKG_VERSION").to_string(),
        input: slash_path(input_rel),
        chunks: map,
    })
}

pub fn write_results(path: &Path, document: &ResultsDocument) -> Result<()> {
    let json = serde_json::to_string_pretty(document)?;
    let json = format!("{}\n", json);
    write_if_changed(path, json)
}

pub fn refresh_results_metadata(
    document: &mut ResultsDocument,
    chunks: &[ChunkSpec],
) -> Result<()> {
    let mut refreshed = IndexMap::with_capacity(chunks.len());
    for chunk in chunks {
        if refreshed.contains_key(&chunk.label) {
            return Err(anyhow!(
                "duplicate chunk label `{}` while refreshing cached results",
                chunk.label
            ));
        }
        let mut result = document
            .chunks
            .get(&chunk.label)
            .cloned()
            .ok_or_else(|| anyhow!("missing cached result for chunk `{}`", chunk.label))?;
        result.label = chunk.label.clone();
        result.engine = chunk.engine.clone();
        result.display_options = chunk.display_options.clone();
        result.crossref_labels = chunk.crossref_labels.clone();
        refreshed.insert(chunk.label.clone(), result);
    }

    if let Some(stale_label) = document
        .chunks
        .keys()
        .find(|label| !refreshed.contains_key(*label))
    {
        return Err(anyhow!("stale cached result for chunk `{stale_label}`"));
    }

    document.chunks = refreshed;
    Ok(())
}

pub fn refresh_cached_results_metadata(path: &Path, chunks: &[ChunkSpec]) -> Result<()> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read cached results {}", path.display()))?;
    let mut document: ResultsDocument = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse cached results {}", path.display()))?;
    refresh_results_metadata(&mut document, chunks)?;
    write_results(path, &document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{ChunkStatus, CrossrefLabelDoc, EngineName, ResultItem, ResultsMode};
    use crate::typst::testfixtures::{chunk, display_options};

    fn result(label: &str) -> ChunkResultDocument {
        ChunkResultDocument {
            label: label.to_string(),
            engine: EngineName::Python,
            status: ChunkStatus::Ok,
            display_options: display_options(ResultsMode::Render),
            items: Vec::new(),
            crossref_labels: vec![],
        }
    }

    #[test]
    fn builds_results_document_keyed_by_label() {
        let doc =
            build_results_document(Path::new("chapters/intro.typ"), vec![result("setup")]).unwrap();

        assert_eq!(doc.schema, 1);
        assert_eq!(doc.input, "chapters/intro.typ");
        assert!(doc.chunks.contains_key("setup"));
    }

    #[test]
    fn build_results_document_rejects_duplicate_labels() {
        let err =
            build_results_document(Path::new("paper.typ"), vec![result("dup"), result("dup")])
                .unwrap_err()
                .to_string();

        assert!(err.contains("duplicate chunk label `dup`"), "{err}");
    }

    #[test]
    fn writes_pretty_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".calepin/paper/results.json");
        let doc = build_results_document(Path::new("paper.typ"), Vec::new()).unwrap();

        write_results(&path, &doc).unwrap();

        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"schema\": 1"));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn refreshes_cached_result_metadata_without_replacing_execution_output() {
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
                engine: EngineName::R,
                status: ChunkStatus::Error,
                display_options: stale_options,
                items: items.clone(),
                crossref_labels: vec![],
            }],
        )
        .unwrap();
        let mut current = chunk("fig-demo", "print(1)", ResultsMode::Render);
        current.display_options.fig_width = Some(serde_json::json!("10%"));
        current.display_options.echo = false;
        current.crossref_labels = vec![CrossrefLabelDoc {
            kind: "fig".to_string(),
            name: "fig-demo".to_string(),
        }];

        refresh_results_metadata(&mut doc, &[current]).unwrap();

        let updated = doc.chunks.get("fig-demo").unwrap();
        assert_eq!(updated.label, "fig-demo");
        assert_eq!(updated.engine, EngineName::Python);
        assert_eq!(updated.status, ChunkStatus::Error);
        assert_eq!(
            updated.display_options.fig_width,
            Some(serde_json::json!("10%"))
        );
        assert!(!updated.display_options.echo);
        assert_eq!(
            updated.crossref_labels,
            vec![CrossrefLabelDoc {
                kind: "fig".to_string(),
                name: "fig-demo".to_string(),
            }]
        );
        assert_eq!(updated.items, items);
    }

    #[test]
    fn refresh_cached_result_metadata_rejects_missing_current_chunks() {
        let mut doc =
            build_results_document(Path::new("paper.typ"), vec![result("cached")]).unwrap();
        let current = chunk("missing", "print(1)", ResultsMode::Render);

        let err = refresh_results_metadata(&mut doc, &[current])
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("missing cached result for chunk `missing`"),
            "{err}"
        );
    }

    #[test]
    fn refresh_cached_result_metadata_rejects_stale_cached_chunks() {
        let mut doc = build_results_document(
            Path::new("paper.typ"),
            vec![result("current"), result("stale")],
        )
        .unwrap();
        let current = chunk("current", "print(1)", ResultsMode::Render);

        let err = refresh_results_metadata(&mut doc, &[current])
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("stale cached result for chunk `stale`"),
            "{err}"
        );
    }

    #[test]
    fn refresh_cached_results_metadata_names_malformed_results_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".calepin/paper/results.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{not json").unwrap();

        let err = refresh_cached_results_metadata(&path, &[])
            .unwrap_err()
            .to_string();

        assert!(err.contains("failed to parse cached results"), "{err}");
        assert!(err.contains("results.json"), "{err}");
    }
}
