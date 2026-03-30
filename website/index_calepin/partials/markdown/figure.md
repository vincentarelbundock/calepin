{%- set loc = cap_location | default("bottom") %}
{%- if link %}[{%- endif %}
![{{alt}}]({{src}})
{%- if link %}]({{link}}){%- endif %}
{%- if caption %}

*{{caption}}*
{%- endif %}
