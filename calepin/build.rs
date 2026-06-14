use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let output = out_dir.join("theme_assets.rs");

    let mut source = String::new();
    write_theme_files(
        &mut source,
        "CALEPIN_FILES",
        &manifest_dir,
        "src/assets/themes/calepin",
    );
    source.push('\n');
    write_theme_files(
        &mut source,
        "ACADEMIC_FILES",
        &manifest_dir,
        "src/assets/themes/academic",
    );

    fs::write(output, source).unwrap();
}

fn write_theme_files(source: &mut String, const_name: &str, manifest_dir: &Path, theme_dir: &str) {
    let root = manifest_dir.join(theme_dir);
    println!("cargo:rerun-if-changed={}", root.display());

    let mut files = Vec::new();
    collect_theme_files(&root, &root, &mut files);
    files.extend(shared_theme_files());
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files.dedup_by(|left, right| left.0 == right.0);

    source.push_str(&format!("static {const_name}: &[BundleFile] = &[\n"));
    for (path, source_path) in files {
        let absolute_source_path = manifest_dir.join(&source_path);
        println!("cargo:rerun-if-changed={}", absolute_source_path.display());
        let content = fs::read_to_string(&absolute_source_path).unwrap();
        let content_hash = {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            hasher.finish()
        };
        source.push_str("    BundleFile {\n");
        source.push_str(&format!("        // content-hash: {content_hash:016x}\n"));
        source.push_str(&format!("        path: {path:?},\n"));
        source.push_str(&format!("        source: {content:?},\n"));
        source.push_str("    },\n");
    }
    source.push_str("];\n");
}

fn collect_theme_files(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) {
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut entries = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_theme_files(root, &path, files);
            continue;
        }
        if !is_theme_asset(&path) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let source_path = path
            .strip_prefix(Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()))
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        files.push((relative, source_path));
    }
}

fn is_theme_asset(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("css" | "html" | "jinja" | "js")
    )
}

fn shared_theme_files() -> Vec<(String, String)> {
    vec![
        (
            "styles/00-theme.css".to_string(),
            "src/assets/snippets/css/theme.css".to_string(),
        ),
        (
            "styles/01-code.css".to_string(),
            "src/assets/snippets/css/code.css".to_string(),
        ),
        (
            "styles/02-widgets.css".to_string(),
            "src/assets/snippets/css/widgets.css".to_string(),
        ),
        (
            "scripts/00-theme-toggle.js".to_string(),
            "src/assets/snippets/js/theme-toggle.js".to_string(),
        ),
        (
            "scripts/01-language-picker.js".to_string(),
            "src/assets/snippets/js/language-picker.js".to_string(),
        ),
        (
            "scripts/02-copy-code.js".to_string(),
            "src/assets/snippets/js/copy-code.js".to_string(),
        ),
    ]
}
