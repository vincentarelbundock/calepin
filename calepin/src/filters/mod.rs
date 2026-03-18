// Filters: transforms on elements and rendered output.
//
// Three filter categories, each with different orchestration:
//
// 1. Element filters (Filter trait) — run during element rendering in the
//    div pipeline (render/div.rs). Enrich template vars or produce final
//    output. Includes: TheoremFilter, CalloutFilter, CodeFilter,
//    FigureFilter, ExternalFilter.
//
// 2. Pre-render passes — run on the element list before rendering.
//    bibliography.rs processes citations and appends a References section.
//
// 3. Post-render passes — run on the final rendered string.
//    crossref.rs resolves @ref-id patterns to links/numbers.
//    shortcodes.rs resolves protected marker content.
//
// Shortcodes transform text by expanding `{{< >}}` directives.
pub mod bibliography;
pub mod callout;
pub mod code;
pub mod crossref;
pub mod external;
pub mod figure;
pub mod highlighting;
pub mod theorem;

pub mod shortcodes;

use std::collections::HashMap;

use crate::types::Element;

pub use callout::CalloutFilter;
pub use external::resolve_external_filter;
pub use theorem::TheoremFilter;

/// Result of applying a filter.
pub enum FilterResult {
    /// Filter produced final rendered output.
    Rendered(String),
    /// Filter enriched the vars map. Proceed with template.
    Continue,
    /// Filter does not handle this element.
    Pass,
}

/// Uniform trait for element filters.
/// All pipeline filters share this interface.
pub trait Filter {
    fn apply(
        &self,
        element: &Element,
        format: &str,
        vars: &mut HashMap<String, String>,
    ) -> FilterResult;
}
