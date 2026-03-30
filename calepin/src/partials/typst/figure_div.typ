{%- set loc = config.cap_location | default("bottom") -%}
{%- if config.caption %}
#figure(placement: auto, caption: [{{config.caption}}]{% if loc == "top" %}, caption-pos: top{% endif %})[
{{calepin.children}}
] <{{config.label}}>
{%- else %}
{{calepin.children}} <{{config.label}}>
{%- endif %}
