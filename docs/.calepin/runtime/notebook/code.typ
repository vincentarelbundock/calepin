#import "../00_syntax-theme.typ": _input-syntax-theme, _output-syntax-theme, _paged-syntax-theme
#import "../core/target.typ": _is-html

#let _block-lang-label(lang) = {
  if lang == none {
    ""
  } else if lang == "r" {
    "R"
  } else {
    lang
  }
}

#let _raw-block(value, lang: none, theme: auto) = {
  raw(value, block: true, lang: lang, theme: theme)
}

#let code-block(
  body,
  fill: rgb("#f7f7f5"),
  stroke: 0.5pt + rgb("#d8d8d2"),
  radius: 2pt,
  inset: (x: 0.65em, y: 0.45em),
  text-fill: rgb("#1f2933"),
  plain: false,
) = {
  let content = if plain {
    body
  } else {
    text(fill: text-fill)[#body]
  }
  block(
    width: 100%,
    fill: fill,
    stroke: stroke,
    radius: radius,
    inset: inset,
  )[
    #content
  ]
}

#let _paged-input-code-block(code, lang: none) = {
  code-block[
    #_raw-block(code, lang: lang, theme: _paged-syntax-theme)
  ]
}

#let _source-block(code, lang: none, theme: _input-syntax-theme) = {
  if _is-html() {
    std.html.elem("div", attrs: (
      class: "sourceCode",
      "data-lang": _block-lang-label(lang),
    ))[
      #_raw-block(code, lang: lang, theme: theme)
    ]
  } else {
    _paged-input-code-block(code, lang: lang)
  }
}

#let _html-themed-raw-block(it) = {
  let lang = if it.has("lang") { it.lang } else { none }
  _source-block(it.text, lang: lang, theme: _input-syntax-theme)
}

#let _input-block(code, lang: none) = {
  _source-block(code, lang: lang, theme: _input-syntax-theme)
}

#let _output-block(output, stream: "stdout") = {
  if _is-html() {
    let class = if stream == "stderr" {
      "cell-output cell-output-stderr"
    } else {
      "cell-output cell-output-stdout"
    }
    std.html.elem("div", attrs: (class: class))[
      #_raw-block(output, theme: _output-syntax-theme)
    ]
  } else {
    let fill = if stream == "stderr" {
      rgb("#fffaf7")
    } else {
      rgb("#fbfbfa")
    }
    let stroke = if stream == "stderr" {
      (
        rest: 0.5pt + rgb("#e2c7ba"),
        left: 1.5pt + rgb("#c48672"),
      )
    } else {
      (
        rest: 0.5pt + rgb("#ddddda"),
        left: 1.5pt + rgb("#cfcfc8"),
      )
    }
    code-block(
      fill: fill,
      stroke: stroke,
      radius: 2pt,
      inset: (x: 0.65em, y: 0.4em),
      plain: true,
    )[
      #if stream == "stderr" {
        text(fill: rgb("#5f3328"))[
          #_raw-block(output, theme: _paged-syntax-theme)
        ]
      } else {
        _raw-block(output, theme: _paged-syntax-theme)
      }
    ]
  }
}
