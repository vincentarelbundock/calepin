use std::path::{Path, PathBuf};

use crate::typst::model::{
    ChunkSpec, DisplayOptions, EngineName, ExecOptions, LayoutPaths, ResultsMode,
};

pub fn exec_options() -> ExecOptions {
    ExecOptions {
        eval: true,
        error: false,
        fig_device_format: "svg".to_string(),
        fig_device_dpi: 150,
        fig_device_width: 6.0,
        fig_device_height: None,
        fig_device_aspect: 0.618,
    }
}

pub fn display_options(results: ResultsMode) -> DisplayOptions {
    DisplayOptions {
        echo: true,
        output: true,
        results,
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

pub fn chunk(label: &str, code: &str, results: ResultsMode) -> ChunkSpec {
    ChunkSpec {
        label: label.to_string(),
        engine: EngineName::Python,
        code: code.to_string(),
        exec_options: exec_options(),
        display_options: display_options(results),
        ordinal: 0,
        crossref_labels: vec![],
    }
}

pub fn layout(root: &Path) -> LayoutPaths {
    LayoutPaths {
        root: root.to_path_buf(),
        input: root.join("paper.typ"),
        input_rel: PathBuf::from("paper.typ"),
        render_input: PathBuf::from("paper.typ"),
        work_dir: root.to_path_buf(),
        artifact_dir: root.join(".calepin/paper"),
        results_path: root.join(".calepin/paper/results.json"),
        figures_dir: root.join(".calepin/paper/figures"),
    }
}
