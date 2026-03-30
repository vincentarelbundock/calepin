{% include "preamble.typ" %}
{{calepin.preamble}}

#align(center)[
  #text(size: 20pt, weight: "bold")[{{config.title}}]
  #v(0.5em)
  {% include "subtitle.typ" %}
  #v(0.5em)
  {% include "authors.typ" %}
  #v(0.3em)
  #text(size: 10pt)[{{config.date}}]
]

{% include "abstract.typ" %}
{% include "keywords.typ" %}
{{calepin.toc}}

{{calepin.body}}

{{calepin.bibliography}}
{{calepin.appendix}}
