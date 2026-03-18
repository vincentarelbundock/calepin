"""Extract Python package documentation as .qmd files using griffe.

Walks the package AST (no import needed), parses numpydoc/google-style
docstrings, and writes each public object as a .qmd file.

Requires: griffe (pip install griffe)
"""

import os
import sys

try:
    import griffe
    from griffe.enumerations import DocstringSectionKind
except ImportError:
    print(
        "Error: griffe is required. Install with: pip install griffe",
        file=sys.stderr,
    )
    sys.exit(1)


def safe_name(name):
    return "".join(c if c.isalnum() or c in "._-" else "_" for c in name)


def render_section(section, depth=0):
    """Convert a griffe DocstringSection to markdown."""
    kind = section.kind

    if kind == DocstringSectionKind.text:
        return section.value.strip()

    if kind == DocstringSectionKind.parameters:
        lines = []
        for param in section.value:
            ann = f" (*{param.annotation}*)" if param.annotation else ""
            desc = (param.description or "").strip()
            lines.append(f"**`{param.name}`**{ann}\n: {desc}\n")
        return "\n".join(lines)

    if kind == DocstringSectionKind.returns:
        lines = []
        for ret in section.value:
            ann = f"*{ret.annotation}*" if ret.annotation else ""
            desc = (ret.description or "").strip()
            if ret.name:
                lines.append(f"**`{ret.name}`** {ann}\n: {desc}\n")
            elif ann:
                lines.append(f"{ann}: {desc}\n")
            else:
                lines.append(f"{desc}\n")
        return "\n".join(lines)

    if kind == DocstringSectionKind.raises:
        lines = []
        for exc in section.value:
            ann = exc.annotation or ""
            desc = (exc.description or "").strip()
            lines.append(f"**`{ann}`**\n: {desc}\n")
        return "\n".join(lines)

    if kind == DocstringSectionKind.examples:
        out = ""
        for item in section.value:
            # item is (kind, value) tuple
            item_kind, item_value = item
            if item_kind == "text":
                text = item_value.strip()
                if text:
                    out += text + "\n\n"
            elif item_kind == "examples":
                # Code block -- strip >>> and ... prompts
                lines = []
                for line in item_value.strip().split("\n"):
                    stripped = line.strip()
                    if stripped.startswith(">>> "):
                        lines.append(stripped[4:])
                    elif stripped.startswith(">>>"):
                        lines.append(stripped[3:])
                    elif stripped.startswith("... "):
                        lines.append(stripped[4:])
                    elif stripped.startswith("..."):
                        lines.append(stripped[3:])
                    elif stripped and not stripped.startswith("#") and lines:
                        # Output line from interactive session -- skip
                        pass
                    else:
                        lines.append(stripped)
                if lines:
                    out += "```{python}\n" + "\n".join(lines) + "\n```\n\n"
        return out.strip()

    if kind == DocstringSectionKind.admonition:
        title = getattr(section, "title", "") or section.value.annotation or ""
        desc = section.value.description or ""
        return f"**{title}**\n\n{desc}"

    if kind in (
        DocstringSectionKind.notes,
        DocstringSectionKind.warnings,
        DocstringSectionKind.references,
        DocstringSectionKind.deprecated,
    ):
        return section.value.strip() if isinstance(section.value, str) else str(section.value)

    # attributes, other_parameters, etc.
    if hasattr(section, "value") and isinstance(section.value, list):
        lines = []
        for item in section.value:
            name = getattr(item, "name", "")
            ann = getattr(item, "annotation", "")
            desc = getattr(item, "description", "") or ""
            ann_str = f" (*{ann}*)" if ann else ""
            lines.append(f"**`{name}`**{ann_str}\n: {desc.strip()}\n")
        return "\n".join(lines)

    return str(section.value).strip() if section.value else ""


SECTION_HEADINGS = {
    DocstringSectionKind.text: None,  # preamble, no heading
    DocstringSectionKind.parameters: "Parameters",
    DocstringSectionKind.other_parameters: "Other Parameters",
    DocstringSectionKind.returns: "Returns",
    DocstringSectionKind.yields: "Yields",
    DocstringSectionKind.raises: "Raises",
    DocstringSectionKind.examples: "Examples",
    DocstringSectionKind.notes: "Notes",
    DocstringSectionKind.warnings: "Warnings",
    DocstringSectionKind.references: "References",
    DocstringSectionKind.attributes: "Attributes",
    DocstringSectionKind.deprecated: "Deprecated",
    DocstringSectionKind.admonition: "Note",
}


def obj_to_qmd(path, obj, parser="numpy"):
    """Convert a griffe Object to .qmd content."""
    if not obj.docstring:
        return None

    sections = obj.docstring.parse(parser)
    if not sections:
        return None

    # Use the short name (e.g., "predictions" not "marginaleffects.predictions")
    short_name = path.rsplit(".", 1)[-1]

    out = f'---\ntitle = "`{short_name}`"\n---\n'

    # Signature
    if hasattr(obj, "parameters") and obj.parameters:
        params = []
        for p in obj.parameters:
            param = p.name
            if p.annotation:
                param += f": {p.annotation}"
            if p.default is not None and str(p.default) != "":
                param += f" = {p.default}"
            params.append(param)
        sig = f"{short_name}({', '.join(params)})"
        out += f"\n## Usage\n\n```python\n{sig}\n```\n"

    # Docstring sections
    for section in sections:
        heading = SECTION_HEADINGS.get(section.kind, section.kind.name.replace("_", " ").title())
        body = render_section(section)
        if not body:
            continue
        if heading is None:
            # Preamble text -- show as italic summary
            out += f"\n{body}\n"
        else:
            out += f"\n## {heading}\n\n{body}\n"

    return out


def collect_public_objects(package, parser="numpy"):
    """Walk package and collect public documented objects."""
    results = []

    def walk(obj, prefix=""):
        # Skip private
        if obj.name.startswith("_"):
            return

        path = f"{prefix}.{obj.name}" if prefix else obj.name

        # Emit functions and classes with docstrings
        if isinstance(obj, (griffe.Function, griffe.Class)):
            if obj.docstring:
                results.append((path, obj))
        elif isinstance(obj, griffe.Module):
            # Only recurse into public modules
            for member in obj.members.values():
                if not member.name.startswith("_"):
                    walk(member, path)

    for member in package.members.values():
        walk(member, package.name)

    return results


def main():
    pkg_name = sys.argv[1]
    outdir = sys.argv[2]

    # Determine docstring style (default: numpy)
    parser = "numpy"
    for arg in sys.argv[3:]:
        if arg.startswith("--style="):
            parser = arg.split("=", 1)[1]

    try:
        if os.path.isdir(pkg_name):
            # Source directory: load from the parent, using the dir name as module
            abs_path = os.path.abspath(pkg_name)
            parent = os.path.dirname(abs_path)
            module_name = os.path.basename(abs_path)
            package = griffe.load(
                module_name,
                search_paths=[parent],
                allow_inspection=False,
            )
        else:
            package = griffe.load(pkg_name, allow_inspection=True)
    except Exception as e:
        print(f"Error: could not load '{pkg_name}': {e}", file=sys.stderr)
        sys.exit(1)

    os.makedirs(outdir, exist_ok=True)

    # Collect public objects
    objects = collect_public_objects(package, parser)

    # Filter to top-level exports: prefer short paths, deduplicate by name
    seen_names = {}
    for path, obj in objects:
        short = path.rsplit(".", 1)[-1]
        if short not in seen_names or len(path) < len(seen_names[short][0]):
            seen_names[short] = (path, obj)

    public = sorted(seen_names.values())
    print(f"Extracting {len(public)} documented objects from '{pkg_name}'")

    written = 0
    for path, obj in public:
        qmd = obj_to_qmd(path, obj, parser)
        if not qmd:
            continue
        short = path.rsplit(".", 1)[-1]
        fname = safe_name(short) + ".qmd"
        with open(os.path.join(outdir, fname), "w") as f:
            f.write(qmd)
        written += 1

    print(f"Wrote {written} .qmd files to '{outdir}'")


if __name__ == "__main__":
    main()
