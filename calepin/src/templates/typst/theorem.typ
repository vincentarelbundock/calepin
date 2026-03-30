{%- set italic = clp.type_class in ["theorem", "lemma", "corollary", "conjecture", "proposition"] -%}
#block(width: 100%, above: 1em, below: 1em)[*{{clp.type_class | title}} {{clp.number}}.* {% if italic %}#emph[{{clp.children}}]{% else %}{{clp.children}}{% endif %}]{% if cfg.id %} <{{cfg.id}}>{% endif %}
