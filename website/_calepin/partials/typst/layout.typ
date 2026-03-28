{%- if is_figure %}#figure([
{% endif %}
{{rows}}
{%- if is_figure %}]{%- if caption %}, caption: [{{caption}}]{% endif %}){% if id %} <{{id}}>{% endif %}
{%- endif %}
