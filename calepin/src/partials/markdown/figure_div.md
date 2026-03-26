{%- set env = fig_env | default("figure") -%}
{{children}}
{%- if caption %}

*{{caption}}*
{%- endif %}
