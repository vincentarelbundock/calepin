#import "card.typ": card
#import "callout.typ": callout
#import "gallery.typ" as gallerymod
#import "columns.typ": columns
#import "lightbox.typ" as lightboxmod
#import "sidenote.typ": sidenote
#import "sidefigure.typ": sidefigure
#import "margin.typ": set-margin-impl
#import "tabs.typ": tabs, tab
#import "target.typ": target

#let gallery = gallerymod.gallery
#let lightbox-image = lightboxmod.lightbox-image
#let lightbox-video = lightboxmod.lightbox-video

#let bind(config) = (
  card: card,
  callout: callout,
  gallery: (items, ..args) => gallerymod._gallery(config, items, ..args),
  columns: columns,
  lightbox-image: (id, src, alt, ..args) => lightboxmod._lightbox-image(config, id, src, alt, ..args),
  lightbox-video: (id, src, ..args) => lightboxmod._lightbox-video(config, id, src, ..args),
  sidenote: sidenote,
  sidefigure: sidefigure,
  set-margin-impl: set-margin-impl,
  tabs: tabs,
  tab: tab,
  target: target,
)
