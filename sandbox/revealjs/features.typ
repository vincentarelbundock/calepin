# Inventory: Touying vs Reveal

== Scope

Two package inventories in this workspace:

- `touying` (Typst)
- `reveal.js` (JavaScript)

== Touying: user-facing functions

Top-level exported functions from package entrypoint (`src/exports.typ`):

- `alert`
- `alternatives`
- `alternatives-cases`
- `alternatives-fn`
- `alternatives-match`
- `appendix`
- `effect`
- `empty-slide`
- `from-wp`
- `get-first`
- `get-last`
- `handout-only`
- `item-by-item`
- `item-by-item-fn`
- `item-by-item-functions`
- `jump`
- `lr-navigation`
- `meanwhile`
- `next-wp`
- `not-wp`
- `only`
- `pause`
- `prev-wp`
- `slide`
- `speaker-note`
- `touying-diagram`
- `touying-equation`
- `touying-fn-wrapper`
- `touying-fn-wrapper-raw`
- `touying-mitex`
- `touying-raw`
- `touying-recall`
- `touying-reduce`
- `touying-reducer`
- `touying-set-config`
- `touying-slide`
- `touying-slide-wrapper`
- `uncover`
- `until-wp`
- `waypoint`
- `config-colors`
- `config-common`
- `config-info`
- `config-methods`
- `config-page`
- `config-store`
- `default-config`
- `touying-get-config`
- `touying-slides`
- `cols`
- `lazy-h`
- `lazy-layout`
- `lazy-v`
- `side-by-side`
- `touying-enable-warnings`
- `touying-disable-warnings`

Namespace re-exports:

- `utils`
- `components`
- `pdfpc`
- `magic`
- `slides`

== Touying: user-facing config

`config-common` keys:

- `align-enum-marker-with-baseline`
- `align-list-marker-with-baseline`
- `appendix`
- `auto-offset-for-heading`
- `breakable`
- `clip`
- `detect-overflow`
- `enable-frozen-states-and-counters`
- `default-frozen-states`
- `default-frozen-counters`
- `frozen-states`
- `frozen-counters`
- `freeze-slide-counter`
- `handout`
- `handout-subslides`
- `horizontal-line-to-pagebreak`
- `label-only-on-last-subslide`
- `default-composer`
- `slide-fn`
- `new-section-slide-fn`
- `new-subsection-slide-fn`
- `new-subsubsection-slide-fn`
- `new-subsubsubsection-slide-fn`
- `receive-body-for-new-section-slide-fn`
- `receive-body-for-new-subsection-slide-fn`
- `receive-body-for-new-subsubsection-slide-fn`
- `receive-body-for-new-subsubsubsection-slide-fn`
- `show-strong-with-alert`
- `datetime-format`
- `zero-margin-header`
- `zero-margin-footer`
- `enable-pdfpc`
- `enable-mark-warning`
- `reset-page-counter-to-slide-counter`
- `show-only-notes`
- `show-notes-on-second-screen`
- `nontight-list-enum-and-terms`
- `scale-list-items`
- `show-hide-set-list-marker-none`
- `show-bibliography-as-footnote`
- `default-preamble`
- `slide-preamble`
- `subslide-preamble`
- `page-preamble`
- `default-slide-preamble`
- `default-subslide-preamble`
- `default-page-preamble`
- `preamble`

`config-info` keys:

- `author`
- `contact`
- `date`
- `institution`
- `logo`
- `short-subtitle`
- `short-title`
- `subtitle`
- `title`
- `extra`

`config-colors` groups:

- neutrals: `neutral`, `neutral-light`, `neutral-lighter`, `neutral-lightest`, `neutral-dark`, `neutral-darker`, `neutral-darkest`
- primaries: `primary`, `primary-light`, `primary-lighter`, `primary-lightest`, `primary-dark`, `primary-darker`, `primary-darkest`
- secondaries: `secondary`, `secondary-light`, `secondary-lighter`, `secondary-lightest`, `secondary-dark`, `secondary-darker`, `secondary-darkest`
- tertiaries: `tertiary`, `tertiary-light`, `tertiary-lighter`, `tertiary-lightest`, `tertiary-dark`, `tertiary-darker`, `tertiary-darkest`

`config-methods` keys:

- `init`
- `cover`
- `uncover`
- `only`
- `effect`
- `alternatives-match`
- `alternatives`
- `alternatives-fn`
- `alternatives-cases`
- `item-by-item`
- `alert`
- `show-only-notes`
- `convert-label-to-short-heading`

`config-page` keys:

- `paper`
- `header`
- `footer`
- `fill`
- `margin`
- `numbering`

`config-store` is a free-form dictionary namespace for theme-specific keys.

== Reveal.js: user-facing functions and API surface

Runtime exports:

- `Reveal` constructor (`new Reveal(...)`)
- `Reveal.VERSION`
- `initialize`
- `configure`
- `destroy`
- `sync`, `syncSlide`, `syncFragments`, `removeHiddenSlides`
- `slide`
- `left`, `right`, `up`, `down`
- `prev`, `next`
- `navigateLeft`, `navigateRight`, `navigateUp`, `navigateDown`
- `navigatePrev`, `navigateNext`
- `navigateFragment`, `prevFragment`, `nextFragment`
- `toggleHelp`, `toggleOverview`, `togglePause`, `toggleAutoSlide`
- `addKeyBinding`, `removeKeyBinding`, `triggerKey`, `registerKeyboardShortcut`
- `getState`, `setState`, `getProgress`, `getIndices`
- `availableRoutes`, `availableFragments`
- `isFirstSlide`, `isLastSlide`, `isLastVerticalSlide`, `isVerticalSlide`
- `isPaused`, `isAutoSliding`, `isSpeakerNotes`, `isOverview`, `isFocused`
- `isPrintingPDF`, `isReady`
- `layout`
- `shuffle`
- `loadSlide`, `unloadSlide`
- `showPreview`, `hidePreview`
- `addEventListeners`, `removeEventListeners`
- `dispatchEvent`
- `getSlidesAttributes`, `getSlidePastCount`, `getTotalSlides`
- `getSlide`, `getPreviousSlide`, `getCurrentSlide`, `getSlideBackground`, `getSlideNotes`
- `getSlides`, `getHorizontalSlides`, `getVerticalSlides`
- `hasHorizontalSlides`, `hasVerticalSlides`, `hasNavigatedHorizontally`, `hasNavigatedVertically`
- `getComputedSlideSize`, `getScale`
- `getConfig`, `getQueryHash`, `getSlidePath`
- `getRevealElement`, `getSlidesElement`, `getViewportElement`, `getBackgroundsElement`
- Plugin helpers: `registerPlugin`, `hasPlugin`, `getPlugin`, `getPlugins`
- Event methods: `on`, `off`, `addEventListener`, `removeEventListener`

Type exports:

- `RevealConfig`
- `TransitionStyle`
- `TransitionSpeed`
- `FragmentAnimation`
- `KatexConfig`
- `Mathjax2Config`
- `Mathjax3Config`
- `Mathjax4Config`
- `HighlightConfig`
- `MarkdownConfig`

`RevealConfig` includes these user-facing options:

- `width`, `height`, `margin`, `minScale`, `maxScale`
- `controls`, `controlsTutorial`, `controlsLayout`, `controlsBackArrows`
- `progress`, `slideNumber`, `showSlideNumber`
- `hashOneBasedIndex`, `hash`, `respondToHashChanges`, `jumpToSlide`, `history`
- `keyboard`, `keyboardCondition`
- `disableLayout`, `overview`, `center`, `touch`, `loop`, `rtl`, `navigationMode`, `shuffle`
- `fragments`, `fragmentInURL`, `embedded`, `help`, `pause`, `showNotes`, `showHiddenSlides`
- `autoPlayMedia`, `preloadIframes`, `preventIframeAutoFocus`
- `autoAnimate`, `autoAnimateMatcher`, `autoAnimateEasing`, `autoAnimateDuration`, `autoAnimateUnmatched`, `autoAnimateStyles`
- `autoSlide`, `autoSlideStoppable`, `autoSlideMethod`, `defaultTiming`
- `mouseWheel`, `previewLinks`, `postMessage`, `postMessageEvents`, `focusBodyOnPageVisibilityChange`
- `transition`, `transitionSpeed`, `backgroundTransition`
- `parallaxBackgroundImage`, `parallaxBackgroundSize`, `parallaxBackgroundRepeat`, `parallaxBackgroundPosition`, `parallaxBackgroundHorizontal`, `parallaxBackgroundVertical`
- `view`, `scrollLayout`, `scrollSnap`, `scrollProgress`, `scrollActivationWidth`
- `pdfMaxPagesPerSlide`, `pdfSeparateFragments`, `pdfPageHeightOffset`
- `viewDistance`, `mobileViewDistance`, `display`, `hideInactiveCursor`, `hideCursorTime`, `sortFragmentsOnSync`
- `highlight`, `markdown`, `katex`, `mathjax2`, `mathjax3`, `mathjax4`
- `dependencies`, `plugins`

== Comparative notes for a common UI

- Touying: compile-time macro model with slide-level composition (`slide-level`, waypoints, reveal-like macros) and rich presentation metadata.
- Reveal: runtime deck model with direct navigation/mode API (`slide`, `left`, `next`, `is*` state checks) and a mostly flat config map.
- Good shared UI crosswalk:
  - transitions (`transition`, `transitionSpeed`)  
  - deck layout sizing (`width`, `height`, `margin`)  
  - controls/progress/note visibility (`controls`, `progress`, `showNotes`, `showSlideNumber`)  
  - fragment behavior (`fragments`, `fragmentInURL`, auto-fragment options)  
  - plugin/config objects (`markdown`, `katex`, `mathjax*`, `highlight`)  
- Touying-only UI sections:
  - `config-info` (title/subtitle/author metadata)
  - `config-colors` (theme color palette)
  - `config-methods` (custom animation wrappers)
  - heading-to-slide shaping (`slide-level`, `config-common`)

If you also want Reveal React wrapper coverage (`Deck`, `Slide`, `Stack`, `Fragment`, `Markdown`, `Code` components and props), I can add that as a third comparison layer.
