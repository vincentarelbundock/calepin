{%- if cfg.abstract %}
#pad(x: 1cm)[
  #align(center)[#smallcaps[Abstract]]
  #v(0.5em)
  #text(size: 9.5pt)[{{cfg.abstract}}]
]
{%- endif %}
