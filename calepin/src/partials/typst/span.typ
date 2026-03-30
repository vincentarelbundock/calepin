{%- if config.class == "aside" %}
#place(right, dx: 1em)[#text(size: 0.8em)[{{calepin.content}}]]
{%- else %}
[{{calepin.content}}]
{%- endif %}
