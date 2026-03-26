//! MiniJinja-based body processing.
//!
//! - `expand_includes()` -- Pre-parse `{% include "file" %}` expansion.
//! - `process_body()`    -- Main entry: Jinja-render a text block (code-block-safe).

mod includes;
pub mod lipsum;
mod protection;
mod variables;

pub use includes::expand_includes;
pub use lipsum::{lipsum_words, lipsum_sentence, lipsum_paragraphs};

use std::sync::LazyLock;

use regex::Regex;

use crate::registry::ModuleRegistry;
use crate::config::Metadata;

use protection::{protect_code_blocks, protect_inline_code, restore_code_blocks};

/// Result of Jinja body processing.
pub struct BodyResult {
    pub text: String,
}

/// Process a text block through MiniJinja, evaluating functions and variable references.
#[inline(never)]
pub fn process_body(
    text: &str,
    format: &str,
    metadata: &Metadata,
    _registry: &ModuleRegistry,
) -> BodyResult {
    // 1. Protect fenced code blocks and inline code from Jinja
    let (protected, mut code_blocks) = protect_code_blocks(text);
    let protected = protect_inline_code(&protected, &mut code_blocks);

    // 1b. Escape heading attribute syntax {#id .class} which Jinja
    //     interprets as comment openers ({# ... #}).
    static RE_HEADING_ATTR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{(#[a-zA-Z][a-zA-Z0-9_-]*(?:\s+\.[a-zA-Z][a-zA-Z0-9_-]*)*)\}").unwrap()
    });
    let protected = RE_HEADING_ATTR.replace_all(&protected, "\u{FDD2}$1\u{FDD3}").to_string();

    // Quick exit: if no Jinja syntax found, skip processing
    if !protected.contains("{{") && !protected.contains("{%") {
        return BodyResult {
            text: text.to_string(),
        };
    }

    // 2. Build MiniJinja environment
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);

    // 3. Build context with metadata, variables, and environment
    let context = variables::build_context(metadata, format);

    // 4. Render through MiniJinja (on error, fall back to protected text so that
    //    restore_code_blocks can still recover code block placeholders)
    let rendered = match env.render_str(&protected, &context) {
        Ok(r) => r,
        Err(e) => {
            cwarn!("body template error: {}", e);
            protected.clone()
        }
    };

    // 5. Restore protected content
    let rendered = rendered.replace('\u{FDD2}', "{").replace('\u{FDD3}', "}");
    let text = restore_code_blocks(&rendered, &code_blocks);

    BodyResult { text }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protect_restore_fenced_code() {
        let text = "before\n```python\nx = {{ hello }}\n```\nafter";
        let (protected, blocks) = protect_code_blocks(text);
        assert!(!protected.contains("{{ hello }}"));
        assert_eq!(blocks.len(), 1);
        let restored = restore_code_blocks(&protected, &blocks);
        assert_eq!(restored, text);
    }

    #[test]
    fn test_inline_code_protected() {
        let mut meta = Metadata::default();
        meta.title = Some("T".to_string());
        let registry = ModuleRegistry::empty();
        let result = process_body("version: `{{ meta.title }}`", "html", &meta, &registry);
        assert!(result.text.contains("`{{ meta.title }}`"));
    }

    #[test]
    fn test_no_jinja_syntax_passthrough() {
        let text = "plain text with no template syntax";
        let meta = Metadata::default();
        let registry = ModuleRegistry::empty();
        let result = process_body(text, "html", &meta, &registry);
        assert_eq!(result.text, text);
    }

    #[test]
    fn test_meta_variable_access() {
        let mut meta = Metadata::default();
        meta.title = Some("My Title".to_string());
        let registry = ModuleRegistry::empty();
        let result = process_body("Title: {{ meta.title }}", "html", &meta, &registry);
        assert_eq!(result.text, "Title: My Title");
    }

    #[test]
    fn test_env_context_variable() {
        std::env::set_var("CALEPIN_TEST_VAR", "hello_jinja");
        let meta = Metadata::default();
        let registry = ModuleRegistry::empty();
        let result = process_body("{{ env.CALEPIN_TEST_VAR }}", "html", &meta, &registry);
        assert_eq!(result.text, "hello_jinja");
        std::env::remove_var("CALEPIN_TEST_VAR");
    }

    #[test]
    fn test_code_blocks_preserved() {
        let text = "before {{ meta.title }}\n```\n{{ not_a_var }}\n```\nafter";
        let mut meta = Metadata::default();
        meta.title = Some("T".to_string());
        let registry = ModuleRegistry::empty();
        let result = process_body(text, "html", &meta, &registry);
        assert!(result.text.contains("before T"));
        assert!(result.text.contains("{{ not_a_var }}"));
    }
}
