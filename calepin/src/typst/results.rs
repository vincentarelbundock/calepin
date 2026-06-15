use anyhow::Result;
use indexmap::IndexMap;
use std::path::Path;

use crate::typst::io::write_if_changed;
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
    let json = serde_json::to_string_pretty(document)?;
    let json = format!("{}\n", json);
    write_if_changed(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{ChunkStatus, EngineName, ResultsMode};
    use crate::typst::testfixtures::display_options;

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
}
