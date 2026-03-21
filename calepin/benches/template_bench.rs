//! Microbenchmark: minijinja apply_template vs plain string replacement
//!
//! Run: cargo bench --bench template_bench

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ── Templates (copied from src/templates/elements/) ─────────────────────

const CODE_OUTPUT_FULL: &str = r#"{%- if format == "html" %}
<pre><code class="output">{{output}}</code></pre>
{%- elif format == "latex" %}
\begin{outcode}
\begin{verbatim}
{{output}}
\end{verbatim}
\end{outcode}
{%- elif format == "typst" %}
#outbox[#raw("{{output}}", block: true)]
{%- else %}
```
{{output}}
```
{%- endif %}"#;

const CODE_SOURCE_FULL: &str = r#"{%- if format == "html" %}
<pre><code class="language-{{lang}} code">{{highlighted}}</code></pre>
{%- elif format == "latex" %}
\begin{srccode}
\begin{Verbatim}[commandchars=\\\{\}]
{{highlighted}}
\end{Verbatim}
\end{srccode}
{%- elif format == "typst" %}
#srcbox[#raw("{{code}}", block: true, lang: "{{lang}}")]
{%- else %}
``` {{lang}}
{{code}}
```
{%- endif %}"#;

const DIV_FULL: &str = r#"{%- if format == "html" %}
<div class="{{classes}}"{{id_attr}}>
{{children}}
</div>
{%- elif format == "latex" %}
\begin{quote}
{{children}}
\end{quote}
{%- elif format == "typst" %}
#block(inset: 1em)[
{{children}}
]{{label}}
{%- else %}
> {{children}}
{%- endif %}"#;

const FIGURE_FULL: &str = r#"{%- if format == "html" %}
<div class="figure" id="fig-{{label}}" style="{{align_style}}">
{{image}}
<p class="caption">{{caption}}</p>
</div>
{%- elif format == "latex" %}
{{fig_begin}}{{fig_pos}}
{{align_style}}
{{image}}
{{caption_cmd}}
\label{fig-{{label}}}
{{fig_end}}
{%- elif format == "typst" %}
#figure({{image}}, caption: [{{caption}}]) <fig-{{label}}>
{%- else %}
![{{alt}}]({{path}})
{%- endif %}"#;

// Pre-resolved HTML-only fragments (what approach 2 would produce)
const CODE_OUTPUT_HTML: &str = r#"<pre><code class="output">{{output}}</code></pre>"#;
const CODE_SOURCE_HTML: &str =
    r#"<pre><code class="language-{{lang}} code">{{highlighted}}</code></pre>"#;
const DIV_HTML: &str = r#"<div class="{{classes}}"{{id_attr}}>
{{children}}
</div>"#;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Current apply_template: creates env + parses + renders every call
fn apply_template_current(template: &str, vars: &HashMap<String, String>) -> String {
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    env.add_template("__tpl__", template).unwrap();
    let mut ctx = std::collections::BTreeMap::new();
    for (key, value) in vars {
        ctx.insert(key.as_str(), minijinja::Value::from(value.as_str()));
    }
    ctx.insert("_lb", minijinja::Value::from("{"));
    ctx.insert("_rb", minijinja::Value::from("}"));
    let tpl = env.get_template("__tpl__").unwrap();
    tpl.render(minijinja::Value::from_serialize(&ctx)).unwrap()
}

/// Approach 1: pre-compiled environment, render only
struct PrecompiledEnv {
    env: minijinja::Environment<'static>,
}

impl PrecompiledEnv {
    fn new(templates: &[(&'static str, &'static str)]) -> Self {
        let mut env = minijinja::Environment::new();
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
        for (name, src) in templates {
            env.add_template(name, src).unwrap();
        }
        Self { env }
    }

    fn render(&self, name: &str, vars: &HashMap<String, String>) -> String {
        let mut ctx = std::collections::BTreeMap::new();
        for (key, value) in vars {
            ctx.insert(key.as_str(), minijinja::Value::from(value.as_str()));
        }
        ctx.insert("_lb", minijinja::Value::from("{"));
        ctx.insert("_rb", minijinja::Value::from("}"));
        let tpl = self.env.get_template(name).unwrap();
        tpl.render(minijinja::Value::from_serialize(&ctx)).unwrap()
    }
}

/// Approach 2: plain string replacement (no jinja at all)
fn apply_plain_replace(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        let pattern = format!("{{{{{}}}}}", key); // {{key}}
        result = result.replace(&pattern, value);
    }
    result
}

// ── Benchmark runner ────────────────────────────────────────────────────

fn bench(name: &str, iterations: u32, f: impl Fn()) -> Duration {
    // Warmup
    for _ in 0..100 {
        f();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let per_call = elapsed / iterations;
    println!(
        "  {:<45} {:>8.1}µs/call  ({} iters in {:.1}ms)",
        name,
        per_call.as_nanos() as f64 / 1000.0,
        iterations,
        elapsed.as_secs_f64() * 1000.0,
    );
    elapsed
}

fn main() {
    let n = 10_000;

    // ── code_output ─────────────────────────────────────────────────
    println!("\n=== code_output (HTML) ===");

    let mut vars = HashMap::new();
    vars.insert("format".to_string(), "html".to_string());
    vars.insert(
        "output".to_string(),
        "Hello world\nLine 2\nLine 3".to_string(),
    );

    let vars_c = vars.clone();
    bench("current (full jinja, full template)", n, || {
        std::hint::black_box(apply_template_current(CODE_OUTPUT_FULL, &vars_c));
    });

    let vars_c = vars.clone();
    bench("current (full jinja, pre-resolved HTML)", n, || {
        std::hint::black_box(apply_template_current(CODE_OUTPUT_HTML, &vars_c));
    });

    let env = PrecompiledEnv::new(&[
        ("code_output_full", CODE_OUTPUT_FULL),
        ("code_output_html", CODE_OUTPUT_HTML),
    ]);
    let vars_c = vars.clone();
    bench("approach 1 (precompiled, full template)", n, || {
        std::hint::black_box(env.render("code_output_full", &vars_c));
    });

    let vars_c = vars.clone();
    bench("approach 1 (precompiled, pre-resolved HTML)", n, || {
        std::hint::black_box(env.render("code_output_html", &vars_c));
    });

    let vars_c = vars.clone();
    bench("approach 2 (string replace, pre-resolved HTML)", n, || {
        std::hint::black_box(apply_plain_replace(CODE_OUTPUT_HTML, &vars_c));
    });

    // ── code_source ─────────────────────────────────────────────────
    println!("\n=== code_source (HTML) ===");

    let mut vars = HashMap::new();
    vars.insert("format".to_string(), "html".to_string());
    vars.insert("lang".to_string(), "r".to_string());
    vars.insert(
        "highlighted".to_string(),
        "<span class=\"kw\">library</span>(ggplot2)".to_string(),
    );
    vars.insert(
        "code".to_string(),
        "library(ggplot2)".to_string(),
    );

    let vars_c = vars.clone();
    bench("current (full jinja, full template)", n, || {
        std::hint::black_box(apply_template_current(CODE_SOURCE_FULL, &vars_c));
    });

    let vars_c = vars.clone();
    bench("current (full jinja, pre-resolved HTML)", n, || {
        std::hint::black_box(apply_template_current(CODE_SOURCE_HTML, &vars_c));
    });

    let env = PrecompiledEnv::new(&[
        ("code_source_full", CODE_SOURCE_FULL),
        ("code_source_html", CODE_SOURCE_HTML),
    ]);
    let vars_c = vars.clone();
    bench("approach 1 (precompiled, full template)", n, || {
        std::hint::black_box(env.render("code_source_full", &vars_c));
    });

    let vars_c = vars.clone();
    bench("approach 1 (precompiled, pre-resolved HTML)", n, || {
        std::hint::black_box(env.render("code_source_html", &vars_c));
    });

    let vars_c = vars.clone();
    bench("approach 2 (string replace, pre-resolved HTML)", n, || {
        std::hint::black_box(apply_plain_replace(CODE_SOURCE_HTML, &vars_c));
    });

    // ── div ─────────────────────────────────────────────────────────
    println!("\n=== div (HTML) ===");

    let mut vars = HashMap::new();
    vars.insert("format".to_string(), "html".to_string());
    vars.insert("classes".to_string(), "note warning".to_string());
    vars.insert("id_attr".to_string(), " id=\"my-div\"".to_string());
    vars.insert(
        "children".to_string(),
        "<p>Some content here</p>".to_string(),
    );

    let vars_c = vars.clone();
    bench("current (full jinja, full template)", n, || {
        std::hint::black_box(apply_template_current(DIV_FULL, &vars_c));
    });

    let vars_c = vars.clone();
    bench("current (full jinja, pre-resolved HTML)", n, || {
        std::hint::black_box(apply_template_current(DIV_HTML, &vars_c));
    });

    let env = PrecompiledEnv::new(&[
        ("div_full", DIV_FULL),
        ("div_html", DIV_HTML),
    ]);
    let vars_c = vars.clone();
    bench("approach 1 (precompiled, full template)", n, || {
        std::hint::black_box(env.render("div_full", &vars_c));
    });

    let vars_c = vars.clone();
    bench("approach 1 (precompiled, pre-resolved HTML)", n, || {
        std::hint::black_box(env.render("div_html", &vars_c));
    });

    let vars_c = vars.clone();
    bench("approach 2 (string replace, pre-resolved HTML)", n, || {
        std::hint::black_box(apply_plain_replace(DIV_HTML, &vars_c));
    });

    // ── figure ──────────────────────────────────────────────────────
    println!("\n=== figure (HTML) ===");

    let mut vars = HashMap::new();
    vars.insert("format".to_string(), "html".to_string());
    vars.insert("label".to_string(), "plot1".to_string());
    vars.insert("align_style".to_string(), "text-align: center".to_string());
    vars.insert(
        "image".to_string(),
        "<img src=\"plot.png\" alt=\"Plot\">".to_string(),
    );
    vars.insert("caption".to_string(), "A nice figure".to_string());

    let vars_c = vars.clone();
    bench("current (full jinja, full template)", n, || {
        std::hint::black_box(apply_template_current(FIGURE_FULL, &vars_c));
    });

    let env = PrecompiledEnv::new(&[("figure_full", FIGURE_FULL)]);
    let vars_c = vars.clone();
    bench("approach 1 (precompiled, full template)", n, || {
        std::hint::black_box(env.render("figure_full", &vars_c));
    });

    // ── Summary ─────────────────────────────────────────────────────
    println!("\n=== Simulated document: 50 code chunks (source+output) + 10 divs ===");

    let mut src_vars = HashMap::new();
    src_vars.insert("format".to_string(), "html".to_string());
    src_vars.insert("lang".to_string(), "r".to_string());
    src_vars.insert("highlighted".to_string(), "<span>code</span>".to_string());
    src_vars.insert("code".to_string(), "x <- 1".to_string());

    let mut out_vars = HashMap::new();
    out_vars.insert("format".to_string(), "html".to_string());
    out_vars.insert("output".to_string(), "[1] 1".to_string());

    let mut div_vars = HashMap::new();
    div_vars.insert("format".to_string(), "html".to_string());
    div_vars.insert("classes".to_string(), "note".to_string());
    div_vars.insert("id_attr".to_string(), String::new());
    div_vars.insert("children".to_string(), "<p>text</p>".to_string());

    let iters = 1000;

    bench("current apply_template (110 calls)", iters, || {
        for _ in 0..50 {
            std::hint::black_box(apply_template_current(CODE_SOURCE_FULL, &src_vars));
            std::hint::black_box(apply_template_current(CODE_OUTPUT_FULL, &out_vars));
        }
        for _ in 0..10 {
            std::hint::black_box(apply_template_current(DIV_FULL, &div_vars));
        }
    });

    let env = PrecompiledEnv::new(&[
        ("src", CODE_SOURCE_FULL),
        ("out", CODE_OUTPUT_FULL),
        ("div", DIV_FULL),
    ]);
    bench("approach 1: precompiled (110 calls)", iters, || {
        for _ in 0..50 {
            std::hint::black_box(env.render("src", &src_vars));
            std::hint::black_box(env.render("out", &out_vars));
        }
        for _ in 0..10 {
            std::hint::black_box(env.render("div", &div_vars));
        }
    });

    bench(
        "approach 2: string replace (110 calls)",
        iters,
        || {
            for _ in 0..50 {
                std::hint::black_box(apply_plain_replace(CODE_SOURCE_HTML, &src_vars));
                std::hint::black_box(apply_plain_replace(CODE_OUTPUT_HTML, &out_vars));
            }
            for _ in 0..10 {
                std::hint::black_box(apply_plain_replace(DIV_HTML, &div_vars));
            }
        },
    );

    println!();
}
