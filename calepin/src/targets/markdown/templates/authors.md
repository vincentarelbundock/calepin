{%- for a in cfg.authors %}{{ a.name }}{%- if not loop.last %}, {% endif %}{%- endfor %}
{%- if cfg.affiliations %}
{% for a in cfg.affiliations %}{%- if a.number %}{{ a.number }}. {% endif %}{{ a.display }}
{% endfor %}{%- endif %}
{%- for a in cfg.authors %}{% if a.corresponding and a.email %}
* Corresponding author: {{ a.email }}
{%- endif %}{% endfor %}
