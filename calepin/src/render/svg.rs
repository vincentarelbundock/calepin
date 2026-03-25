use std::path::Path;

use anyhow::{Context, Result};

/// Convert an SVG file to a PDF file alongside it.
/// Returns the path to the generated PDF.
pub fn convert_svg_to_pdf(svg_path: &Path) -> Result<std::path::PathBuf> {
    let pdf_path = svg_path.with_extension("pdf");

    // Skip if PDF already exists and is newer than the SVG
    if pdf_path.exists() {
        if let (Ok(svg_meta), Ok(pdf_meta)) = (
            std::fs::metadata(svg_path),
            std::fs::metadata(&pdf_path),
        ) {
            if let (Ok(svg_time), Ok(pdf_time)) = (svg_meta.modified(), pdf_meta.modified()) {
                if pdf_time >= svg_time {
                    return Ok(pdf_path);
                }
            }
        }
    }

    let svg_data = std::fs::read(svg_path)
        .with_context(|| format!("Failed to read SVG: {}", svg_path.display()))?;

    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(&svg_data, &options)
        .with_context(|| format!("Failed to parse SVG: {}", svg_path.display()))?;

    let pdf_data = svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions::default())
        .map_err(|e| anyhow::anyhow!("Failed to convert SVG to PDF ({}): {:?}", svg_path.display(), e))?;

    std::fs::write(&pdf_path, &pdf_data)
        .with_context(|| format!("Failed to write PDF: {}", pdf_path.display()))?;

    Ok(pdf_path)
}
