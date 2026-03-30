{%- set loc = config.cap_location | default("bottom") %}
#figure(
{%- if config.link %}#link("{{config.link}}")[{%- endif %}
  image("{{calepin.src}}", width: {{calepin.width_attr | default("70%")}}{% if calepin.height_attr %}, height: {{calepin.height_attr}}{% endif %})
{%- if config.link %}]{%- endif %}
{%- if config.caption %}, caption: [{{config.caption}}]{%- endif %}
){% if config.label %} <{{config.label}}>{% endif %}
