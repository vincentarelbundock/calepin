{%- set loc = config.cap_location | default("bottom") -%}
{%- if loc == "bottom" %}
{{calepin.children}}

: {{config.caption}}
{%- else %}
: {{config.caption}}

{{calepin.children}}
{%- endif %}
