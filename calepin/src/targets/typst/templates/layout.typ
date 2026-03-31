{%- if clp.is_figure %}#figure([
{% endif %}
{{clp.rows}}
{%- if clp.is_figure %}]{%- if cfg.caption %}, caption: [{{cfg.caption}}]{% endif %}){% if cfg.id %} <{{cfg.id}}>{% endif %}
{%- endif %}
