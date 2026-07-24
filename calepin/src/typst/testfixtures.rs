use std::path::{Path, PathBuf};

use crate::typst::model::{
    ChunkSpec, DisplayOptions, EngineName, ExecOptions, LayoutPaths, ResultsMode,
    DEFAULT_FIG_DEVICE_ASPECT, DEFAULT_FIG_DEVICE_DPI, DEFAULT_FIG_DEVICE_FORMAT,
    DEFAULT_FIG_DEVICE_HEIGHT, DEFAULT_FIG_DEVICE_WIDTH,
};
use crate::typst::paths::CALEPIN_DIR;

pub fn exec_options() -> ExecOptions {
    ExecOptions {
        eval: true,
        error: false,
        store_get: Vec::new(),
        store_set: Vec::new(),
        fig_device_format: DEFAULT_FIG_DEVICE_FORMAT.to_string(),
        fig_device_dpi: DEFAULT_FIG_DEVICE_DPI,
        fig_device_width: DEFAULT_FIG_DEVICE_WIDTH,
        fig_device_height: DEFAULT_FIG_DEVICE_HEIGHT,
        fig_device_aspect: DEFAULT_FIG_DEVICE_ASPECT,
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
        script: Default::default(),
        exec_options: exec_options(),
        display_options: display_options(results),
        ordinal: 0,
        crossref_labels: vec![],
    }
}

pub fn layout(root: &Path) -> LayoutPaths {
    let artifact_dir = root.join(CALEPIN_DIR).join("paper");
    LayoutPaths {
        root: root.to_path_buf(),
        input: root.join("paper.typ"),
        input_rel: PathBuf::from("paper.typ"),
        render_input: PathBuf::from("paper.typ"),
        work_dir: root.to_path_buf(),
        results_path: artifact_dir.join("results.json"),
        figures_dir: artifact_dir.join("figures"),
        artifact_dir,
    }
}
