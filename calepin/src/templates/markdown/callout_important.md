{%- set title = cfg.title -%}
{%- if not title -%}{%- set title = "Important" -%}{%- endif %}
> **❗ {{title}}**
>
> {{clp.children}}
