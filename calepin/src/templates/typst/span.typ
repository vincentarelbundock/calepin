{%- if cfg.class == "aside" %}
#place(right, dx: 1em)[#text(size: 0.8em)[{{clp.content}}]]
{%- else %}
[{{clp.content}}]
{%- endif %}
