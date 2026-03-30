{%- set env = config.fig_env | default("figure") -%}
{{calepin.children}}
{%- if config.caption %}

*{{config.caption}}*
{%- endif %}
