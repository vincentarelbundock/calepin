#import "@preview/calepin:0.0.1" as calepin
#import "/assets/site.typ" as site

#set document(title: [Blogue])
#metadata((title: "Blogue", translation_key: "blog", slug: "blogue")) <website-metadata>

#title()

Tous les billets, du plus récent au plus ancien.

#site.listing("Billets", calepin.pages(), kind: "post", language: "fr")
