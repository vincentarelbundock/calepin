{%- set loc = cfg.cap_location | default("bottom") -%}
{%- if loc == "bottom" %}
{{clp.children}}

: {{cfg.caption}}
{%- else %}
: {{cfg.caption}}

{{clp.children}}
{%- endif %}
