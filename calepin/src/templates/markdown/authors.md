{%- for a in cfg.authors %}{{ a.name }}{%- if not loop.last %}, {% endif %}{%- endfor %}
{%- if cfg.affiliations %}
{% for a in cfg.affiliations %}{%- if a.number %}{{ a.number }}. {% endif %}{{ a.display }}
{% endfor %}{%- endif %}
{%- for c in cfg.corresponding %}
* Corresponding author: {{ c.email }}
{%- endfor %}
