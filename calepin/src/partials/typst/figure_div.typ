{%- set env = fig_env | default("figure") -%}
{%- if caption %}
#figure([
{{children}}
], caption: [{{caption}}]) <{{label}}>
{%- else %}
{{children}} <{{label}}>
{%- endif %}
