{% include "preamble.jinja" %}

#align(center)[
  #text(size: 17pt)[{{title}}]
  #v(0.5em)
  {% include "subtitle.jinja" %}
  #v(0.5em)
  {% include "author_block.jinja" %}
  #v(0.3em)
  #text(size: 10pt)[{{date}}]
]

{% include "abstract.jinja" %}
{% include "keywords.jinja" %}
{{toc}}

{{body}}

{{bibliography_block}}
{{appendix_block}}
