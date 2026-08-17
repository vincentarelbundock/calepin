#set document(title: [Multilingual sites])
#metadata((
  title: "Multilingual",
  tags: ("websites", "localization"),
  summary: "Build a site in several languages: one content directory per language, pages linked as translations of each other, and language-code URL prefixes.",
)) <website-metadata>

#title()

Configure languages with one content directory per language:

```toml
default-language = "en"

[languages.en]
label = "English"
content-dir = "."

[languages.fr]
label = "Français"
content-dir = "fr"
```

With this layout, `about.typ` and `fr/about.typ` are treated as translations of the same page. The default language keeps root URLs like `about.html`; other languages use their language code as a URL prefix, such as `fr/about.html`.

Use page metadata when translations move or need different slugs:

```typ
#set document(title: [À propos])

#metadata((
  translation_key: "about",
  slug: "a-propos",
)) <website-metadata>

#title()
```

When more than one language is configured, the bundled themes show a language picker. Local navigation links are shown only for the current language; external links stay global.
