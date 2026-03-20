#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["pyyaml"]
# ///
"""Astro/Starlight site builder for calepin.

Reads _calepin.yaml (or _variables.yml), renders .qmd pages via
calepin --batch, and generates an Astro/Starlight project in _astro/.

Usage:
    uv run build.py [--quiet] [--skip-build]
"""

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

import yaml

PLUGIN_DIR = Path(__file__).parent
TEMPLATES_DIR = PLUGIN_DIR / "templates"
OUTPUT_DIR = "_astro"
SCAFFOLD_CMD = (
    "npm create astro@latest -- "
    "--template starlight --no-install --yes _astro"
)


# ---------------------------------------------------------------------------
# Template engine
# ---------------------------------------------------------------------------


def _load_template(name):
    return (TEMPLATES_DIR / name).read_text()


def _apply_template(template, vars):
    """Replace {{key}} placeholders in template with values from vars dict."""
    for key, val in vars.items():
        template = template.replace("{{" + key + "}}", val)
    return template


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------


def read_config(path=None):
    if path is None:
        for candidate in ["_calepin.yaml", "_calepin.yml", "_variables.yml"]:
            if Path(candidate).exists():
                path = candidate
                break
        else:
            print("Error: no config file found (_calepin.yaml or _variables.yml)",
                  file=sys.stderr)
            sys.exit(1)
    with open(path) as f:
        return yaml.safe_load(f)


def get(d, *keys, default=None):
    """Nested dict lookup."""
    for k in keys:
        if not isinstance(d, dict):
            return default
        d = d.get(k)
        if d is None:
            return default
    return d


# ---------------------------------------------------------------------------
# Page collection
# ---------------------------------------------------------------------------


def collect_pages(entries):
    """Flatten page hierarchy into [(href, text_or_none), ...]."""
    pages = []
    if not entries:
        return pages
    for entry in entries:
        if isinstance(entry, str):
            pages.append((entry, None))
        elif isinstance(entry, dict):
            if "section" in entry:
                pages.extend(collect_pages(entry.get("pages", [])))
            elif "href" in entry:
                pages.append((entry["href"], entry.get("text")))
    return pages


def flatten_overrides(html_config):
    """Flatten format.html config into ['key=value', ...] overrides."""
    overrides = []
    if not isinstance(html_config, dict):
        return overrides
    for key, val in html_config.items():
        if isinstance(val, dict):
            for mk, mv in val.items():
                overrides.append(f"{key}.{mk}={_override_val(mv)}")
        else:
            overrides.append(f"{key}={_override_val(val)}")
    return overrides


def _override_val(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    return str(v)


# ---------------------------------------------------------------------------
# Batch rendering
# ---------------------------------------------------------------------------


def install_format():
    """Install the 'astro' custom format and body-only page template."""
    fmt_dir = Path("_calepin/formats")
    tpl_dir = Path("_calepin/templates")
    fmt_dir.mkdir(parents=True, exist_ok=True)
    tpl_dir.mkdir(parents=True, exist_ok=True)

    fmt_file = fmt_dir / "astro.yaml"
    if not fmt_file.exists():
        fmt_file.write_text("base: html\nextension: html\n")

    tpl_file = tpl_dir / "calepin.astro"
    src = TEMPLATES_DIR / "calepin.astro.html"
    shutil.copy2(src, tpl_file)


def run_batch(pages, overrides, quiet=False):
    """Build manifest and call calepin --batch --batch-stdout."""
    install_format()

    manifest = []
    for href, _ in pages:
        if not href.endswith(".qmd"):
            continue
        manifest.append({
            "input": href,
            "format": "astro",
            "overrides": overrides,
        })

    cmd = ["calepin", "--batch", "-", "--batch-stdout"]
    if quiet:
        cmd.append("-q")

    result = subprocess.run(
        cmd,
        input=json.dumps(manifest),
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print(f"Error: calepin --batch failed:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)

    return json.loads(result.stdout)


# ---------------------------------------------------------------------------
# Heading extraction
# ---------------------------------------------------------------------------


_HEADING_RE = re.compile(
    r'<h([234])\s[^>]*id="([^"]*)"[^>]*>(.*?)</h\1>',
    re.DOTALL,
)
_TAG_RE = re.compile(r"<[^>]+>")


def extract_headings(html):
    """Extract h2-h4 headings with id and text from HTML."""
    headings = []
    for m in _HEADING_RE.finditer(html):
        text = _TAG_RE.sub("", m.group(3)).strip()
        if text:
            headings.append({
                "depth": int(m.group(1)),
                "slug": m.group(2),
                "text": text,
            })
    return headings


def format_headings_js(headings):
    if not headings:
        return "[]"
    items = []
    for h in headings:
        slug = h["slug"].replace("'", "\\'")
        text = h["text"].replace("'", "\\'")
        items.append(
            f"  {{ depth: {h['depth']}, slug: '{slug}', text: '{text}' }}"
        )
    return "[\n" + ",\n".join(items) + "\n]"


# ---------------------------------------------------------------------------
# Astro page generation
# ---------------------------------------------------------------------------


def _relative_prefix(stem):
    """Compute ../ prefix from src/pages/{stem}.astro back to src/."""
    depth = stem.count("/") + 1
    return "../" * depth


def _escape_astro(s):
    return s.replace('"', "&quot;").replace("{", "&#123;").replace("}", "&#125;")


def _strip_markdown(s):
    return s.replace("*", "").replace("_", "")


def build_astro_page(stem, title, body_html):
    tpl = _load_template("page.astro") + _load_template("scripts.html")
    return _apply_template(tpl, {
        "prefix": _relative_prefix(stem),
        "stem": stem,
        "headings_js": format_headings_js(extract_headings(body_html)),
        "title": _escape_astro(_strip_markdown(title)),
    })


def build_astro_index_page(
    stem, title, subtitle, author, date, abstract_text, body_html, config
):
    # Build hero body lines
    parts = []
    logo = get(config, "website", "navbar", "logo")
    if logo:
        filename = Path(logo).name
        alt = _escape_astro(get(config, "website", "title", default=""))
        dark = _dark_variant(logo)
        if Path(dark).exists():
            dark_filename = Path(dark).name
            parts.append(
                f'    <img src="/{filename}" alt="{alt}" '
                f'class="hero-logo hero-logo-light" />'
                f'<img src="/{dark_filename}" alt="{alt}" '
                f'class="hero-logo hero-logo-dark" />'
            )
        else:
            parts.append(
                f'    <img src="/{filename}" alt="{alt}" class="hero-logo" />'
            )
    if subtitle:
        parts.append(f'    <div class="hero-subtitle">{subtitle}</div>')
    if author:
        parts.append(f'    <div class="hero-meta">{author}</div>')
    # Skip date if it contains unevaluated inline code
    if date and "`{" not in date:
        parts.append(f'    <div class="hero-meta">{date}</div>')
    if abstract_text:
        parts.append(f'    <div class="hero-abstract">{abstract_text}</div>')

    tpl = _load_template("index.astro") + _load_template("scripts.html")
    return _apply_template(tpl, {
        "prefix": _relative_prefix(stem),
        "stem": stem,
        "headings_js": format_headings_js(extract_headings(body_html)),
        "title": _escape_astro(_strip_markdown(title)),
        "hero_body": "\n".join(parts),
    })


# ---------------------------------------------------------------------------
# astro.config.mjs generation
# ---------------------------------------------------------------------------


def _escape_js(s):
    return s.replace("\\", "\\\\").replace("'", "\\'")


def _dark_variant(path):
    """logo.png -> logo_dark.png"""
    p = Path(path)
    return str(p.with_name(f"{p.stem}_dark{p.suffix}"))

    # Fall back to config
    logo = get(config, "website", "navbar", "logo")
    if not logo:
        return {}
    return {"light": logo, "dark": None, "alt": None}


def _title_from_filename(href):
    stem = Path(href).stem
    return " ".join(w.capitalize() for w in stem.split("_"))


def _guess_social_icon(text, href):
    t = text.lower()
    h = href.lower()
    for keyword, icon in [
        ("github", "github"), ("discord", "discord"),
        ("twitter", "x.com"), ("x.com", "x.com"),
        ("mastodon", "mastodon"), ("bluesky", "blueSky"),
        ("linkedin", "linkedin"), ("youtube", "youtube"),
    ]:
        if keyword in t or keyword in h:
            return icon
    if "github.com" in h:
        return "github"
    if "twitter.com" in h:
        return "x.com"
    if "bsky" in h:
        return "blueSky"
    return "github"


def _sidebar_items(entries, titles, indent=8):
    """Recursively build sidebar config JS."""
    pad = " " * indent
    items = []
    if not entries:
        return ""
    for entry in entries:
        if isinstance(entry, str):
            href = entry
            text = None
        elif isinstance(entry, dict) and "section" in entry:
            label = _escape_js(entry["section"])
            children = _sidebar_items(
                entry.get("pages", []), titles, indent + 4
            )
            items.append(
                f"{pad}{{\n{pad}  label: '{label}',\n"
                f"{pad}  items: [\n{children}\n{pad}  ]\n{pad}}}"
            )
            continue
        elif isinstance(entry, dict) and "href" in entry:
            href = entry["href"]
            text = entry.get("text")
        else:
            continue

        stem = href.removesuffix(".qmd") if href.endswith(".qmd") else href
        link = "/" if stem == "index" else f"/{stem}"
        if text:
            label = _escape_js(_strip_markdown(text))
        else:
            label = _escape_js(
                _strip_markdown(titles.get(stem, _title_from_filename(href)))
            )
        items.append(f"{pad}{{ label: '{label}', link: '{link}' }}")

    return ",\n".join(items)


def build_astro_config(config, titles):
    title = _escape_js(get(config, "website", "title", default="Untitled"))

    # Logo
    logo = get(config, "website", "navbar", "logo")
    logo_config = ""
    if logo:
        filename = Path(logo).name
        dark = _dark_variant(logo)
        if Path(dark).exists():
            dark_filename = Path(dark).name
            logo_config = (
                f"\n      logo: {{ replacesTitle: true, "
                f"light: './src/assets/{filename}', "
                f"dark: './src/assets/{dark_filename}' }},"
            )
        else:
            logo_config = (
                f"\n      logo: {{ replacesTitle: true, "
                f"src: './src/assets/{filename}' }},"
            )

    # Favicon
    favicon = get(config, "website", "favicon")
    favicon_config = ""
    if favicon:
        favicon_config = f"\n      favicon: '{Path(favicon).name}',"

    # Social links
    nav_right = get(config, "website", "navbar", "right", default=[])
    social_lines = []
    for item in nav_right:
        if isinstance(item, dict) and "text" in item and "href" in item:
            icon = _guess_social_icon(item["text"], item["href"])
            social_lines.append(
                f"        {{ icon: '{icon}', "
                f"label: '{_escape_js(item['text'])}', "
                f"href: '{_escape_js(item['href'])}' }}"
            )
    social_config = ""
    if social_lines:
        social_config = (
            "\n      social: [\n"
            + ",\n".join(social_lines)
            + "\n      ],"
        )

    # Sidebar
    pages = get(config, "website", "pages", default=[])
    sidebar = _sidebar_items(pages, titles)

    tpl = _load_template("astro.config.mjs")
    return _apply_template(tpl, {
        "title": title,
        "logo_config": logo_config,
        "favicon_config": favicon_config,
        "social_config": social_config,
        "sidebar": sidebar,
    })


# ---------------------------------------------------------------------------
# Asset copying
# ---------------------------------------------------------------------------


def copy_assets(config, rendered_pages, output_dir):
    out = Path(output_dir)

    # Logo
    logo = get(config, "website", "navbar", "logo")
    if logo and Path(logo).is_file():
        filename = Path(logo).name
        for dest in [out / "src/assets" / filename, out / "public" / filename]:
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(logo, dest)
        # Dark variant
        dark = _dark_variant(logo)
        if Path(dark).is_file():
            dark_fn = Path(dark).name
            for dest in [out / "src/assets" / dark_fn, out / "public" / dark_fn]:
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(dark, dest)

    # Favicon
    favicon = get(config, "website", "favicon")
    if favicon and Path(favicon).is_file():
        dest = out / "public" / Path(favicon).name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(favicon, dest)

    # Resources
    for res in get(config, "project", "resources", default=[]):
        src = Path(res)
        dest = out / "public" / res
        if src.is_dir():
            if dest.exists():
                shutil.rmtree(dest)
            shutil.copytree(src, dest)
        elif src.is_file():
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dest)

    # Figure directories
    for page in rendered_pages:
        if page.get("status") != "ok":
            continue
        stem = Path(page["input"]).stem
        fig_dir = Path(f"{stem}_files")
        if fig_dir.is_dir():
            dest = out / "public" / f"{stem}_files"
            if dest.exists():
                shutil.rmtree(dest)
            shutil.copytree(fig_dir, dest)

    # Non-.qmd page files (PDFs, etc.)
    all_pages = collect_pages(get(config, "website", "pages", default=[]))
    for href, _ in all_pages:
        if not href.endswith(".qmd") and Path(href).is_file():
            dest = out / "public" / href
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(href, dest)


# ---------------------------------------------------------------------------
# Scaffolding and npm
# ---------------------------------------------------------------------------


def scaffold(output_dir, quiet=False):
    if (Path(output_dir) / "package.json").exists():
        return
    if not quiet:
        print("Scaffolding Astro/Starlight project...", file=sys.stderr)
    result = subprocess.run(
        SCAFFOLD_CMD, shell=True, capture_output=True, text=True,
    )
    if result.returncode != 0:
        print(f"Error: scaffold failed:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)
    git_dir = Path(output_dir) / ".git"
    if git_dir.is_dir():
        shutil.rmtree(git_dir)


def npm_install(output_dir, quiet=False):
    if (Path(output_dir) / "node_modules").exists():
        return
    if not quiet:
        print("Installing npm dependencies...", file=sys.stderr)
    subprocess.run(["npm", "install"], cwd=output_dir, capture_output=quiet)


def npm_build(output_dir, quiet=False):
    if not quiet:
        print("Building site...", file=sys.stderr)
    result = subprocess.run(
        ["npm", "run", "build"], cwd=output_dir, capture_output=True, text=True,
    )
    if result.returncode != 0:
        print(f"Error: npm build failed:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)


# ---------------------------------------------------------------------------
# YAML front matter reader (for author on index page)
# ---------------------------------------------------------------------------


def read_qmd_metadata(path):
    try:
        text = Path(path).read_text()
    except FileNotFoundError:
        return {}
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end < 0:
        return {}
    try:
        return yaml.safe_load(text[3:end]) or {}
    except yaml.YAMLError:
        return {}


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    quiet = "--quiet" in sys.argv or "-q" in sys.argv
    skip_build = "--skip-build" in sys.argv

    config = read_config()
    pages_config = get(config, "website", "pages", default=[])
    pages = collect_pages(pages_config)
    html_config = get(config, "format", "html", default={})
    overrides = flatten_overrides(html_config)

    scaffold(OUTPUT_DIR, quiet)

    if not quiet:
        print("Rendering pages...", file=sys.stderr)
    results = run_batch(pages, overrides, quiet)

    out = Path(OUTPUT_DIR)

    # Title map from batch results
    titles = {}
    for r in results:
        if r.get("status") == "ok" and r.get("title"):
            stem = Path(r["input"]).with_suffix("").as_posix()
            titles[stem] = r["title"]

    # Write rendered pages and generate .astro wrappers
    for r in results:
        if r.get("status") != "ok":
            if not quiet:
                print(
                    f"Warning: {r['input']}: {r.get('error', 'unknown error')}",
                    file=sys.stderr,
                )
            continue

        stem = Path(r["input"]).with_suffix("").as_posix()
        body = r.get("body", "")
        title = r.get("title") or _title_from_filename(r["input"])

        # Write HTML body
        html_path = out / f"src/html/{stem}.html"
        html_path.parent.mkdir(parents=True, exist_ok=True)
        html_path.write_text(body)

        # Copy .qmd source for split view
        qmd_dest = out / f"src/qmd/{stem}.qmd"
        qmd_dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(r["input"], qmd_dest)

        # Generate .astro page
        if stem == "index":
            meta = read_qmd_metadata(r["input"])
            author = meta.get("author")
            if isinstance(author, list):
                author = ", ".join(str(a) for a in author)
            astro = build_astro_index_page(
                stem, title,
                subtitle=r.get("subtitle") or meta.get("subtitle"),
                author=author,
                date=r.get("date"),
                abstract_text=r.get("abstract"),
                body_html=body,
                config=config,
            )
        else:
            astro = build_astro_page(stem, title, body)

        astro_path = out / f"src/pages/{stem}.astro"
        astro_path.parent.mkdir(parents=True, exist_ok=True)
        astro_path.write_text(astro)

    # Starlight content collection placeholder (required by Starlight)
    placeholder = out / "src/content/docs/index.mdx"
    placeholder.parent.mkdir(parents=True, exist_ok=True)
    placeholder.write_text(
        "---\ntitle: Home\ntemplate: splash\nhero:\n"
        "  title: ' '\n---\n"
    )

    # astro.config.mjs
    (out / "astro.config.mjs").write_text(build_astro_config(config, titles))

    # CSS
    css_dest = out / "src/styles/calepin.css"
    css_dest.parent.mkdir(parents=True, exist_ok=True)
    css_dest.write_text((PLUGIN_DIR / "astro.css").read_text())

    # Clean stale Starlight docs (keep our placeholder)
    docs_dir = out / "src/content/docs"
    if docs_dir.is_dir():
        for f in docs_dir.iterdir():
            if f.name != "index.mdx":
                if f.is_dir():
                    shutil.rmtree(f)
                else:
                    f.unlink()

    # Copy assets
    copy_assets(config, results, OUTPUT_DIR)

    if not skip_build:
        npm_install(OUTPUT_DIR, quiet)
        npm_build(OUTPUT_DIR, quiet)

    if not quiet:
        print(f"\u2192 {OUTPUT_DIR}/dist/", file=sys.stderr)


if __name__ == "__main__":
    main()
