"""Extract Python package documentation as .qmd files.

Walks all public objects exported by a package's top-level __init__.py,
extracts docstrings, and writes each as a .qmd file with TOML front matter.

Requires: docstring_parser (pip install docstring_parser)
"""

import importlib
import inspect
import os
import sys
import textwrap

try:
    from docstring_parser import parse as parse_docstring
except ImportError:
    print(
        "Error: docstring_parser is required. Install with: pip install docstring_parser",
        file=sys.stderr,
    )
    sys.exit(1)


def safe_name(name):
    return "".join(c if c.isalnum() or c in "._-" else "_" for c in name)


def parse_markdown_sections(text):
    """Parse markdown-heading-style docstrings (e.g., marginaleffects).

    Splits on ## or #### headings, returns list of (heading, body) pairs
    and any preamble text before the first heading.
    """
    lines = text.split("\n")
    preamble = []
    sections = []
    current_heading = None
    current_body = []

    for line in lines:
        stripped = line.strip()
        # Detect ## or #### headings
        if stripped.startswith("## ") or stripped.startswith("#### "):
            if current_heading is not None:
                sections.append((current_heading, "\n".join(current_body).strip()))
            elif current_body:
                preamble = current_body[:]
            level = stripped.split(" ", 1)
            current_heading = level[1].rstrip(":") if len(level) > 1 else ""
            current_body = []
        else:
            current_body.append(line)

    if current_heading is not None:
        sections.append((current_heading, "\n".join(current_body).strip()))
    elif current_body:
        preamble = current_body

    return "\n".join(preamble).strip(), sections


def docstring_to_qmd(name, obj):
    """Convert a Python object's docstring to .qmd content."""
    doc = inspect.getdoc(obj)
    if not doc:
        return None

    parsed = parse_docstring(doc)

    # If docstring_parser found no structure (0 params, 0 meta), the docstring
    # likely uses markdown headings (#### param:). Fall back to markdown parsing.
    use_markdown_fallback = (
        len(parsed.params) == 0
        and len(parsed.meta) == 0
        and ("#### " in doc or "## " in doc)
    )

    if use_markdown_fallback:
        return docstring_to_qmd_markdown(name, obj, doc)

    # TOML front matter
    out = f'---\ntitle = "`{name}`"\n---\n'

    # Summary (short + long description)
    if parsed.short_description:
        out += f"\n*{parsed.short_description}*\n"
    if parsed.long_description:
        out += f"\n{parsed.long_description}\n"

    # Usage (signature)
    if callable(obj) and not isinstance(obj, type):
        try:
            sig = inspect.signature(obj)
            out += f"\n## Usage\n\n```python\n{name}{sig}\n```\n"
        except (ValueError, TypeError):
            pass
    elif isinstance(obj, type):
        try:
            sig = inspect.signature(obj.__init__)
            # Strip 'self' from class __init__
            params = list(sig.parameters.values())
            if params and params[0].name == "self":
                params = params[1:]
            new_sig = sig.replace(parameters=params)
            out += f"\n## Usage\n\n```python\n{name}{new_sig}\n```\n"
        except (ValueError, TypeError):
            pass

    # Parameters
    if parsed.params:
        out += "\n## Parameters\n\n"
        for p in parsed.params:
            type_str = f" (*{p.type_name}*)" if p.type_name else ""
            desc = p.description or ""
            # Indent continuation lines for definition list
            desc_lines = desc.split("\n")
            desc_formatted = desc_lines[0]
            if len(desc_lines) > 1:
                desc_formatted += "\n" + "\n".join(
                    "  " + l for l in desc_lines[1:]
                )
            out += f"**`{p.arg_name}`**{type_str}\n: {desc_formatted}\n\n"

    # Returns
    if parsed.returns:
        out += "\n## Returns\n\n"
        if parsed.returns.type_name:
            out += f"*{parsed.returns.type_name}*\n\n"
        if parsed.returns.description:
            out += f"{parsed.returns.description}\n"
    elif parsed.many_returns:
        out += "\n## Returns\n\n"
        for r in parsed.many_returns:
            type_str = f" (*{r.type_name}*)" if r.type_name else ""
            desc = r.description or ""
            out += f"**`{r.return_name}`**{type_str}\n: {desc}\n\n"

    # Raises
    if parsed.raises:
        out += "\n## Raises\n\n"
        for r in parsed.raises:
            desc = r.description or ""
            out += f"**`{r.type_name}`**\n: {desc}\n\n"

    # Notes
    notes = [m for m in parsed.meta if m.key == "notes"]
    if notes:
        out += "\n## Notes\n\n"
        for n in notes:
            out += f"{n.description}\n"

    # Examples
    examples = [m for m in parsed.meta if m.key == "examples"]
    if examples:
        out += "\n## Examples\n\n"
        for ex in examples:
            desc = (ex.description or "").strip()
            if not desc:
                continue
            has_prompts = any(
                l.strip().startswith(">>>") for l in desc.split("\n")
            )
            if has_prompts:
                # Interactive-style: strip >>> and ... prefixes
                lines = desc.split("\n")
                in_code = False
                code_lines = []
                for line in lines:
                    stripped = line.strip()
                    if stripped.startswith(">>>") or stripped.startswith("..."):
                        if not in_code:
                            in_code = True
                            code_lines = []
                        code = stripped
                        for prefix in (">>> ", ">>>", "... ", "..."):
                            if code.startswith(prefix):
                                code = code[len(prefix):]
                                break
                        code_lines.append(code)
                    else:
                        if in_code:
                            out += "```{python}\n"
                            out += "\n".join(code_lines) + "\n"
                            out += "```\n\n"
                            in_code = False
                            code_lines = []
                        if stripped:
                            out += stripped + "\n"
                if in_code:
                    out += "```{python}\n"
                    out += "\n".join(code_lines) + "\n"
                    out += "```\n"
            else:
                # Plain code block (no >>> prompts)
                out += "```{python}\n" + desc + "\n```\n"

    # References
    refs = [m for m in parsed.meta if m.key == "references"]
    if refs:
        out += "\n## References\n\n"
        for r in refs:
            out += f"{r.description}\n"

    # See Also
    see_also = [m for m in parsed.meta if m.key in ("see_also", "see also")]
    if see_also:
        out += "\n## See Also\n\n"
        for s in see_also:
            out += f"{s.description}\n"

    return out


def docstring_to_qmd_markdown(name, obj, doc):
    """Convert a markdown-heading-style docstring to .qmd."""
    preamble, sections = parse_markdown_sections(doc)

    out = f'---\ntitle = "`{name}`"\n---\n'

    # Preamble as italic summary (strip markdown heading if it's the function name)
    if preamble:
        # Remove leading "# `name()`" style headings
        preamble_clean = preamble
        for prefix in (f"# `{name}()`", f"# `{name}`", f"# {name}"):
            if preamble_clean.startswith(prefix):
                preamble_clean = preamble_clean[len(prefix):].strip()
                break
        if preamble_clean:
            out += f"\n{preamble_clean}\n"

    # Usage (signature)
    if callable(obj) and not isinstance(obj, type):
        try:
            sig = inspect.signature(obj)
            out += f"\n## Usage\n\n```python\n{name}{sig}\n```\n"
        except (ValueError, TypeError):
            pass

    # Emit sections, mapping common heading names
    param_headings = {"Parameters", "Args", "Arguments"}
    for heading, body in sections:
        # Parameter-like headings: check for #### sub-entries
        if heading in param_headings or "#### " in body or heading.startswith("`"):
            # This is a parameter entry (#### `name`: (type)) or a Parameters section
            if heading in param_headings:
                out += f"\n## {heading}\n\n{body}\n"
            else:
                # Individual parameter as #### heading -- emit as definition list
                # heading is like "`model`: (model object)"
                out += f"\n**{heading.split(':')[0].strip()}**"
                type_part = heading.split(":", 1)[1].strip() if ":" in heading else ""
                if type_part:
                    out += f" (*{type_part.strip('() ')}*)"
                out += f"\n: {body}\n\n"
        else:
            # Regular section
            out += f"\n## {heading}\n\n{body}\n"

    return out


def collect_public_api(package):
    """Collect public API objects: only what's exported from the top-level package.

    Uses __all__ if defined, otherwise all non-underscore attributes.
    Deduplicates by object identity (keeps shortest name).
    """
    objects = {}
    seen_ids = {}

    # Get the public names from the top-level module
    if hasattr(package, "__all__"):
        names = package.__all__
    else:
        names = [n for n in dir(package) if not n.startswith("_")]

    for name in sorted(names):
        obj = getattr(package, name, None)
        if obj is None:
            continue
        # Only include functions, classes, and modules with docstrings
        if not (inspect.isfunction(obj) or inspect.isclass(obj) or inspect.ismodule(obj)):
            continue
        # Skip objects imported from outside this package
        obj_mod = getattr(obj, "__module__", "")
        if obj_mod and not obj_mod.startswith(package.__name__):
            continue
        # Deduplicate by identity
        obj_id = id(obj)
        if obj_id in seen_ids:
            # Keep the shorter name
            if len(name) < len(seen_ids[obj_id]):
                del objects[seen_ids[obj_id]]
                objects[name] = obj
                seen_ids[obj_id] = name
        else:
            objects[name] = obj
            seen_ids[obj_id] = name

    return objects


def main():
    pkg_name = sys.argv[1]
    outdir = sys.argv[2]

    try:
        package = importlib.import_module(pkg_name)
    except ImportError:
        print(f"Error: package '{pkg_name}' is not installed.", file=sys.stderr)
        sys.exit(1)

    os.makedirs(outdir, exist_ok=True)

    objects = collect_public_api(package)
    print(f"Extracting {len(objects)} public objects from '{pkg_name}'")

    written = 0
    for name, obj in sorted(objects.items()):
        qmd = docstring_to_qmd(name, obj)
        if not qmd:
            continue
        fname = safe_name(name) + ".qmd"
        with open(os.path.join(outdir, fname), "w") as f:
            f.write(qmd)
        written += 1

    print(f"Wrote {written} .qmd files to '{outdir}'")


if __name__ == "__main__":
    main()
