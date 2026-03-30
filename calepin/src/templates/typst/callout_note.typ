{%- set title = cfg.title -%}
{%- if not title -%}{%- set title = "Note" -%}{%- endif %}
#block(fill: rgb("#dbeafe"), stroke: (left: 3pt + rgb("#3b82f6")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[ℹ️ {{title}}] \
  {{clp.content}}
]{% if cfg.id %} <{{cfg.id}}>{% endif %}
