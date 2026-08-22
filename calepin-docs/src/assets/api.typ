// Default styling for calepin-docs API reference pages.
//
// Generated once, then left alone: regenerating the reference overwrites the
// page files but never this template, so local restyling survives.
//
// Every construct here is chosen to survive both paged and HTML export, so the
// same reference renders in a PDF and on a Calepin website.

#let api-signature(signature) = block(
  width: 100%,
  breakable: false,
  raw(signature, lang: "python", block: true),
)

#let api-entries(title, entries) = {
  if entries.len() == 0 { return }
  heading(level: 4, title)
  for entry in entries {
    let label = if "name" in entry and entry.name != "" {
      if "type" in entry and entry.type != "" {
        [#raw(entry.name) #text(size: 0.9em, raw(entry.type))]
      } else {
        raw(entry.name)
      }
    } else if "type" in entry {
      raw(entry.type)
    } else {
      []
    }
    terms.item(label, entry.desc)
  }
}

#let api-notes(notes) = {
  for note in notes {
    heading(level: 4, note.title)
    note.body
  }
}

#let api-examples(examples) = {
  if examples == none or examples == "" { return }
  heading(level: 4, "Examples")
  raw(examples, lang: "python", block: true)
}

#let api-deprecated(version, note) = {
  if version == none { return }
  block(
    inset: 0.6em,
    radius: 0.2em,
    width: 100%,
    [*Deprecated#if version != "" [ since #version].* #note],
  )
}

#let api-def(
  level: 2,
  kind: none,
  name: "",
  qualname: "",
  signature: none,
  bases: (),
  summary: [],
  description: [],
  deprecated: none,
  deprecated-note: [],
  decorators: (),
  params: (),
  returns: (),
  raises: (),
  attributes: (),
  seealso: (),
  notes: (),
  examples: none,
) = {
  heading(level: level, raw(name))

  if kind != none or qualname != "" {
    text(size: 0.85em, [#kind #raw(qualname)])
    parbreak()
  }

  for decorator in decorators {
    raw("@" + decorator, lang: "python")
    linebreak()
  }

  if bases.len() > 0 {
    text(size: 0.85em, [Bases: #bases.map(raw).join(", ")])
    parbreak()
  }

  if signature != none { api-signature(signature) }

  api-deprecated(deprecated, deprecated-note)

  summary
  if description != [] { parbreak(); description }

  api-entries("Parameters", params)
  api-entries("Returns", returns)
  api-entries("Raises", raises)
  api-entries("Attributes", attributes)
  api-entries("See also", seealso)
  api-notes(notes)
  api-examples(examples)
}

#let api-function(..args) = api-def(level: 2, kind: "function", ..args)
#let api-class(..args) = api-def(level: 2, kind: "class", ..args)
#let api-method(..args) = api-def(level: 3, kind: "method", ..args)

#let api-index(package: "", entries: ()) = {
  heading(level: 1, [#raw(package) API reference])

  for entry in entries {
    terms.item(
      link(entry.file + ".typ", raw(entry.qualname)),
      entry.summary,
    )
  }
}
