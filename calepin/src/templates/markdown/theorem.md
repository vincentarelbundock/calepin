{%- set italic = clp.type_class in ["theorem", "lemma", "corollary", "conjecture", "proposition"] -%}
**{{clp.type_class | title}} {{clp.number}}.** {% if italic %}*{{clp.children}}*{% else %}{{clp.children}}{% endif %}
