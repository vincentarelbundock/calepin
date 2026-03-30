{%- if cfg.caption %}
#figure(placement: auto, caption: [{{cfg.caption}}])[
{{clp.content}}
] <{{cfg.id}}>
{%- else %}
{{clp.content}} <{{cfg.id}}>
{%- endif %}
