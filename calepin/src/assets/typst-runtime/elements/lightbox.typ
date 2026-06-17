#import "../core/target.typ": _is-html, _is-query
#import "../core/assets.typ": _resolve-asset-path

#let _dialog-close(label) = std.html.elem("button", attrs: (
  rel: "prev",
  type: "button",
  "data-close-dialog": "",
  "aria-label": label,
))

#let lightbox-image(
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
    return image(_resolve-asset-path(src), width: width, alt: alt)
  }

  [
    #std.html.elem("div", attrs: (class: "calepin-screenshot-block"))[
      #std.html.elem("button", attrs: (
        class: "calepin-screenshot-thumb",
        type: "button",
        "data-lightbox-dialog": id,
        "aria-label": open-label,
      ))[
        #std.html.elem("img", attrs: (
          src: src,
          alt: alt,
          class: "calepin-screenshot-thumb__media",
        ))
        #std.html.elem("span", attrs: (
          class: "calepin-screenshot-thumb__zoom",
          "aria-hidden": "true",
        ))[↗]
      ]
    ]
    #std.html.elem("dialog", attrs: (id: id, class: "calepin-screenshot-dialog"))[
      #std.html.elem("article")[
        #std.html.elem("header")[
          #_dialog-close(close-label)
        ]
        #std.html.elem("img", attrs: (
          class: "calepin-screenshot-dialog__media",
          src: src,
          alt: alt,
        ))
      ]
    ]
  ]
}

#let lightbox-video(
  id,
  src,
  poster: none,
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
        #image(_resolve-asset-path(poster), width: width)
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

  let thumb-attrs = (
    class: "calepin-video-thumb__media",
    src: src,
    muted: "",
    playsinline: "",
    preload: "metadata",
  )
  if poster != none {
    thumb-attrs.insert("poster", poster)
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
    #std.html.elem("dialog", attrs: (id: id, class: "calepin-video-dialog"))[
      #std.html.elem("article")[
        #std.html.elem("header")[
          #_dialog-close(close-label)
        ]
        #std.html.elem("video", attrs: (
          class: "calepin-video-dialog__media",
          src: src,
          muted: "",
          autoplay: "",
          controls: "",
          playsinline: "",
          preload: "metadata",
        ))
      ]
    ]
  ]
}
