#import "../core/target.typ": _is-html, _is-query
#import "../core/assets.typ": _resolve-asset-href, _resolve-asset-path

#let _dialog-close(label) = std.html.elem("button", attrs: (
  rel: "prev",
  type: "button",
  "data-close-dialog": "",
  "aria-label": label,
))

#let _dialog(id, class, close-label, body) = {
  std.html.elem("dialog", attrs: (id: id, class: class))[
    #std.html.elem("article")[
      #std.html.elem("header")[
        #_dialog-close(close-label)
      ]
      #body
    ]
  ]
}

#let _lightbox-image(
  config,
  id,
  src,
  alt,
  width: 18em,
  open-label: "Open image preview",
  close-label: "Close image preview",
) = {
  if _is-query() {
    return none
  }
  if not _is-html() {
    return image(_resolve-asset-path(src, config: config), width: width, alt: alt)
  }
  let href = _resolve-asset-href(src, config: config)

  [
    #std.html.elem("div", attrs: (class: "calepin-screenshot-block"))[
      #std.html.elem("button", attrs: (
        class: "calepin-screenshot-thumb",
        type: "button",
        "data-lightbox-dialog": id,
        "aria-label": open-label,
      ))[
        #std.html.elem("img", attrs: (
          src: href,
          alt: alt,
          class: "calepin-screenshot-thumb__media",
        ))
        #std.html.elem("span", attrs: (
          class: "calepin-screenshot-thumb__zoom",
          "aria-hidden": "true",
        ))[↗]
      ]
    ]
    #_dialog(id, "calepin-screenshot-dialog", close-label)[
      #std.html.elem("img", attrs: (
        class: "calepin-screenshot-dialog__media",
        src: href,
        alt: alt,
      ))
    ]
  ]
}

#let lightbox-image(id, src, alt, ..args) = _lightbox-image(none, id, src, alt, ..args)

#let _lightbox-video(
  config,
  id,
  src,
  poster: none,
  alt: none,
  width: 18em,
  open-label: "Open video preview",
  close-label: "Close video preview",
) = {
  if _is-query() {
    return none
  }
  if not _is-html() {
    if poster != none {
      return [
        #image(
          _resolve-asset-path(poster, config: config),
          width: width,
          alt: if alt == none { open-label } else { alt },
        )
        #v(0.25em)
        #text(size: 0.82em, fill: luma(40%))[Video: #src]
      ]
    }
    return box(
      width: width,
      inset: 0.75em,
      radius: 3pt,
      stroke: 0.5pt + luma(70%),
      fill: luma(96%),
    )[
      #text(size: 0.82em, fill: luma(35%))[Video: #src]
    ]
  }

  let href = _resolve-asset-href(src, config: config)

  let thumb-attrs = (
    class: "calepin-video-thumb__media",
    src: href,
    muted: "",
    playsinline: "",
    preload: "metadata",
  )
  if poster != none {
    thumb-attrs.insert("poster", _resolve-asset-href(poster, config: config))
  }

  [
    #std.html.elem("div", attrs: (class: "calepin-video-block"))[
      #std.html.elem("button", attrs: (
        class: "calepin-video-thumb",
        type: "button",
        "data-video-dialog": id,
        "aria-label": open-label,
      ))[
        #std.html.elem("video", attrs: thumb-attrs)
        #std.html.elem("span", attrs: (
          class: "calepin-video-thumb__play",
          "aria-hidden": "true",
        ))[▶]
      ]
    ]
    #_dialog(id, "calepin-video-dialog", close-label)[
      #std.html.elem("video", attrs: (
        class: "calepin-video-dialog__media",
        src: href,
        muted: "",
        autoplay: "",
        controls: "",
        playsinline: "",
        preload: "metadata",
      ))
    ]
  ]
}

#let lightbox-video(id, src, ..args) = _lightbox-video(none, id, src, ..args)
