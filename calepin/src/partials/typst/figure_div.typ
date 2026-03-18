{%- set loc = cap_location | default("bottom") -%}
{%- if caption %}
#figure(placement: auto, caption: [{{caption}}]{% if loc == "top" %}, caption-pos: top{% endif %})[
{{children}}
] <{{label}}>
{%- else %}
{{children}} <{{label}}>
{%- endif %}
