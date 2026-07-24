#import "../core/assets.typ": _image-meta-entry, _js-string-literal
#import "../core/assets.typ": _resolve-asset-href, _resolve-asset-path
#import "../core/css.typ": _css-size
#import "../core/target.typ": _is-html, _is-query

#let _asset-config = state("calepin-elements-gallery-assets", none)

#let _asset_once(styles-url, lightbox-url, module-url) = context {
  let config = (styles-url, lightbox-url, module-url)
  let selected = _asset-config.get()
  if selected != none and selected != config {
    panic("calepin.elements.gallery: every gallery must use the same frontend asset URLs")
  }
  if selected == none {
    let asset-step = _asset-config.update(_ => config)
    [
      #asset-step
      #std.html.elem("link", "", attrs: (
        rel: "stylesheet",
        href: styles-url,
      ))
      #std.html.elem("style", "
        .calepin-elements-gallery {
          list-style: none;
          padding: 0;
        }

        .calepin-elements-gallery__item {
          display: block;
          break-inside: avoid;
          margin-block-end: var(--calepin-elements-gallery-gap, 0.75em);
          border-radius: 0.5rem;
          border: 1px solid var(--pico-muted-border-color);
          overflow: hidden;
          position: relative;
          isolation: isolate;
          text-decoration: none;
          background: var(--pico-background-color);
          box-shadow: 0 0.5rem 1.25rem rgba(15, 23, 42, 0.09);
          color: var(--pico-color);
        }

        .calepin-elements-gallery__image {
          display: block;
          width: 100%;
          height: auto;
        }

        .calepin-elements-gallery__item::after {
          content: \"\";
          pointer-events: none;
          position: absolute;
          inset: 0;
          background: linear-gradient(to top, rgba(15, 23, 42, 0.55), transparent 45%);
          opacity: 0;
          transition: opacity 180ms ease;
        }

        .calepin-elements-gallery__item:hover::after,
        .calepin-elements-gallery__item:focus-visible::after {
          opacity: 1;
        }

        .calepin-elements-gallery__caption {
          position: absolute;
          left: 0.6rem;
          right: 0.6rem;
          bottom: 0.5rem;
          color: white;
          z-index: 1;
          font-size: 0.86rem;
          line-height: 1.2;
          display: none;
        }

        .calepin-elements-gallery__item:hover .calepin-elements-gallery__caption,
        .calepin-elements-gallery__item:focus-visible .calepin-elements-gallery__caption {
          display: block;
        }
      ")
      #std.html.elem("script", "import PhotoSwipeLightbox from " + _js-string-literal(lightbox-url) + ";

        document.addEventListener('DOMContentLoaded', () => {
          const lightbox = new PhotoSwipeLightbox({
            gallery: '.calepin-elements-gallery--lightbox',
            children: 'a',
            pswpModule: () => import(" + _js-string-literal(module-url) + "),
          });
          lightbox.init();
        });
      ", attrs: (type: "module"))
    ]
  } else {
    none
  }
}

#let _dim(value) = {
  if (type(value) == int or type(value) == float) and value > 0 {
    str(value)
  } else if type(value) == str and value != "" {
    value
  } else {
    none
  }
}

#let _entry(config, item) = {
  let src = none
  let alt = ""
  let caption = none
  let width = none
  let height = none

  if type(item) == dictionary {
    src = item.at("src", default: item.at("path", default: none))
    alt = item.at("alt", default: "")
    caption = item.at("caption", default: none)
    width = item.at("width", default: none)
    height = item.at("height", default: none)
  } else if type(item) == array {
    src = item.at(0, default: none)
    alt = item.at(1, default: "")
    caption = item.at(2, default: none)
    width = item.at(3, default: none)
    height = item.at(4, default: none)
  }

  if type(src) != str {
    return none
  }
  if type(alt) != str {
    panic("calepin.elements.gallery: alt must be a string")
  }

  let meta = _image-meta-entry(src, config: config)
  let href = _resolve-asset-href(src, config: config)
  (
    src: src,
    path: _resolve-asset-path(src, config: config),
    href: href,
    alt: alt,
    caption: caption,
    width: _dim(if width == none and meta != none { meta.at("width", default: none) } else { width }),
    height: _dim(if height == none and meta != none { meta.at("height", default: none) } else { height }),
  )
}

#let _entries(config, items) = {
  let raw = if type(items) == array { items } else { (items,) }
  let out = ()
  for item in raw {
    if type(item) == array and (type(item.at(0, default: none)) == array or type(item.at(0, default: none)) == dictionary) {
      for nested in item {
        let entry = _entry(config, nested)
        if entry != none { out.push(entry) }
      }
    } else {
      let entry = _entry(config, item)
      if entry != none { out.push(entry) }
    }
  }
  out
}

#let _style(columns, gap, max-width) = {
  let gap = _css-size(gap)
  let width = _css-size(max-width)
  let style = if type(columns) == int {
    let columns = if columns <= 0 { 3 } else { columns }
    "column-count: " + str(columns) + "; "
  } else if type(columns) == str {
    "display: grid; align-items: start; grid-template-columns: " + columns + "; "
  } else {
    "column-count: 3; "
  }
  if gap != none {
    style += if type(columns) == int {
      "column-gap: " + gap + "; --calepin-elements-gallery-gap: " + gap + "; "
    } else {
      "gap: " + gap + "; "
    }
  }
  if width != none {
    style += "max-width: " + width + "; "
  }
  style + "margin: 1rem auto;"
}

#let _anchor(entry, show-captions) = {
  let attrs = (
    href: entry.href,
    class: "calepin-elements-gallery__item",
    "data-calepin-elements-photo": "",
    "aria-label": "Open image: " + entry.alt,
  )
  if entry.width != none {
    attrs.insert("data-pswp-width", entry.width)
  }
  if entry.height != none {
    attrs.insert("data-pswp-height", entry.height)
  }
  std.html.elem("a", attrs: attrs)[
    #std.html.elem("img", attrs: (
      src: entry.href,
      alt: entry.alt,
      class: "calepin-elements-gallery__image",
      loading: "lazy",
      decoding: "async",
    ))
    #if show-captions and entry.caption != none {
      std.html.elem("span", attrs: (class: "calepin-elements-gallery__caption"))[#entry.caption]
    }
  ]
}

#let _gallery(
  config,
  items,
  columns: 3,
  gap: 0.75em,
  max-width: 42em,
  show-captions: true,
  styles-url: "https://unpkg.com/photoswipe@5.4.4/dist/photoswipe.css",
  lightbox-url: "https://unpkg.com/photoswipe@5.4.4/dist/photoswipe-lightbox.esm.js",
  module-url: "https://unpkg.com/photoswipe@5.4.4/dist/photoswipe.esm.js",
) = {
  if type(show-captions) != bool {
    panic("calepin.elements.gallery: show-captions must be a boolean")
  }
  for (name, value) in (
    ("styles-url", styles-url),
    ("lightbox-url", lightbox-url),
    ("module-url", module-url),
  ) {
    if type(value) != str or value == "" {
      panic("calepin.elements.gallery: " + name + " must be a non-empty string")
    }
  }
  if _is-query() {
    return none
  }

  let entries = _entries(config, items)
  if entries.len() == 0 {
    return if _is-html() { text[No images yet.] } else { none }
  }

  if _is-html() {
    return [
      #_asset_once(styles-url, lightbox-url, module-url)
      #std.html.elem("div", attrs: (
        class: "calepin-elements-gallery calepin-elements-gallery--lightbox",
        data-calepin-elements-gallery: "true",
        style: _style(columns, gap, max-width),
      ))[
        #for entry in entries {
          _anchor(entry, show-captions)
        }
      ]
    ]
  }

  let fallback-columns = if type(columns) == int and columns > 0 { columns } else { 3 }
  grid(
    columns: fallback-columns,
    gutter: gap,
    ..entries.map(entry => {
      let img = image(entry.path, width: 100%, alt: entry.alt)
      if show-captions and entry.caption != none {
        let caption = entry.caption
        [#img #v(0.3em) #text(size: 0.8em, fill: luma(38%))[#caption]]
      } else {
        img
      }
    }),
  )
}

#let gallery(items, ..args) = _gallery(none, items, ..args)
