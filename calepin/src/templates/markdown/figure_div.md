{%- set env = cfg.fig_env | default("figure") -%}
{{clp.children}}
{%- if cfg.caption %}

*{{cfg.caption}}*
{%- endif %}
