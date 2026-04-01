{% include "preamble.typ" %}
{{clp.preamble}}

#align(center)[
  #text(size: 17pt)[{{cfg.title}}]
  #v(0.5em)
  {% include "subtitle.typ" %}
  #v(0.5em)
  {% include "authors.typ" %}
  #v(0.3em)
  #text(size: 10pt)[{{cfg.date}}]
]

{% include "abstract.typ" %}
{% include "keywords.typ" %}
{{clp.toc}}

{{clp.body}}

{{clp.bibliography}}
{{clp.appendix}}
