{%- set title = cfg.title -%}
{%- if not title -%}{%- set title = "Warning" -%}{%- endif %}
> **⚠️ {{title}}**
>
> {{clp.children}}
