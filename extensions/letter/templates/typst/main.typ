{% include "preamble.typ" %}
{{clp.preamble}}

{% raw %}
#show: letterloom.with(
  from-name: "{% endraw %}{{cfg.from_name}}{% raw %}",
  from-address: [{% endraw %}{{cfg.from_address | replace("\n", "\\\n")}}{% raw %}],
  to-name: "{% endraw %}{{cfg.to_name}}{% raw %}",
  to-address: [{% endraw %}{{cfg.to_address | replace("\n", "\\\n")}}{% raw %}],
  date: "{% endraw %}{{cfg.date}}{% raw %}",
  salutation: "{% endraw %}{{cfg.salutation}}{% raw %}",
  subject: "{% endraw %}{{cfg.subject}}{% raw %}",
  closing: "{% endraw %}{{cfg.closing}}{% raw %}",
  signatures: (
    (
      name: "{% endraw %}{{cfg.signature_name}}{% raw %}",
      {% endraw %}{% if cfg.signature_image %}{% raw %}signature: image("{% endraw %}{{cfg.signature_image}}{% raw %}", height: 40pt),{% endraw %}{% endif %}{% raw %}
      {% endraw %}{% if cfg.signature_title %}{% raw %}title: "{% endraw %}{{cfg.signature_title}}{% raw %}",{% endraw %}{% endif %}{% raw %}
      {% endraw %}{% if cfg.signature_affiliation %}{% raw %}affiliation: "{% endraw %}{{cfg.signature_affiliation}}{% raw %}",{% endraw %}{% endif %}{% raw %}
    ),
  ),
{% endraw %}{% if cfg.font %}{% raw %}  main-font: "{% endraw %}{{cfg.font}}{% raw %}",
{% endraw %}{% endif %}{% raw %}
)
{% endraw %}

{{clp.body}}
