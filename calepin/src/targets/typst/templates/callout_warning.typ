{%- set title = cfg.title -%}
{%- if not title -%}{%- set title = "Warning" -%}{%- endif %}
#block(fill: rgb("#fef9c3"), stroke: (left: 3pt + rgb("#eab308")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
  #text(weight: "bold")[⚠️ {{title}}] \
  {{clp.content}}
]{% if cfg.id %} <{{cfg.id}}>{% endif %}
