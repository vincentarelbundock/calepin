{%- set loc = cfg.cap_location | default("bottom") -%}
{%- if cfg.caption %}
#figure(placement: auto, caption: [{{cfg.caption}}]{% if loc == "top" %}, caption-pos: top{% endif %})[
{{clp.children}}
] <{{cfg.label}}>
{%- else %}
{{clp.children}} <{{cfg.label}}>
{%- endif %}
