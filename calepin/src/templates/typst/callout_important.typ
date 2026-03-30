{%- set title = cfg.title -%}
{%- if not title -%}{%- set title = "Important" -%}{%- endif %}
#block(fill: rgb("#ede9fe"), stroke: (left: 3pt + rgb("#8b5cf6")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[❗ {{title}}] \
  {{clp.children}}
]{% if cfg.id %} <{{cfg.id}}>{% endif %}
