// Compatibility facade. Runtime consumers import the focused modules directly;
// these aliases preserve existing imports from `core/state.typ`.
#import "pages.typ" as pagesmod
#import "../notebook/defaults.typ" as defaults
#import "../notebook/chunk-support.typ" as chunksupport
#import "../notebook/result-support.typ" as resultsupport

#let pages = pagesmod.pages
#let _site-root-prefix = pagesmod._site-root-prefix

#let _auto-label-index = defaults._auto-label-index
#let _auto-inline-label-index = defaults._auto-inline-label-index
#let _base-options = defaults._base-options
#let _setup-defaults = defaults._setup-defaults
#let _call-extra-defaults = defaults._call-extra-defaults
#let _auto-call-defaults = defaults._auto-call-defaults
#let _call-defaults = defaults._call-defaults

#let _disable-raw-chunk-transforms = chunksupport._disable-raw-chunk-transforms
#let _relocate-opts = chunksupport._relocate-opts
#let _raw-node = chunksupport._raw-node
#let _raw-text = chunksupport._raw-text
#let _sync-auto-label-counter = chunksupport._sync-auto-label-counter
#let _derive-label = chunksupport._derive-label

#let _select-representation = resultsupport._select-representation
#let _artifact-path = resultsupport._artifact-path
#let _attach-label = resultsupport._attach-label
#let _attach-labels = resultsupport._attach-labels
#let _crossref-labels-for = resultsupport._crossref-labels-for
