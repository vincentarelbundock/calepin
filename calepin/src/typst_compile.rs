//! Native Typst → PDF compilation using the typst library.
//!
//! Implements a minimal `World` so we can compile `.typ` files to PDF
//! without shelling out to the external `typst` binary.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::{FontSearcher, Fonts};

/// Compile a `.typ` file to PDF and write the result to `output`.
pub fn compile_typst_to_pdf(input: &Path, output: &Path) -> Result<()> {
    let parent = input.parent().unwrap_or(Path::new("."));
    let parent = if parent.as_os_str().is_empty() { Path::new(".") } else { parent };
    let root = parent
        .canonicalize()
        .with_context(|| format!("Failed to resolve root directory for {}", input.display()))?;

    let file_name = input
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Input path has no file name: {}", input.display()))?;

    let main_id = FileId::new(None, VirtualPath::new(Path::new("/").join(file_name)));

    let world = CalepinWorld::new(root, main_id)?;

    let document: PagedDocument = typst::compile(&world)
        .output
        .map_err(|diags| {
            let messages: Vec<String> = diags
                .iter()
                .map(|d| d.message.to_string())
                .collect();
            anyhow::anyhow!("Typst compilation failed:\n{}", messages.join("\n"))
        })?;

    let pdf_bytes = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|diags| {
            let messages: Vec<String> = diags
                .iter()
                .map(|d| d.message.to_string())
                .collect();
            anyhow::anyhow!("PDF export failed:\n{}", messages.join("\n"))
        })?;

    fs::write(output, &pdf_bytes)
        .with_context(|| format!("Failed to write PDF to {}", output.display()))?;

    Ok(())
}

/// A minimal Typst world for compiling `.typ` files from disk.
struct CalepinWorld {
    /// Root directory for resolving files.
    root: PathBuf,
    /// The main source file ID.
    main_id: FileId,
    /// The standard library.
    library: LazyHash<Library>,
    /// Font metadata.
    book: LazyHash<FontBook>,
    /// Font slots for lazy loading.
    fonts: Vec<typst_kit::fonts::FontSlot>,
    /// Cached source files.
    sources: Mutex<HashMap<FileId, Source>>,
}

impl CalepinWorld {
    fn new(root: PathBuf, main_id: FileId) -> Result<Self> {
        let Fonts { book, fonts } = FontSearcher::new().search();

        Ok(Self {
            root,
            main_id,
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            sources: Mutex::new(HashMap::new()),
        })
    }

    /// Resolve a `FileId` to a real filesystem path.
    fn resolve_path(&self, id: FileId) -> PathBuf {
        self.root.join(id.vpath().as_rootless_path())
    }
}

impl World for CalepinWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let mut cache = self.sources.lock().unwrap();
        if let Some(source) = cache.get(&id) {
            return Ok(source.clone());
        }

        let path = self.resolve_path(id);
        let text = fs::read_to_string(&path).map_err(|e| FileError::from_io(e, &path))?;
        let source = Source::new(id, text);
        cache.insert(id, source.clone());
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.resolve_path(id);
        let data = fs::read(&path).map_err(|e| FileError::from_io(e, &path))?;
        Ok(Bytes::new(data))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index)?.get()
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let now = chrono::Local::now();
        let naive = match offset {
            Some(hours) => {
                let utc = now.naive_utc();
                utc + chrono::Duration::hours(hours)
            }
            None => now.naive_local(),
        };
        Datetime::from_ymd_hms(
            naive.format("%Y").to_string().parse().ok()?,
            naive.format("%m").to_string().parse().ok()?,
            naive.format("%d").to_string().parse().ok()?,
            naive.format("%H").to_string().parse().ok()?,
            naive.format("%M").to_string().parse().ok()?,
            naive.format("%S").to_string().parse().ok()?,
        )
    }
}
