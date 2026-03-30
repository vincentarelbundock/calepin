#set document(title: "Basics", author: "")
#set text(size: 11pt)
#set par(justify: true, leading: 0.65em)
#set heading(numbering: "1.1")

#let srcbox(body) = block(
  stroke: 0.4pt + luma(200),
  inset: (x: 8pt, y: 6pt),
  width: 100%,
  body
)

#let outbox(body) = block(
  stroke: 0.4pt + luma(200),
  inset: (x: 8pt, y: 6pt),
  width: 100%,
  body
)


#align(center)[
  #text(size: 17pt)[Basics]
  #v(0.5em)
  #text(size: 13pt)[]
  #v(0.5em)
  #text(size: 12pt)[]
  #v(0.3em)
  #text(size: 10pt)[]
]





This page reviews some of the basics of Quarto and Markdown syntax supported by _Calepin_. It covers document structure, code chunks, figures, math, cross-references, citations, and other features that work out of the box.

== Getting started <getting-started>
_Calepin_ renders `.qmd` documents to HTML, LaTeX, Typst, and Markdown. It executes R code chunks, captures output and plots, resolves citations and cross-references, and wraps everything in a template.

To render this tutorial:




#srcbox[#raw("calepin tutorial.qmd", block: true, lang: "")]

== Markdown basics <markdown-basics>
_Calepin_ supports standard CommonMark and GitHub Flavored Markdown. The table below summarizes the basic Markdown notation from the original specification.

#table(
  columns: 2,
  align: (auto, auto),
  [Element],
  [Syntax],
  [Heading],
  [`# H1`, `## H2`, …, `###### H6`],
  [Paragraph],
  [Blank line between blocks of text],
  [Line break],
  [Two trailing spaces or `\` at end of line],
  [Italic],
  [`*text*` or `_text_`],
  [Bold],
  [`**text**` or `__text__`],
  [Bold + italic],
  [`***text***`],
  [Blockquote],
  [`> quoted text`],
  [Ordered list],
  [`1. item`],
  [Unordered list],
  [`- item`, `* item`, or `+ item`],
  [Inline code],
  [```code```],
  [Code block],
  [Indent by 4 spaces or fence with ```````],
  [Horizontal rule],
  [`---`, `***`, or `___`],
  [Link],
  [`[text](url)` or `[text](url "title")`],
  [Reference link],
  [`[text][id]` then `[id]: url`],
  [Image],
  [`![alt text](path)`],
  [Escape],
  [`\*`, `\[`, `\\`, etc.],
)

Here is a paragraph about Montreal that illustrates all of these features.

#line(length: 100%)

=== Montréal <montréal>
#quote(block: true)[
_Montréal is a city of contrasts: old-world charm meets modern ambition._

]

Montréal is the largest city in the province of Québec and the _second most populous_ in Canada. Founded in 1642, the city sits on an island in the _Saint Lawrence River_. Visitors should explore these landmarks:


- Old Montréal: cobblestone streets and the #link("https://www.basiliquenotredame.ca")[Notre-Dame Basilica]


- Mount Royal: the park designed by Frederick Law Olmsted


- The Plateau: colourful row houses and lively cafés



Local specialties worth trying:


- Poutine


- Smoked meat sandwiches


- Bagels from #link("https://www.stviateurbagel.com")[St-Viateur]



The city’s motto is `Concordia Salus`#footnote[The motto was adopted in 1833 and appears on the city’s coat of arms.], meaning “well-being through harmony.”

As the Montréal tourism board puts it:\
“_There is no place quite like it._”



#box(image("/assets/placeholder.png"))== Code chunks <code-chunks>
Code chunks are fenced with triple backticks and a `{r}` header. Options go inside the chunk as `#|` pipe comments. See the #link("/code/chunks.html")[Chunks] vignette for the full list of chunk options.




#srcbox[#raw("```{r, chunk-1}
#| echo = fenced
x <- 1:10
mean(x)
```", block: true, lang: "r")]

#outbox[#raw("> [1] 5.5", block: true)]

== Inline code <inline-code>
R expressions can be evaluated inline using back ticks and curly braces. See the #link("/code/inline.html")[Inline] vignette for more details.




#srcbox[#raw("`{r} round(pi, 4)`", block: true, lang: "")]

The numbers in this sentence are computed dynamically: The mean of 1:10 is 5.5 and pi is approximately 3.14.

== Figures <figures>



#srcbox[#raw("plot(
    1:10,
    (1:10)^2,
    main = \"Square Numbers\",
    xlab = \"x\",
    ylab = \"x^2\"
)", block: true, lang: "r")]


#figure(
  image("_calepin/files/fig-scatter-1.svg", width: 70%), caption: [A simple scatter plot]
) <fig-scatter>

== Cross-references <cross-references>
_Calepin_ resolves references to figures, sections, and theorem environments.


- \Figure 1 renders as a linked reference


- \Section 4 links to the Figures section


- [-\Figure 1] suppresses the type prefix



Sections get automatic ids from their heading text.

== Math <math>
Inline math: . Display math:





#block(width: 100%, above: 1em, below: 1em)[*Theorem 1.* #emph[In a right triangle, the square of the hypotenuse equals the sum of the squares of the other two sides: .

]] <thm-pythagoras>

#block(width: 100%, above: 1em, below: 1em)[_Proof._ Let a right triangle have legs ,  and hypotenuse . By constructing squares on each side and comparing areas, we obtain .

 #h(1fr) □]

By \Theorem 1, the relationship is fundamental to Euclidean geometry.

== Callouts <callouts>
Callout divs highlight important information. Five types are available: `callout-note`, `callout-warning`, `callout-tip`, `callout-caution`, and `callout-important`. They can be nested.




#block(fill: rgb("#dbeafe"), stroke: (left: 3pt + rgb("#3b82f6")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[ℹ️ Note] \
  This is a note callout. Use it for supplementary information.




#block(fill: rgb("#fef9c3"), stroke: (left: 3pt + rgb("#eab308")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[⚠️ Warning] \
  Callouts can be nested. This warning appears inside the note above.


]
]

== Conditional content <conditional-content>
Fenced divs with `.content-visible` or `.content-hidden` classes control which content appears in each output format.



#block(inset: 1em)[
This paragraph is hidden in LaTeX but visible in all other formats.


]

Format aliases are supported: `latex` and `pdf` both match LaTeX output, `markdown` and `md` both match Markdown.

== Raw blocks <raw-blocks>
Raw blocks inject format-specific markup that passes through verbatim when the output format matches and is dropped otherwise.



== Tabsets <tabsets>
Tabsets organize content into switchable tabs. In HTML, readers click to switch tabs. In LaTeX and Typst, tabs render as regular sections.



=== R <r>



#srcbox[#raw("x <- c(1, 2, 3)
mean(x)", block: true, lang: "r")]

#outbox[#raw("> [1] 2", block: true)]

=== Python <python>



#srcbox[#raw("x = [1, 2, 3]
sum(x) / len(x)", block: true, lang: "python")]

=== Julia <julia>



#srcbox[#raw("x = [1, 2, 3]
sum(x) / length(x)", block: true, lang: "julia")]

Tabsets with the same `group` attribute switch together:



=== R <r>
R version of example 2.

=== Python <python>
Python version of example 2.



=== R <r>
R content stays synced with the tabset above.

=== Python <python>
Python content stays synced too.



== Line blocks <line-blocks>
Line blocks preserve line breaks and leading spaces. Prefix each line with `|`.

The limerick packs laughs anatomical\
Into space that is quite economical.\
   But the good ones I’ve seen\
   So seldom are clean\
And the clean ones so seldom are comical.

== Footnotes <footnotes>
There are two ways to create footnotes. Named references use `[^label]` at the point of reference and `[^label]: text` for the definition elsewhere in the document. Inline footnotes use `^[text]` directly without a separate definition.

Named footnote#footnote[This is defined separately.] and inline footnote#footnote[This is defined right here.].

== Numbered sections <numbered-sections>
Add `number-sections: true` to the YAML front matter to automatically number all section headings.

== The `.hidden` div <the-div>
The `.hidden` div executes its content but produces no output.



The value computed silently: \_\_CALEPIN\_e102\_2\_\_\_ERROR:object ‘secret’ not found.




