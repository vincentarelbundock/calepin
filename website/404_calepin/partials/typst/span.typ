{%- if class == "aside" %}
#place(right, dx: 1em)[#text(size: 0.8em)[{{content}}]]
{%- else %}
[{{content}}]
{%- endif %}
