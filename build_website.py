#!/usr/bin/env python3
"""Build the docs website with Calepin from Typst sources."""

from __future__ import annotations

import fnmatch
import html
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tomllib
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Optional

ROOT = Path(__file__).resolve().parent
DEFAULT_SRC_DIR = ROOT / "docs"
DEFAULT_OUT_DIR = DEFAULT_SRC_DIR
DEFAULT_TEMPLATE = "calepin-website"
FALLBACK_PAGE = "404.typ"
CONFIG_PATH = ROOT / "website.toml"
SITE_METADATA_LABEL = "<website-metadata>"
SOURCE_DATA_ID = "calepin-website-source-data"


def run_command(cmd: list[str], *, cwd: Path | None = None) -> str:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd is not None else None,
        check=True,
        capture_output=True,
        text=True,
    )
    return proc.stdout.strip()


def typst_query_json(path: Path, selector: str, *, field: Optional[str] = None) -> list[object]:
    cmd = ["typst", "query", str(path), selector]
    if field:
        cmd.extend(["--field", field])

    try:
        output = run_command(cmd)
    except subprocess.CalledProcessError:
        return []

    try:
        result = json.loads(output)
    except json.JSONDecodeError:
        return []

    return result if isinstance(result, list) else []


def title_from_metadata(path: Path) -> Optional[str]:
    label = SITE_METADATA_LABEL.strip("<>")
    values = typst_query_json(path, f'label("{label}")', field="value")
    if not values:
        return None

    first = values[0]
    if isinstance(first, dict):
        title = first.get("title")
        if isinstance(title, str):
            title = title.strip()
            if title:
                return title
    return None


def title_from_typst_file(path: Path) -> str:
    return title_from_metadata(path) or path.stem.replace("-", " ").replace("_", " ")


def iter_typ_files(src_dir: Path, *, include_hidden: bool = False, exclude: tuple[str, ...] = ()):
    exclude_rel = {Path(name) for name in exclude}
    for typ_file in sorted(src_dir.rglob("*.typ")):
        rel = typ_file.relative_to(src_dir)
        if not include_hidden and any(part.startswith(".") for part in rel.parts):
            continue
        if rel in exclude_rel:
            continue
        if typ_file.is_file():
            yield typ_file


def rel_html_path(src_dir: Path, path: Path) -> str:
    return path.relative_to(src_dir).with_suffix(".html").as_posix()


def rel_typ_posix(src_dir: Path, path: Path) -> str:
    return path.relative_to(src_dir).as_posix()


def resolve_file_list(
    src_dir: Path,
    item_cfg: dict,
    all_typ_files: list[Path],
) -> list[Path]:
    path_value = item_cfg.get("path")
    glob_value = item_cfg.get("glob")

    if path_value is not None and glob_value is not None:
        print(
            f"warning: item has both path and glob, using path only: {item_cfg}",
            file=sys.stderr,
        )

    if path_value is not None:
        if not isinstance(path_value, str):
            return []

        candidate = src_dir / Path(path_value)
        if candidate.is_file() and candidate.suffix == ".typ":
            return [candidate]

        print(f"warning: sidebar item path does not exist or is not a .typ file: {path_value}", file=sys.stderr)
        return []

    if glob_value is not None:
        if not isinstance(glob_value, str):
            return []

        pattern = Path(glob_value).as_posix()
        return [
            path
            for path in all_typ_files
            if fnmatch.fnmatch(rel_typ_posix(src_dir, path), pattern)
        ]

    return []


def build_auto_navigation(src_dir: Path, *, include_hidden: bool = False) -> tuple[list[dict], list[Path]]:
    files = list(iter_typ_files(src_dir, include_hidden=include_hidden, exclude=(FALLBACK_PAGE,)))
    items = [
        {
            "href": rel_html_path(src_dir, path),
            "label": title_from_typst_file(path),
            "path": path,
        }
        for path in files
    ]
    return [{"title": None, "items": items}], files


def build_manual_navigation(src_dir: Path, sidebar_cfg: dict) -> tuple[list[dict], list[Path]]:
    show_hidden = bool(sidebar_cfg.get("show_hidden", False))
    raw_sections = sidebar_cfg.get("section") or []

    if not isinstance(raw_sections, list):
        print("warning: [sidebar.section] must be a list", file=sys.stderr)
        return [{"title": None, "items": []}], []

    all_typ_files = list(iter_typ_files(src_dir, include_hidden=show_hidden, exclude=(FALLBACK_PAGE,)))
    sections: list[dict] = []
    used: set[Path] = set()

    for section_cfg in raw_sections:
        if not isinstance(section_cfg, dict):
            continue

        section_title = section_cfg.get("title", "")
        if not isinstance(section_title, str):
            section_title = str(section_title)

        raw_items = section_cfg.get("item") or []
        if not isinstance(raw_items, list):
            print(
                f"warning: sidebar section '{section_title}' has invalid item list",
                file=sys.stderr,
            )
            continue

        resolved_items: list[dict] = []
        for item_cfg in raw_items:
            if not isinstance(item_cfg, dict):
                continue

            candidates = resolve_file_list(src_dir, item_cfg, all_typ_files)
            if not candidates:
                continue

            item_label_cfg = item_cfg.get("label")
            item_label = item_label_cfg.strip() if isinstance(item_label_cfg, str) else None

            for path in candidates:
                if path in used:
                    continue

                used.add(path)
                label = item_label or title_from_typst_file(path)
                resolved_items.append(
                    {
                        "href": rel_html_path(src_dir, path),
                        "label": label,
                        "path": path,
                    }
                )

        sections.append(
            {
                "title": section_title,
                "items": resolved_items,
            }
        )

    build_files: list[Path] = []
    for section in sections:
        for item in section["items"]:
            build_files.append(item["path"])

    return sections, build_files


def render_sidebar_html(sections: list[dict], current_href: str) -> str:
    lines: list[str] = []
    for section in sections:
        section_title = section.get("title")
        items = section.get("items", []) or []
        if not items:
            continue

        if section_title:
            lines.append(f"<p><strong>{html.escape(str(section_title))}</strong></p>")

        lines.append("<ul>")
        for item in items:
            href = item["href"]
            label = item["label"]
            is_active = href == current_href

            attrs = ' class="active" aria-current="page"' if is_active else ""
            lines.append(
                f'<li><a href="{html.escape(href)}"{attrs}>{html.escape(label)}</a></li>'
            )
        lines.append("</ul>")

    return "\n".join(lines)


def apply_sidebar(html_path: Path, sections: list[dict], current_href: str) -> None:
    html_value = html_path.read_text(encoding="utf-8")
    rendered_sidebar = render_sidebar_html(sections, current_href)

    pattern = re.compile(
        r'(<aside\b[^>]*class="[^\"]*calepin-website-sidebar[^\"]*"[^>]*>.*?<nav\b[^>]*>)(.*?)(</nav>)',
        re.DOTALL,
    )

    match = pattern.search(html_value)
    if not match:
        return

    new_html = html_value[: match.start(1)] + match.group(1) + rendered_sidebar + match.group(3) + html_value[match.end(3) :]
    html_path.write_text(new_html, encoding="utf-8")



def load_website_config() -> dict:
    if not CONFIG_PATH.is_file():
        return {}

    try:
        with CONFIG_PATH.open("rb") as fp:
            return tomllib.load(fp)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise SystemExit(f"failed to parse {CONFIG_PATH}: {exc}")


def write_calepin_config(src_dir: Path) -> None:
    (src_dir / ".calepin").mkdir(parents=True, exist_ok=True)
    (src_dir / ".calepin" / "config.toml").write_text(
        "\n".join(
            [
                'themes_dir = "../themes"',
                "",
                "[executables]",
                'python = "../.venv/bin/python"',
                "",
            ]
        )
    )


def embed_source_blob(html_output: Path, source_path: Path) -> None:
    source_payload = json.dumps(source_path.read_text(encoding="utf-8"))
    html_value = html_output.read_text(encoding="utf-8")

    embed_script = (
        f"\n<script id=\"{SOURCE_DATA_ID}\" type=\"application/json\">"
        f"{source_payload}"
        "</script>\n"
    )

    if "</head>" in html_value:
        html_value = html_value.replace("</head>", embed_script + "</head>", 1)
    else:
        html_value = html_value + embed_script

    html_output.write_text(html_value, encoding="utf-8")


def copy_assets(src_dir: Path, out_dir: Path) -> None:
    assets_dir = src_dir / "assets"
    if assets_dir.is_dir():
        target = out_dir / "assets"
        if target.exists():
            shutil.rmtree(target)
        shutil.copytree(assets_dir, target)


def copy_typ_sources(src_dir: Path, out_dir: Path, typ_files: list[Path]) -> None:
    for input_path in typ_files:
        rel = input_path.relative_to(src_dir)
        target = out_dir / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(input_path.read_text(encoding="utf-8"))


def _resolve_max_workers() -> int:
    workers = os.environ.get("WEBSITE_PARALLELISM")
    if workers:
        try:
            parsed = int(workers)
            if parsed <= 0:
                raise ValueError
        except ValueError:
            print(f"warning: WEBSITE_PARALLELISM must be a positive integer: {workers}", file=sys.stderr)
            parsed = os.cpu_count() or 1
    else:
        parsed = min(32, (os.cpu_count() or 1))

    return max(1, parsed)


def _compile_document(
    src_dir: Path,
    out_dir: Path,
    calepin_base: list[str],
    input_path: Path,
    sidebar_sections: list[dict],
    template: str,
) -> None:
    rel_output = input_path.relative_to(src_dir).with_suffix("")
    html_output = out_dir / rel_output.with_suffix(".html")
    html_output.parent.mkdir(parents=True, exist_ok=True)

    run_command(
        [
            *calepin_base,
            "compile",
            str(input_path),
            str(html_output),
            "--format=html",
            f"--template={template}",
            "--quiet",
        ],
        cwd=ROOT,
    )

    current_href = rel_output.with_suffix(".html").as_posix()
    apply_sidebar(html_output, sidebar_sections, current_href)
    embed_source_blob(html_output, input_path)

    pdf_output = out_dir / rel_output.with_suffix(".pdf")
    run_command(
        [
            *calepin_base,
            "compile",
            str(input_path),
            str(pdf_output),
            "--format=pdf",
            "--quiet",
        ],
        cwd=ROOT,
    )


def compile_documents(
    src_dir: Path,
    out_dir: Path,
    calepin_cmd: str,
    typ_files: list[Path],
    sidebar_sections: list[dict],
    template: str,
) -> None:
    calepin_base = shlex.split(calepin_cmd)
    max_workers = min(_resolve_max_workers(), len(typ_files) or 1)

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = {
            executor.submit(
                _compile_document,
                src_dir,
                out_dir,
                calepin_base,
                input_path,
                sidebar_sections,
                template,
            ): input_path
            for input_path in typ_files
        }

        for future in as_completed(futures):
            input_path = futures[future]
            try:
                future.result()
            except Exception as exc:
                raise RuntimeError(
                    f"failed to render {input_path} with calepin: {exc}"
                ) from exc


def default_calepin_cmd() -> str:
    return "calepin" if shutil.which("calepin") else "uv run calepin"


def clear_previous_outputs(src_dir: Path, out_dir: Path) -> None:
    if out_dir == src_dir:
        for input_path in iter_typ_files(src_dir):
            rel_output = input_path.relative_to(src_dir).with_suffix("")
            html_output = out_dir / rel_output.with_suffix(".html")
            pdf_output = out_dir / rel_output.with_suffix(".pdf")
            if html_output.exists():
                html_output.unlink()
            if pdf_output.exists():
                pdf_output.unlink()
    elif out_dir.exists():
        for item in out_dir.iterdir():
            if item.is_file() and item.name != ".gitkeep":
                item.unlink()
            elif item.is_dir():
                shutil.rmtree(item)


def build_sidebar_navigation(src_dir: Path, config: dict) -> tuple[list[dict], list[Path]]:
    sidebar_cfg = config.get("sidebar")
    if not isinstance(sidebar_cfg, dict):
        return build_auto_navigation(src_dir)

    sections, files = build_manual_navigation(src_dir, sidebar_cfg)
    if not sections:
        sections = [{"title": None, "items": []}]
    return sections, files


def build_navigation(src_dir: Path, config: dict) -> tuple[list[dict], list[Path]]:
    if "sidebar" not in config:
        return build_auto_navigation(src_dir)

    return build_sidebar_navigation(src_dir, config)


def main() -> None:
    config = load_website_config()

    src_default = config.get("src", str(DEFAULT_SRC_DIR))
    out_default = config.get("out", str(DEFAULT_OUT_DIR))
    template_default = config.get("template", DEFAULT_TEMPLATE)

    src_dir = Path(os.environ.get("SRC_DIR", src_default))
    if not src_dir.is_absolute():
        src_dir = (ROOT / src_dir).resolve()

    out_dir = Path(os.environ.get("OUT_DIR", out_default))
    if not out_dir.is_absolute():
        out_dir = (ROOT / out_dir).resolve()

    template = os.environ.get("WEBSITE_TEMPLATE", template_default)

    if not src_dir.is_dir():
        raise SystemExit(f"source directory not found: {src_dir}")

    clear_previous_outputs(src_dir, out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    write_calepin_config(src_dir)

    sidebar_sections, typ_files = build_navigation(src_dir, config)

    # The 404 page is always compiled but kept out of the sidebar navigation.
    fallback_page = src_dir / FALLBACK_PAGE
    if fallback_page.is_file():
        typ_files = list(typ_files) + [fallback_page]

    typ_files = sorted(set(typ_files), key=lambda p: p.relative_to(src_dir).as_posix())

    if out_dir != src_dir:
        copy_assets(src_dir, out_dir)
        copy_typ_sources(src_dir, out_dir, typ_files if "sidebar" in config else list(iter_typ_files(src_dir)))

    calepin_cmd = os.environ.get("CALEPIN", default_calepin_cmd())

    if "sidebar" in config and not sidebar_sections:
        raise SystemExit("sidebar is defined but produced no items")

    compile_documents(src_dir, out_dir, calepin_cmd, typ_files, sidebar_sections, template)


if __name__ == "__main__":
    main()
