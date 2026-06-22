#import "../core/target.typ": _is-html, _is-query

#let _assets-loaded = state("calepin-elements-tabs-assets", false)

#let _asset_once() = context {
  if _assets-loaded.get() {
    none
  } else {
    _assets-loaded.update(_ => true)
    [
      #std.html.elem("script", "
        import 'https://ka-f.webawesome.com/webawesome@3.8.0/components/tab-group/tab-group.js';
      ", attrs: (type: "module"))
      #std.html.elem("style", "
        wa-tab-group.calepin-elements-tabs {
          display: block;
          margin-block: 1rem;
          --indicator-color: transparent;
          --track-color: var(--pico-muted-border-color, #d0d7de);
          --track-width: 1px;
        }

        wa-tab-group.calepin-elements-tabs::part(nav) {
          border-bottom: 1px solid var(--pico-muted-border-color, #d0d7de);
        }

        wa-tab-group.calepin-elements-tabs::part(body) {
          padding-block-start: 0.75rem;
        }

        wa-tab-group.calepin-elements-tabs wa-tab {
          border-bottom: 0.1875rem solid transparent;
          margin-block-end: -1px;
        }

        wa-tab-group.calepin-elements-tabs wa-tab[active] {
          border-bottom-color: var(--pico-primary, #0172ad);
        }
      ")
    ]
  }
}

#let _assert-dict(name, value) = {
  if type(value) != dictionary {
    panic(name + " must be a dictionary")
  }
}

#let _auto-panel-name(label) = {
  let slug = label.trim().replace(regex("[^A-Za-z0-9_-]+"), "-")
  if slug == "" {
    "calepin-tab"
  } else {
    "calepin-tab-" + slug
  }
}

#let tabs(
  without-scroll-controls: false,
  html-tag: "wa-tab-group",
  html-class: "calepin-elements-tabs",
  html-attrs: (:),
  body,
) = {
  if type(without-scroll-controls) != bool {
    panic("calepin.elements.tabs: without-scroll-controls must be a boolean")
  }
  if type(html-tag) != str or html-tag == "" {
    panic("calepin.elements.tabs: html-tag must be a non-empty string")
  }
  if type(html-class) != str {
    panic("calepin.elements.tabs: html-class must be a string")
  }
  _assert-dict("calepin.elements.tabs: html-attrs", html-attrs)

  if _is-query() {
    return body
  }

  if _is-html() {
    let classes = html-attrs.at("class", default: "")
    let attrs-class = if classes == "" {
      html-class
    } else if html-class == "" {
      classes
    } else {
      html-class + " " + classes
    }
    let scroll-attrs = if without-scroll-controls {
      ("without-scroll-controls": "")
    } else {
      (:)
    }
    let attrs = (
      ..html-attrs,
      class: attrs-class,
      ..scroll-attrs,
    )
    return [
      #_asset_once()
      #std.html.elem(html-tag, attrs: attrs)[#body]
    ]
  }

  body
}

#let tab(
  label,
  name: none,
  active: false,
  disabled: false,
  attrs: (:),
  panel-attrs: (:),
  body,
) = {
  if type(label) != str or label == "" {
    panic("calepin.elements.tab: label must be a non-empty string")
  }
  if name != none and (type(name) != str or name == "") {
    panic("calepin.elements.tab: name must be none or a non-empty string")
  }
  if type(active) != bool {
    panic("calepin.elements.tab: active must be a boolean")
  }
  if type(disabled) != bool {
    panic("calepin.elements.tab: disabled must be a boolean")
  }
  _assert-dict("calepin.elements.tab: attrs", attrs)
  _assert-dict("calepin.elements.tab: panel-attrs", panel-attrs)

  if _is-query() {
    return body
  }

  if _is-html() {
    let panel-name = if name == none {
      _auto-panel-name(label)
    } else {
      name
    }
    let active-attr = if active { (active: "") } else { (:) }
    let disabled-attr = if disabled { (disabled: "") } else { (:) }
    let tab-attrs = (
      ..attrs,
      panel: panel-name,
      ..active-attr,
      ..disabled-attr,
    )
    let panel-attrs = (
      ..panel-attrs,
      name: panel-name,
      ..active-attr,
    )

    return [
      #std.html.elem("wa-tab", attrs: tab-attrs)[#label]
      #std.html.elem("wa-tab-panel", attrs: panel-attrs)[#body]
    ]
  }

  if disabled {
    return none
  }

  block(width: 100%, breakable: true)[
    #strong[#label]
    #v(0.35em)
    #body
  ]
}
