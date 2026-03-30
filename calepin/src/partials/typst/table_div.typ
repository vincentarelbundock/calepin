{%- set loc = config.cap_location | default("bottom") -%}
{%- if config.caption %}
#figure(kind: table, [
{{calepin.children}}
], caption: [{{config.caption}}]) <{{config.id}}>
{%- else %}
{{calepin.children}} <{{config.id}}>
{%- endif %}
