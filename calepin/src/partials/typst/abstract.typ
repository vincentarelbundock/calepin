{%- if config.abstract %}
#pad(x: 1cm)[
  #align(center)[#smallcaps[{{config.label_abstract}}]]
  #v(0.5em)
  #text(size: 9.5pt)[{{config.abstract}}]
]
{%- endif %}
