{% include "preamble.typ" %}
{{preamble}}

#align(center)[
  #text(size: 17pt)[{{title}}]
  #v(0.5em)
  {% include "subtitle.typ" %}
  #v(0.5em)
  {% include "authors.typ" %}
  #v(0.3em)
  #text(size: 10pt)[{{date}}]
]

{% include "abstract.typ" %}
{% include "keywords.typ" %}
{{toc}}

{{body}}

{{bibliography}}
{{appendix}}
