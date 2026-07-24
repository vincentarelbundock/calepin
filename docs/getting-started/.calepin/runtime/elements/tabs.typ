#import "../core/target.typ": _is-html, _is-query
#import "../core/assets.typ": _js-string-literal

#let _asset-config = state("calepin-elements-tabs-assets", none)
#let _panel-index = state("calepin-elements-tabs-panel-index", 1)

#let _asset_once(module-url) = context {
  let selected = _asset-config.get()
  if selected != none and selected != module-url {
    panic("calepin.elements.tabs: every tabs container must use the same module-url")
  }
  if selected == none {
    let asset-step = _asset-config.update(_ => module-url)
    [
      #asset-step
      #std.html.elem("script", "import " + _js-string-literal(module-url) + ";

        let syncingTabGroup = false;
        document.addEventListener('wa-tab-show', event => {
          if (syncingTabGroup) return;

          const source = event.target;
          const group = source.dataset.calepinTabGroup;
          const sourceTabs = Array.from(source.querySelectorAll('wa-tab'));
          const activeTab = sourceTabs.find(tab => tab.panel === source.active);
          const key = activeTab?.dataset.calepinTabKey;
          if (!group || !key) return;
          const occurrence = sourceTabs
            .filter(tab => tab.dataset.calepinTabKey === key && !tab.disabled)
            .indexOf(activeTab);

          syncingTabGroup = true;
          try {
            document.querySelectorAll('wa-tab-group[data-calepin-tab-group]').forEach(target => {
              if (target !== source && target.dataset.calepinTabGroup === group) {
                const tab = Array.from(target.querySelectorAll('wa-tab'))
                  .filter(candidate => candidate.dataset.calepinTabKey === key && !candidate.disabled)
                  .at(occurrence);
                if (tab) target.active = tab.panel;
              }
            });
          } finally {
            syncingTabGroup = false;
          }
        });
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
  } else {
    none
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

#let _html-tab(label, name, active, disabled, attrs, panel-attrs, body) = context {
  let panel-name = if name == none {
    _auto-panel-name(label) + "-" + str(_panel-index.get())
  } else {
    name
  }
  let panel-step = if name == none { _panel-index.update(n => n + 1) } else { none }
  // Keep the human label as the synchronization key. The generated panel
  // id is slugged for DOM use, but different labels can share a slug (for
  // example `A B` and `A-B`) and must remain distinct across groups.
  let sync-key = if name == none { label } else { name }
  let active-attr = if active { (active: "") } else { (:) }
  let disabled-attr = if disabled { (disabled: "") } else { (:) }
  let tab-attrs = (
    ..attrs,
    panel: panel-name,
    "data-calepin-tab-key": sync-key,
    ..active-attr,
    ..disabled-attr,
  )
  let panel-attrs = (
    ..panel-attrs,
    name: panel-name,
    ..active-attr,
  )

  [
    #panel-step
    #std.html.elem("wa-tab", attrs: tab-attrs)[#label]
    #std.html.elem("wa-tab-panel", attrs: panel-attrs)[#body]
  ]
}

#let tabs(
  group: none,
  without-scroll-controls: false,
  html-tag: "wa-tab-group",
  html-class: "calepin-elements-tabs",
  html-attrs: (:),
  module-url: "https://ka-f.webawesome.com/webawesome@3.8.0/components/tab-group/tab-group.js",
  body,
) = {
  if group != none and (type(group) != str or group == "") {
    panic("calepin.elements.tabs: group must be none or a non-empty string")
  }
  if type(without-scroll-controls) != bool {
    panic("calepin.elements.tabs: without-scroll-controls must be a boolean")
  }
  if type(html-tag) != str or html-tag == "" {
    panic("calepin.elements.tabs: html-tag must be a non-empty string")
  }
  if type(html-class) != str {
    panic("calepin.elements.tabs: html-class must be a string")
  }
  if type(module-url) != str or module-url == "" {
    panic("calepin.elements.tabs: module-url must be a non-empty string")
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
    let group-attrs = if group == none {
      (:)
    } else {
      ("data-calepin-tab-group": group)
    }
    let attrs = (
      ..html-attrs,
      class: attrs-class,
      ..scroll-attrs,
      ..group-attrs,
    )
    return [
      #_asset_once(module-url)
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
    return _html-tab(label, name, active, disabled, attrs, panel-attrs, body)
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
