#import "@preview/calepin:0.0.1" as calepin
#import "/assets/blog-listings.typ": listings

#metadata((title: "Blog")) <website-metadata>

= Blog

#listings(calepin.pages(), pattern: "posts/*")
