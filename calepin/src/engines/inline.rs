// Inline code expression evaluation.
//
// - evaluate_inline() — Find `{r}`/`{python}` inline expressions in text, evaluate each
//                       through the appropriate engine, and replace them with their results.

use anyhow::Result;

use crate::engines::{self, EngineContext};
use crate::parse::blocks::collect_inline_code;

/// Evaluate inline code expressions in text, replacing `{r}` and `{python}`
/// expressions with their results.
#[inline(never)]
pub fn evaluate_inline(text: &str, ctx: &mut EngineContext) -> Result<String> {
    let inlines = collect_inline_code(text);
    if inlines.is_empty() {
        return Ok(text.to_string());
    }

    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;

    for (start, end, inline) in &inlines {
        result.push_str(&text[last_end..*start]);

        let known = matches!(inline.engine.as_str(), "r" | "python" | "sh");
        match engines::evaluate_inline(&inline.engine, &inline.expr, ctx) {
            Ok(value) => result.push_str(&value),
            Err(e) => {
                if known {
                    cwarn!(
                        "Warning: inline {} expression `{}` failed: {}",
                        inline.engine, inline.expr, e
                    );
                    result.push_str(&format!("`{{{}}} {}`", inline.engine, inline.expr));
                } else {
                    return Err(anyhow::anyhow!(
                        "Unknown inline engine `{}`. Supported engines: r, python, sh",
                        inline.engine
                    ));
                }
            }
        }

        last_end = *end;
    }

    result.push_str(&text[last_end..]);
    Ok(result)
}
