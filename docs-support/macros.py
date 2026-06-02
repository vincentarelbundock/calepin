from pathlib import Path
import re

XML_PROLOG_RE = re.compile(r"^\s*<\?xml[^>]*\?>\s*", re.IGNORECASE)
PROJECT_ROOT = Path(__file__).resolve().parents[1]


def define_env(env):
    @env.macro
    def inline_svg(path, cls=""):
        target = (PROJECT_ROOT / path).resolve()
        if not target.is_relative_to(PROJECT_ROOT):
            raise ValueError(f"SVG include path escapes project root: {path}")
        svg = XML_PROLOG_RE.sub("", target.read_text(encoding="utf-8")).strip()
        if cls:
            svg = re.sub(r"<svg\b", f'<svg class="{cls}"', svg, count=1)
        return svg

    @env.macro
    def include_text(path):
        target = (PROJECT_ROOT / path).resolve()
        if not target.is_relative_to(PROJECT_ROOT):
            raise ValueError(f"Text include path escapes project root: {path}")
        return target.read_text(encoding="utf-8").rstrip("\n")
