{%- set loc = config.cap_location | default("bottom") %}
{%- if config.link %}[{%- endif %}
![{{config.alt}}]({{calepin.src}})
{%- if config.link %}]({{config.link}}){%- endif %}
{%- if config.caption %}

*{{config.caption}}*
{%- endif %}
