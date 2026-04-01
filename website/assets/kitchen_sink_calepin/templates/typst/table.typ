{%- if cfg.caption %}
#figure(kind: table, caption: [{{cfg.caption}}])[
{{clp.content}}
] <{{cfg.id}}>
{%- else %}
{{clp.content}} <{{cfg.id}}>
{%- endif %}
