#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Templating])
#import "/.calepin/calepin.typ" as calepin
#metadata((tags: ("themes", "templates"))) <website-metadata>
#title()

Theme layouts are #link("https://docs.rs/minijinja/latest/minijinja/syntax/index.html")[MiniJinja] templates. The HTML layouts (`layouts/site.html` and `layouts/document.html`) and the paged layout (`layouts/pdf.typ`) all use the same engine, so the syntax on this page applies to both. The #link("html_templates.html")[HTML templates] and #link("pdf_templates.html")[PDF templates] pages document the values each layout receives.

= Syntax

MiniJinja has two kinds of tags.

Interpolation prints a value:

```html
<title>{{ doc.title }}</title>
```

Statements control flow. Loops repeat a block:

```html
{% for file in css %}
<style>
{{ file.content }}
</style>
{% endfor %}
```

Conditionals show a block only when a value is set:

```html
{% if site.logo %}
<img src="{{ site.logo }}" alt="{{ site.logo_alt }}">
{% endif %}
```

Includes pull in another template file, which is how themes share partials:

```html
{% include "partials/site-footer.html" %}
```

The #link("https://docs.rs/minijinja/latest/minijinja/syntax/index.html")[MiniJinja syntax reference] covers the rest: filters, tests, macros, and more.

= Context

Each layout receives a context: a set of named values you reference with `{{ }}`. The available names depend on the target.

- HTML layouts receive `site`, `css`, `js`, `doc`, `theme`, `target`, `vars`, and more. See #link("html_templates.html")[HTML templates].
- The paged layout receives `doc`, `theme`, `target`, and `vars`. See #link("pdf_templates.html")[PDF templates].

Both targets receive `theme`, `target`, and `vars`.

= Variables

Pass project-specific template values with `--set vars.<name>=...`:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile notebook.typ --set vars.course=\"Econ 101\" --set vars.semester=\"Fall 2026\"\n", block: true, lang: "sh"))

These values are available as a top-level `vars` in both HTML and paged layouts. Document-level `calepin.setup(vars: ...)` values are merged into the same map, and CLI `--set vars.<name>=...` values take precedence. In HTML templates, `vars` sits at the top level, not under `site`:

```html
<p>{{ vars.course }}, {{ vars.semester }}</p>
```

In a `layouts/pdf.typ` paged layout, read the same values and emit Typst:

```typ
#let course = "{{ vars.course }}"
```
