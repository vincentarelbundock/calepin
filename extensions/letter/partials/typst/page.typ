{% include "preamble.typ" %}
{{calepin.preamble}}

{% raw %}
#show: letterloom.with(
  from-name: "{% endraw %}{{config.from_name}}{% raw %}",
  from-address: [{% endraw %}{{config.from_address | replace("\n", "\\\n")}}{% raw %}],
  to-name: "{% endraw %}{{config.to_name}}{% raw %}",
  to-address: [{% endraw %}{{config.to_address | replace("\n", "\\\n")}}{% raw %}],
  date: "{% endraw %}{{config.date}}{% raw %}",
  salutation: "{% endraw %}{{config.salutation}}{% raw %}",
  subject: "{% endraw %}{{config.subject}}{% raw %}",
  closing: "{% endraw %}{{config.closing}}{% raw %}",
  signatures: (
    (
      name: "{% endraw %}{{config.signature_name}}{% raw %}",
      {% endraw %}{% if config.signature_image %}{% raw %}signature: image("{% endraw %}{{config.signature_image}}{% raw %}", height: 40pt),{% endraw %}{% endif %}{% raw %}
      {% endraw %}{% if config.signature_title %}{% raw %}title: "{% endraw %}{{config.signature_title}}{% raw %}",{% endraw %}{% endif %}{% raw %}
      {% endraw %}{% if config.signature_affiliation %}{% raw %}affiliation: "{% endraw %}{{config.signature_affiliation}}{% raw %}",{% endraw %}{% endif %}{% raw %}
    ),
  ),
{% endraw %}{% if config.font %}{% raw %}  main-font: "{% endraw %}{{config.font}}{% raw %}",
{% endraw %}{% endif %}{% raw %}
)
{% endraw %}

{{calepin.body}}
