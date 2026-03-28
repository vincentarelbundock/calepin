{%- set loc = cap_location | default("bottom") -%}
{%- if caption %}
#figure(kind: table, [
{{children}}
], caption: [{{caption}}]) <{{id}}>
{%- else %}
{{children}} <{{id}}>
{%- endif %}
