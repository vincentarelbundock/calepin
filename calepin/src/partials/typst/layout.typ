{%- if calepin.is_figure %}#figure([
{% endif %}
{{calepin.rows}}
{%- if calepin.is_figure %}]{%- if config.caption %}, caption: [{{config.caption}}]{% endif %}){% if config.id %} <{{config.id}}>{% endif %}
{%- endif %}
