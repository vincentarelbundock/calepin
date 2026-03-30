{%- set loc = cfg.cap_location | default("bottom") -%}
{%- if cfg.caption %}
#figure(kind: table, [
{{clp.children}}
], caption: [{{cfg.caption}}]) <{{cfg.id}}>
{%- else %}
{{clp.children}} <{{cfg.id}}>
{%- endif %}
