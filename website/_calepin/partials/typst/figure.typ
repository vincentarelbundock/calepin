{%- set loc = cap_location | default("bottom") %}
#figure(
{%- if link %}#link("{{link}}")[{%- endif %}
  image("{{src}}", width: {{width_attr | default("70%")}}{% if height_attr %}, height: {{height_attr}}{% endif %})
{%- if link %}]{%- endif %}
{%- if caption %}, caption: [{{caption}}]{%- endif %}
){% if label %} <{{label}}>{% endif %}
