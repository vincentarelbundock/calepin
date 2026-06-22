#import "/.calepin/calepin.typ" as calepin
#import "@preview/classicthesis:0.1.0": *

#show: classicthesis.with(
  {% if vars.title %}title: "{{ vars.title }}",{% endif %}
  {% if vars.subtitle %}subtitle: "{{ vars.subtitle }}",{% endif %}
  {% if vars.author %}author: "{{ vars.author }}",{% endif %}
  {% if vars.dedication %}dedication: "{{ vars.dedication }}",{% endif %}
  {% if vars.abstract %}abstract: "{{ vars.abstract }}",{% endif %}
  {% if vars.date %}date: "{{ vars.date }}",{% endif %}
)

{{ doc.body }}
