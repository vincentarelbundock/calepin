{%- set title = cfg.title -%}
{%- if not title -%}{%- set title = "Tip" -%}{%- endif %}
#block(fill: rgb("#dcfce7"), stroke: (left: 3pt + rgb("#22c55e")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[💡 {{title}}] \
  {{clp.content}}
]{% if cfg.id %} <{{cfg.id}}>{% endif %}
