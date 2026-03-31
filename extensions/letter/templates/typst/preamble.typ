{% raw %}
// letterloom by nandac (Unlicense)
// https://github.com/nandac/letterloom

// ===========================================================================
// validate-inputs.typ
// ===========================================================================

#let validate-length(length-value: none, field-name: none) = {
  if type(length-value) != length {
    panic(str(field-name) + " must be of type length.")
  }
}

#let validate-string(string-data: none, field-name: none, required: true) = {
  if not required and string-data == none { return }
  if required and string-data == none { panic(str(field-name) + " is missing.") }
  if type(string-data) not in (str, content) { panic(str(field-name) + " must be a string or content block.") }
  if string-data in ("", []) { panic(str(field-name) + " is empty.") }
}

#let validate-boolean(boolean-data: none, field-name: none, required: true) = {
  if not required and boolean-data == none { return }
  if required and boolean-data == none { panic(str(field-name) + " is missing.") }
  if type(boolean-data) != bool { panic(str(field-name) + " must be a true or false value.") }
}

#let validate-contact(contact: none, field-name: none) = {
  if contact in (none, ()) { panic(str(field-name) + " is missing.") }
  if type(contact) != dictionary { panic(str(field-name) + " details must be a dictionary with name and address fields.") }
  if "name" not in contact { panic(str(field-name) + " name is missing.") }
  let name = contact.at("name")
  if type(name) not in (str, content) { panic(str(field-name) + " name must be a string or content block.") }
  if name in ("", []) { panic(str(field-name) + " name is empty.") }
  if "address" not in contact { panic(str(field-name) + " address is missing.") }
  let address = contact.at("address")
  if type(address) != content { panic(str(field-name) + " address must be a content block.") }
  if address == [] { panic(str(field-name) + " address is empty.") }
}

#let validate-signatures(signatures: none) = {
  if signatures in (none, ()) { panic("signatures are missing.") }
  if type(signatures) != array { signatures = (signatures, ) }
  for signature in signatures {
    if type(signature) != dictionary { panic("signature must be a dictionary with a name field.") }
    if "name" not in signature { panic("signature name is missing.") }
    let name = signature.at("name")
    if type(name) not in (str, content) { panic("signature name must be a string or content block.") }
    if name in ("", []) { panic("signature name is empty.") }
  }
}

#let validate-attn(attn-name: none, attn-label: none, attn-position: none) = {
  validate-string(string-data: attn-name, field-name: "attn-name")
  if attn-label != "Attn:" { validate-string(string-data: attn-label, field-name: "attn-label") }
  if attn-position != "above" {
    if attn-position not in ("above", "below") { panic("attn-position must be one of above or below.") }
  }
}

#let validate-cc(cc: none, cc-label: none) = {
  if cc in (none, (), "", []) { panic("cc is empty.") }
  if type(cc) != array { cc = (cc, ) }
  for cc-recipient in cc {
    if type(cc-recipient) not in (str, content) { panic("cc recipient must be a string or content block.") }
  }
  if cc-label != "cc:" {
    if type(cc-label) not in (str, content) { panic("cc-label must be a string or content block.") }
    if cc-label in ("", []) { panic("cc-label is empty.") }
  }
}

#let validate-enclosures(enclosures: none, enclosures-label: none) = {
  if enclosures in (none, (), "", []) { panic("enclosures are empty.") }
  if type(enclosures) != array { enclosures = (enclosures, ) }
  for enclosure in enclosures {
    if type(enclosure) not in (str, content) { panic("enclosure must be a string or content block.") }
    if enclosure in ("", []) { panic("empty enclosure item found.") }
  }
  if enclosures-label != "encl:" {
    if type(enclosures-label) not in (str, content) { panic("enclosure label must be a string or content block.") }
    if enclosures-label in ("", []) { panic("enclosure label is empty.") }
  }
}

#let validate-footer(footer: none) = {
  if footer not in (none, ()) {
    if type(footer) != array { footer = (footer, ) }
    for footer-elem in footer {
      if type(footer-elem) != dictionary { panic("footer element must be a dictionary.") }
      if "footer-text" not in footer-elem { panic("footer-text is missing.") }
      let footer-text = footer-elem.at("footer-text")
      if type(footer-text) not in (str, content) { panic("footer-text must be a string or content block.") }
      if "footer-type" in footer-elem {
        let footer-type = footer-elem.at("footer-type")
        if footer-type not in ("url", "email", "string") { panic("footer-type must be one of url, email or string.") }
      }
    }
  }
}

#let validate-inputs(
    from-name: none, from-address: none, to-name: none, to-address: none,
    date: none, salutation: none, subject: none, closing: none,
    signatures: none, signature-alignment: left,
    attn-name: none, attn-label: "Attn:", attn-position: "above",
    cc: none, cc-label: "cc:", enclosures: none, enclosures-label: "encl:",
    footer: none, par-leading: 0.8em, par-spacing: 1.8em, number-pages: false,
    main-font-size: 11pt, footer-font-size: 9pt, footnote-font-size: 7pt,
    from-alignment: right, footnote-alignment: left, link-color: blue,
  ) = {
  validate-string(string-data: from-name, field-name: "from-name")
  validate-string(string-data: from-address, field-name: "from-address")
  validate-string(string-data: to-name, field-name: "to-name")
  validate-string(string-data: to-address, field-name: "to-address")
  validate-string(string-data: date, field-name: "date")
  validate-string(string-data: salutation, field-name: "salutation")
  validate-string(string-data: subject, field-name: "subject")
  validate-string(string-data: closing, field-name: "closing")
  validate-signatures(signatures: signatures)
  validate-length(length-value: main-font-size, field-name: "main-font-size")
  validate-length(length-value: footer-font-size, field-name: "footer-font-size")
  validate-length(length-value: footnote-font-size, field-name: "footnote-font-size")
  validate-length(length-value: par-leading, field-name: "par-leading")
  validate-length(length-value: par-spacing, field-name: "par-spacing")
  if attn-name != none { validate-attn(attn-name: attn-name, attn-label: attn-label, attn-position: attn-position) }
  if cc != none { validate-cc(cc: cc, cc-label: cc-label) }
  if enclosures != none { validate-enclosures(enclosures: enclosures, enclosures-label: enclosures-label) }
  if footer != none { validate-footer(footer: footer) }
  if number-pages != false { validate-boolean(boolean-data: number-pages, field-name: "number-pages", required: false) }
  if type(from-alignment) != alignment { panic("from-alignment must be a valid alignment type.") }
  if type(footnote-alignment) != alignment { panic("footnote-alignment must be a valid alignment type.") }
  if type(signature-alignment) != alignment { panic("signature-alignment must be a valid alignment type.") }
  if type(link-color) != color { panic("link-color must be a valid color type.") }
}

// ===========================================================================
// construct-outputs.typ
// ===========================================================================

#let construct-signatures(signatures: none, signature-alignment: left) = {
  let sigs-per-row = 3
  let blank-space = none
  if type(signatures) != array { signatures = (signatures, ) }
  if signatures.len() < 3 { sigs-per-row = signatures.len() }
  if signatures.len() == 1 { signature-alignment = signature-alignment } else { signature-alignment = left }
  grid(
    columns: 1, rows: auto, row-gutter: 10pt, align: left,
    ..signatures.chunks(sigs-per-row).map(sigs => {
      grid(
        columns: (1fr, ) * sigs-per-row, align: signature-alignment,
        rows: 2, row-gutter: 10pt, column-gutter: 40pt,
        ..sigs.map(signatory => signatory.at("signature", default: rect(height: 40pt, stroke: none)))
          + (blank-space, ) * (sigs-per-row - sigs.len()),
        ..sigs.map(signatory => stack(
            spacing: 10pt, signatory.name,
            signatory.at("title", default: none),
            signatory.at("affiliation", default: none)
          )) + (blank-space, ) * (sigs-per-row - sigs.len()),
      )
    })
  )
}

#let construct-cc(cc: none, cc-label: none) = {
  if cc != none {
    set enum(indent: 15pt)
    cc-label
    if type(cc) != array { cc = (cc, ) }
    for cc-recipient in cc { enum.item(text(cc-recipient)) }
  }
}

#let construct-enclosures(enclosures: none, enclosures-label: none) = {
  if enclosures != none {
    set enum(indent: 15pt)
    enclosures-label
    if type(enclosures) != array { enclosures = (enclosures, ) }
    for enclosure in enclosures { enum.item(text(enclosure)) }
  }
}

#let construct-custom-footer(
  footer: none, footer-font: "DejaVu Sans Mono", footer-font-size: 7pt, link-color: blue
) = {
  if footer not in (none, ()) {
    if type(footer) != array { footer = (footer, ) }
    grid(
      columns: footer.len(), rows: 1, gutter: 20pt,
      ..footer.map(footer-item => {
        let footer-type = footer-item.at("footer-type", default: "string")
        let footer-text = footer-item.at("footer-text")
        if footer-type == "url" {
          text(link(footer-text), font: footer-font, size: footer-font-size, fill: link-color)
        } else if footer-type == "email" {
          text(link("mailto:" + footer-text), font: footer-font, size: footer-font-size, fill: link-color)
        } else {
          text(footer-text, font: footer-font, size: footer-font-size)
        }
      })
    )
  } else { grid() }
}

#let construct-page-numbering(number-pages: false) = {
  if number-pages {
    grid(context(if here().page() > 1 { counter(page).display("1") }))
  } else { grid() }
}

// ===========================================================================
// lib.typ
// ===========================================================================

#let letterloom(
  from-name: none, from-address: none,
  to-name: none, to-address: none,
  date: none, salutation: none, subject: none, closing: none,
  signatures: none, signature-alignment: left,
  attn-name: none, attn-label: "Attn:", attn-position: "above",
  cc: none, cc-label: "cc:", enclosures: none, enclosures-label: "encl:",
  footer: none, paper-size: "a4", margins: auto,
  par-leading: 0.8em, par-spacing: 1.8em, number-pages: false,
  main-font: "Libertinus Serif", main-font-size: 11pt,
  footer-font: "DejaVu Sans Mono", footer-font-size: 9pt,
  footnote-font: "Libertinus Serif", footnote-font-size: 7pt,
  from-alignment: right, footnote-alignment: left, link-color: blue,
  doc
) = {
  validate-inputs(
    from-name: from-name, from-address: from-address,
    to-name: to-name, to-address: to-address,
    date: date, salutation: salutation, subject: subject, closing: closing,
    signatures: signatures, signature-alignment: signature-alignment,
    attn-name: attn-name, attn-label: attn-label, attn-position: attn-position,
    cc: cc, cc-label: cc-label, enclosures: enclosures, enclosures-label: enclosures-label,
    footer: footer, par-leading: par-leading, par-spacing: par-spacing,
    number-pages: number-pages, main-font-size: main-font-size,
    footer-font-size: footer-font-size, footnote-font-size: footnote-font-size,
    from-alignment: from-alignment, footnote-alignment: footnote-alignment,
    link-color: link-color,
  )

  let custom-footer = construct-custom-footer(
    footer: footer, footer-font: footer-font,
    footer-font-size: footer-font-size, link-color: link-color
  )
  let page-numbering = construct-page-numbering(number-pages: number-pages)

  set page(paper: paper-size, margin: margins, footer: align(center, custom-footer + page-numbering))
  set text(font: main-font, size: main-font-size)
  set par(leading: par-leading, spacing: par-spacing)
  show link: set text(fill: link-color)
  set footnote.entry(separator: align(footnote-alignment, line(length: 30% + 0pt, stroke: 0.5pt)))
  show footnote.entry: it => { set align(footnote-alignment); set text(font: footnote-font, size: footnote-font-size); it }

  align(from-alignment, block[
    #set align(left)
    #from-name
    #linebreak()
    #from-address
    #linebreak()
    #v(2pt)
    #date
  ])

  let attn = none
  if attn-name != none { attn = attn-label + " " + attn-name }

  block[
    #v(5pt)
    #set align(left)
    #if attn-position == "above" { text(attn); linebreak() }
    #to-name
    #linebreak()
    #to-address
    #linebreak()
    #if attn-position == "below" { text(attn); linebreak() }
  ]

  v(5pt)
  text(salutation)
  linebreak()
  v(5pt)
  text(subject)
  doc
  linebreak()
  v(5pt)
  text(closing)
  construct-signatures(signatures: signatures, signature-alignment: signature-alignment)
  v(10pt)
  construct-cc(cc: cc, cc-label: cc-label)
  construct-enclosures(enclosures: enclosures, enclosures-label: enclosures-label)
}
{% endraw %}
