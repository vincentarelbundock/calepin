{%- set loc = cap_location | default("bottom") -%}
{%- if loc == "bottom" %}
{{children}}

: {{caption}}
{%- else %}
: {{caption}}

{{children}}
{%- endif %}
