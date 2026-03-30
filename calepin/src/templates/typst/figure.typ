{%- set loc = cfg.cap_location | default("bottom") %}
#figure(
{%- if cfg.link %}#link("{{cfg.link}}")[{%- endif %}
  image("{{clp.src}}", width: {{clp.width_attr | default("70%")}}{% if clp.height_attr %}, height: {{clp.height_attr}}{% endif %})
{%- if cfg.link %}]{%- endif %}
{%- if cfg.caption %}, caption: [{{cfg.caption}}]{%- endif %}
){% if cfg.label %} <{{cfg.label}}>{% endif %}
