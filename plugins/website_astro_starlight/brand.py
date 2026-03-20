"""_brand.yml parser for Astro/Starlight site builder.

Reads _brand.yml and extracts logo, color, and typography information
for use in Starlight config and CSS generation.

Brand spec: https://posit-dev.github.io/brand-yml/
"""

from pathlib import Path

import yaml


def read_brand(path="_brand.yml"):
    """Read _brand.yml if it exists, return dict or None."""
    p = Path(path)
    if not p.exists():
        return None
    with open(p) as f:
        return yaml.safe_load(f) or {}


def get(d, *keys, default=None):
    for k in keys:
        if not isinstance(d, dict):
            return default
        d = d.get(k)
        if d is None:
            return default
    return d


# ---------------------------------------------------------------------------
# Logo
# ---------------------------------------------------------------------------


def _resolve_logo_path(logo_val, brand):
    """Resolve a logo value to a file path.

    logo_val can be:
      - a string path
      - a named reference to logo.images.{name}
      - a dict with 'path' (and optional 'alt')
    """
    if logo_val is None:
        return None, None

    images = get(brand, "logo", "images", default={})

    if isinstance(logo_val, str):
        # Check if it's a named image reference
        if logo_val in images:
            img = images[logo_val]
            if isinstance(img, dict):
                return img.get("path"), img.get("alt")
            return img, None
        return logo_val, None

    if isinstance(logo_val, dict):
        path = logo_val.get("path")
        alt = logo_val.get("alt")
        # path might be a named reference
        if path and path in images:
            img = images[path]
            if isinstance(img, dict):
                return img.get("path"), alt or img.get("alt")
            return img, alt
        return path, alt

    return None, None


def get_logos(brand):
    """Extract logo paths from brand, returning dict with keys:
    light, dark, alt.  Any may be None.

    Preference order: medium > small > large (per Quarto spec for navbar).
    """
    if brand is None:
        return {}

    logo = get(brand, "logo")
    if logo is None:
        return {}

    # Try medium, small, large in preference order
    for size in ("medium", "small", "large"):
        val = get(logo, size)
        if val is None:
            continue

        # Check for light/dark object
        if isinstance(val, dict) and ("light" in val or "dark" in val):
            light_path, light_alt = _resolve_logo_path(val.get("light"), brand)
            dark_path, dark_alt = _resolve_logo_path(val.get("dark"), brand)
            return {
                "light": light_path,
                "dark": dark_path,
                "alt": light_alt or dark_alt,
            }

        # Single logo
        path, alt = _resolve_logo_path(val, brand)
        if path:
            return {"light": path, "dark": None, "alt": alt}

    # Bare logo: string at top level
    if isinstance(logo, str):
        return {"light": logo, "dark": None, "alt": None}

    return {}


# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------


def _resolve_color(val, palette):
    """Resolve a color value: if it's a palette name, look it up."""
    if val is None:
        return None
    if isinstance(val, str) and not val.startswith("#"):
        return palette.get(val, val)
    if isinstance(val, dict):
        # light/dark variant — resolve both
        return {
            k: palette.get(v, v) if isinstance(v, str) and not v.startswith("#") else v
            for k, v in val.items()
        }
    return val


def get_colors(brand):
    """Extract semantic colors from brand, resolving palette references.

    Returns dict with keys: foreground, background, primary, etc.
    Values are either a string "#hex" or a dict {"light": "#hex", "dark": "#hex"}.
    """
    if brand is None:
        return {}

    color = get(brand, "color", default={})
    palette = get(color, "palette", default={})

    result = {}
    for key in ("foreground", "background", "primary", "secondary",
                "success", "info", "warning", "danger", "light", "dark"):
        val = color.get(key)
        if val is not None:
            result[key] = _resolve_color(val, palette)

    return result


# ---------------------------------------------------------------------------
# Typography
# ---------------------------------------------------------------------------


def get_google_fonts(brand):
    """Extract Google font family names from brand typography."""
    if brand is None:
        return []

    fonts = get(brand, "typography", "fonts", default=[])
    families = []
    for font in fonts:
        if isinstance(font, dict) and font.get("source") == "google":
            family = font.get("family")
            if family:
                families.append(family)
    return families


def get_font_families(brand):
    """Extract font family assignments from brand typography.

    Returns dict with keys: base, headings, monospace (values are family names).
    """
    if brand is None:
        return {}

    typo = get(brand, "typography", default={})
    result = {}
    for key in ("base", "headings", "monospace"):
        val = typo.get(key)
        if isinstance(val, str):
            result[key] = val
        elif isinstance(val, dict) and "family" in val:
            result[key] = val["family"]
    return result


# ---------------------------------------------------------------------------
# CSS generation
# ---------------------------------------------------------------------------


def _color_value(val, mode):
    """Extract color string for a given mode from a color value."""
    if isinstance(val, dict):
        return val.get(mode)
    return val


def brand_css(brand):
    """Generate CSS custom properties from brand colors and typography."""
    if brand is None:
        return ""

    parts = []
    colors = get_colors(brand)
    fonts = get_font_families(brand)
    google_fonts = get_google_fonts(brand)

    # Google font imports
    for family in google_fonts:
        encoded = family.replace(" ", "+")
        parts.append(
            f"@import url('https://fonts.googleapis.com/css2?"
            f"family={encoded}:ital,wght@0,300..900;1,300..900&display=swap');"
        )

    # Starlight maps its accent color from --sl-color-accent
    # and text/bg from --sl-color-text / --sl-color-bg
    # We generate light and dark scoped rules.
    for mode in ("light", "dark"):
        props = []
        primary = _color_value(colors.get("primary"), mode)
        fg = _color_value(colors.get("foreground"), mode)
        bg = _color_value(colors.get("background"), mode)

        if primary:
            props.append(f"  --sl-color-accent: {primary};")
        if fg:
            props.append(f"  --sl-color-text: {fg};")
        if bg:
            props.append(f"  --sl-color-bg: {bg};")

        if props:
            parts.append(f"[data-theme='{mode}'] {{")
            parts.extend(props)
            parts.append("}")

    # Font families
    font_props = []
    if "base" in fonts:
        font_props.append(f"  --sl-font: '{fonts['base']}', sans-serif;")
    if "headings" in fonts:
        font_props.append(f"  --sl-font-headings: '{fonts['headings']}', sans-serif;")
    if "monospace" in fonts:
        font_props.append(f"  --sl-font-mono: '{fonts['monospace']}', monospace;")

    if font_props:
        parts.append(":root {")
        parts.extend(font_props)
        parts.append("}")

    return "\n".join(parts)
