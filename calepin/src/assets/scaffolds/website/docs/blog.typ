#import "@preview/calepin:0.0.1" as calepin
#import "/assets/site.typ" as site

#set document(title: [Blog])
#metadata((title: "Blog", translation_key: "blog")) <website-metadata>

#title()

All posts, sorted from newest to oldest.

#site.listing("Posts", calepin.pages(), kind: "post", language: "en")
