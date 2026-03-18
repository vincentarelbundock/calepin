#set document(title: "*Calepin* Tutorial (PDF)", author: "Vincent Arel-Bundock, Norah Jones")
#set page(margin: 2.5cm)
#set text(size: 11pt)
#set par(justify: true, leading: 0.65em)
#set heading(numbering: "1.1")

// Heading styles: h1 large smallcaps, h2 italic, h3 small normal
#show heading.where(level: 1): set text(size: 12pt, weight: "regular")
#show heading.where(level: 1): it => {
  v(1.2em)
  block(smallcaps(it.body))
  v(0.4em)
}
#show heading.where(level: 2): set text(size: 10pt, style: "italic", weight: "regular")
#show heading.where(level: 3): set text(size: 9pt, weight: "regular")

// Source code box (light gray, left accent bar)
#let srcbox(body) = block(
  fill: luma(245),
  stroke: (left: 3pt + luma(180)),
  inset: (x: 8pt, y: 6pt),
  radius: 0pt,
  width: 100%,
  body
)

// Output box (white, thin border)
#let outbox(body) = block(
  fill: white,
  stroke: 0.5pt + luma(180),
  inset: (x: 8pt, y: 6pt),
  radius: 0pt,
  width: 100%,
  body
)

#align(center)[
  #text(size: 17pt, weight: "bold")[_Calepin_ Tutorial (PDF)]
  #v(0.3em)
  
  #v(0.5em)
  #text(size: 12pt)[Vincent Arel-Bundock#super[1]#super[\*], Norah Jones#super[2,3]]

  #v(0.3em)
  #text(size: 9pt, style: "italic")[#super[1] Department of Political Science, Université de Montréal, Montreal, Canada \
#super[2] Machine Learning, Carnegie Mellon University, Pittsburgh, PA, USA \
#super[3] University of Chicago, Chicago, IL, USA]
  #v(0.3em)
  #text(size: 10pt, style: "italic")[2026-03-19]
]

#block(fill: luma(240), inset: 1em, width: 100%)[#text(weight: "bold")[Abstract] \
This document renders the _Calepin_ tutorial to PDF via LaTeX. It demonstrates scholarly front matter (authors, affiliations, ORCID), the appendix, and the #raw("include") shortcode for pulling in content from another file.]



This page reviews some of the basics of Quarto and Markdown syntax supported by _Calepin_. It covers document structure, code chunks, figures, math, cross-references, citations, and other features that work out of the box.

= Getting started <getting-started>

_Calepin_ renders #raw(".qmd") documents to HTML, LaTeX, Typst, and Markdown. It executes R code chunks, captures output and plots, resolves citations and cross-references, and wraps everything in a template.

To render this tutorial:


#srcbox[#raw("calepin tutorial.qmd", block: true, lang: "")]

= Markdown basics <markdown-basics>

_Calepin_ supports standard CommonMark and GitHub Flavored Markdown.

Lorem ipsum dolor sit amet, _consectetur_ adipiscing elit. Sed do *eiusmod tempor* incididunt ut labore.

Ordered list:

#enum(
  tight: true,
  [First item],
  [Second item],
  [Third item],
)

Unordered list:

#list(
  tight: true,
  [Alpha],
  [Beta],
  [Gamma],
)

A #link("https://example.com")[hyperlink] and some #raw("inline code").

#quote(block: true)[A blockquote: Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.]

Text with a footnote#footnote[This is the first footnote.] <footnote-1> and another#footnote[This is the second footnote.] <footnote-2>.

Superscript: E = mc#super[2]. Strikethrough: #strike[deleted].

En dash: – and em dash: —

= Code chunks <code-chunks>

Code chunks are fenced with triple backticks and a #raw("{r}") header. Options go inside the chunk as #raw("#|") pipe comments.


#srcbox[#raw("x <- 1:10
mean(x)", block: true, lang: "r")]

#outbox[#raw("[1] 5.5", block: true)]

= Inline code <inline-code>

R expressions can be evaluated inline. The mean of 1:10 is 5.5 and pi is approximately 3.14.

= Figures <figures>

Code chunks that produce plots automatically generate figures. Use #raw("fig-cap") for a caption and a label for cross-referencing.


#srcbox[#raw("plot(
    1:10,
    (1:10)^2,
    main = \"Square Numbers\",
    xlab = \"x\",
    ylab = \"x^2\"
)", block: true, lang: "r")]

#figure(image("pdf_files/scatter-1.png", width: 60%), caption: [A simple scatter plot]) <fig-scatter>

= Cross-references <cross-references>

_Calepin_ resolves references to figures, sections, and theorem environments.

#list(
  tight: true,
  [Figure 1 renders as a linked reference],
  [Section 5 links to the Figures section],
  [1 suppresses the type prefix],
)

Sections get automatic ids from their heading text.

= Math <math>

We can represent dollar amounts using a single dollar sign like 24\$ or enclose math between two dollar signs: $a^2 + b^2 = c^2$.

Display math uses double dollars:

$$
\int_0^1 x^2 \, dx = \frac{1}{3}
$$

== Theorem environments <theorem-environments>

Fenced divs with theorem-type classes get automatic numbering and can be cross-referenced.


#block(width: 100%, above: 1em, below: 1em)[*Theorem 1.* #emph[In a right triangle, the square of the hypotenuse equals the sum of the squares of the other two sides: $a^2 + b^2 = c^2$.
]] <thm-pythagoras>

#block(width: 100%, above: 1em, below: 1em)[_Proof._ Let a right triangle have legs $a$, $b$ and hypotenuse $c$. By constructing squares on each side and comparing areas, we obtain $a^2 + b^2 = c^2$.
 #h(1fr) □]

By Theorem 1, the relationship is fundamental to Euclidean geometry.

The supported theorem types are: #raw("theorem"), #raw("lemma"), #raw("corollary"), #raw("proposition"), #raw("conjecture"), #raw("definition"), #raw("example"), #raw("exercise"), #raw("solution"), #raw("remark"), and #raw("algorithm"). Each type maintains its own counter.

= Callouts <callouts>

Callout divs highlight important information. Five types are available: #raw("callout-note"), #raw("callout-warning"), #raw("callout-tip"), #raw("callout-caution"), and #raw("callout-important"). They can be nested.


#block(fill: rgb("#dbeafe"), stroke: (left: 3pt + rgb("#3b82f6")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[ℹ️ Note] \
  This is a note callout. Use it for supplementary information.


#block(fill: rgb("#fef9c3"), stroke: (left: 3pt + rgb("#eab308")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[⚠️ Warning] \
  Callouts can be nested. This warning appears inside the note above.

]
]

= Conditional content <conditional-content>

Fenced divs with #raw(".content-visible") or #raw(".content-hidden") classes control which content appears in each output format.


#block(inset: 1em)[
This paragraph is hidden in LaTeX but visible in all other formats.

]

Format aliases are supported: #raw("latex") and #raw("pdf") both match LaTeX output, #raw("markdown") and #raw("md") both match Markdown.

= Raw blocks <raw-blocks>

Raw blocks inject format-specific markup that passes through verbatim when the output format matches and is dropped otherwise.


= Shortcodes <shortcodes>

Shortcodes are inline directives written as #raw("{{< name args >}}"). They are processed during rendering and replaced with format-aware output.

== pagebreak <pagebreak>

Insert a native page break. Produces format-appropriate output: #raw("<div style=\"page-break-after: always;\">") in HTML, #raw("\\newpage{}") in LaTeX, #raw("#pagebreak()") in Typst.

Page one content.



Page two content.

== meta <meta>

Print a value from the document’s YAML front matter.

The title of this document is “_Calepin_ Tutorial (PDF)”.

This works with standard fields (#raw("title"), #raw("subtitle"), #raw("author"), #raw("date"), #raw("abstract")) and custom metadata.

== env <env>

Print a system environment variable.

The #raw("SHELL") environment variable on the local machine is: /bin/zsh.

If the variable is not set, the shortcode produces an empty string.

= Citations <citations>

_Calepin_ processes citations from BibTeX files listed in the YAML front matter under #raw("bibliography"). Use #raw("@key") for author-year citations and #raw("[-@key]") for year-only.

As Arel-Bundock et al. (2026) show, political science research is underpowered. This finding has important implications 2026.

= Tabsets <tabsets>

Tabsets organize content into switchable tabs. In HTML, readers click to switch tabs. In LaTeX and Typst, tabs render as regular sections.


== R <r>


#srcbox[#raw("x <- c(1, 2, 3)
mean(x)", block: true, lang: "r")]

#outbox[#raw("[1] 2", block: true)]

== Python <python>


#srcbox[#raw("x = [1, 2, 3]
sum(x) / len(x)", block: true, lang: "python")]

== Julia <julia>


#srcbox[#raw("x = [1, 2, 3]
sum(x) / length(x)", block: true, lang: "julia")]

Tabsets with the same #raw("group") attribute switch together:


== R <r>

R version of example 2.

== Python <python>

Python version of example 2.


== R <r>

R content stays synced with the tabset above.

== Python <python>

Python content stays synced too.


= Line blocks <line-blocks>

Line blocks preserve line breaks and leading spaces. Prefix each line with #raw("|").

The limerick packs laughs anatomical\
Into space that is quite economical.\
   But the good ones I’ve seen\
   So seldom are clean\
And the clean ones so seldom are comical.

= Footnotes <footnotes>

There are two ways to create footnotes. Named references use #raw("[^label]") at the point of reference and #raw("[^label]: text") for the definition elsewhere in the document. Inline footnotes use #raw("^[text]") directly without a separate definition.

Named footnote#footnote[This is defined separately.] <footnote-1> and inline footnote#footnote[This is defined right here.] <footnote-2>.

= Numbered sections <numbered-sections>

Add #raw("number-sections: true") to the YAML front matter to automatically number all section headings.

= The #raw(".hidden") div <the-hidden-div>

The #raw(".hidden") div executes its content but produces no output.


The value computed silently: 42.


= References <references>

Vincent Arel-Bundock, Ryan C Briggs, Hristos Doucouliagos, Marco M Aviña, Tom D Stanley. 2026. Quantitative political science research is greatly underpowered. The Journal of politics


#pagebreak()
= Appendix
== Reuse
#link("https://creativecommons.org/licenses/by/4.0/")[Creative Commons Attribution 4.0]
== Citation
Vincent Arel-Bundock, Norah Jones. "*Calepin* Tutorial (PDF)". #emph[Journal of Reproducible Research]. 1(1). (2026-03). DOI: #link("https://doi.org/10.1234/calepin.2026")[10.1234/calepin.2026]
== Copyright
Copyright 2026 Vincent Arel-Bundock
== Funding
- Social Sciences and Humanities Research Council, Grant #123456, Vincent Arel-Bundock
