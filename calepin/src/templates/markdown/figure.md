{%- set loc = cfg.cap_location | default("bottom") %}
{%- if cfg.link %}[{%- endif %}
![{{cfg.alt}}]({{clp.src}})
{%- if cfg.link %}]({{cfg.link}}){%- endif %}
{%- if cfg.caption %}

*{{cfg.caption}}*
{%- endif %}
