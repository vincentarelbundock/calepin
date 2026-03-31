#text(size: 12pt)[{% for a in cfg.authors %}{{ a.name }}{%- if a.superscripts %}#super[{{ a.superscripts }}]{% endif %}{%- if a.corresponding %}#super[\*]{% endif %}{%- if not loop.last %}, {% endif %}{%- endfor %}]
{%- if cfg.affiliations %}

  #v(0.3em)
  #text(size: 9pt, style: "italic")[{% for a in cfg.affiliations %}{%- if a.number %}#super[{{ a.number }}] {% endif %}{{ a.display }}{%- if not loop.last %} \
{% endif %}{%- endfor %}]
{%- endif %}
