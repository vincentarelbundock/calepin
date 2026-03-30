{%- set title = cfg.title -%}
{%- if not title -%}{%- set title = "Caution" -%}{%- endif %}
#block(fill: rgb("#fee2e2"), stroke: (left: 3pt + rgb("#ef4444")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[🔥 {{title}}] \
  {{clp.children}}
]{% if cfg.id %} <{{cfg.id}}>{% endif %}
