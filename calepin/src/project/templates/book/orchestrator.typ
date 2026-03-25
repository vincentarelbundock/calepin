{% include "preamble.jinja" %}

#align(center + horizon)[
  #text(size: 24pt, weight: "bold")[{{meta.title}}]
  {% if meta.subtitle %}#v(0.8em)
  #text(size: 14pt)[{{meta.subtitle}}]{% endif %}
  #v(1em)
  #text(size: 14pt)[{{meta.author}}]
]
#pagebreak()

#outline(title: "{{label_contents}}", indent: auto, depth: 2)

#set page(numbering: "1")
#counter(page).update(1)

{% for node in pages %}
{% if node.children %}
#heading(level: 1, numbering: none, outlined: true)[{{node.title}}]
{% for child in node.children %}
#include "{{child.file}}"
{% endfor %}
{% else %}
#include "{{node.file}}"
{% endif %}
{% endfor %}
