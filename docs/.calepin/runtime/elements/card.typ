#import "../core/css.typ": _css-size
#import "../core/target.typ": _is-html, _is-query
#import "target.typ": target

#let _assets-loaded = state("calepin-elements-card-assets", false)

#let _asset_once() = context {
  if _assets-loaded.get() {
    none
  } else {
    _assets-loaded.update(_ => true)
    std.html.elem("style", "
      .calepin-elements-card {
        display: block;
        margin-block: 1rem;
        padding: 1rem;
        overflow: hidden;
        border: 1px solid var(--pico-muted-border-color);
        border-radius: 0.8rem;
        background: var(--pico-background-color);
        color: var(--pico-color);
        box-shadow: 0 0.65rem 1.5rem rgba(15, 23, 42, 0.08);
      }

      a.calepin-elements-card {
        text-decoration: none;
      }

      a.calepin-elements-card:hover,
      a.calepin-elements-card:focus-visible {
        transform: translateY(-1px);
        box-shadow: 0 0.9rem 1.9rem rgba(15, 23, 42, 0.12);
      }
    ")
  }
}

#let _classes(extra) = {
  let classes = "calepin-elements-card"
  if extra != none and extra != "" {
    classes += " " + extra
  }
  classes
}

#let _style(width, style) = {
  let out = if style == none { "" } else { style }
  let css-width = _css-size(width)
  if css-width != none {
    if out != "" and not out.ends-with(";") {
      out += ";"
    }
    out += " max-width: " + css-width + ";"
  }
  out
}

#let _html-card(body, href, class, style, width) = {
  let tag = if href == none { "article" } else { "a" }
  let attrs = (
    class: _classes(class),
    style: _style(width, style),
  )
  if href != none {
    attrs.insert("href", href)
  }
  std.html.elem(tag, attrs: attrs)[#body]
}

#let _paged-card(body, href, width) = {
  let content = block(
    width: width,
    inset: 0.85em,
    radius: 6pt,
    stroke: 0.5pt + luma(75%),
    fill: luma(98%),
  )[#body]

  if href == none {
    content
  } else {
    link(href)[#content]
  }
}

#let card(
  html: none,
  paged: none,
  href: none,
  class: none,
  style: none,
  width: 100%,
  body,
) = {
  let content = target(html: html, paged: paged, fallback: body)

  if _is-query() {
    return content
  }

  if _is-html() {
    return [
      #_asset_once()
      #_html-card(content, href, class, style, width)
    ]
  }

  _paged-card(content, href, width)
}
