{% include "preamble.typ" %}
{{clp.preamble}}

{% raw %}
#show: cv.with(
  author: "{% endraw %}{{cfg.name}}{% raw %}",
  address: "{% endraw %}{{cfg.address}}{% raw %}",
  contacts: (
{% endraw %}{% for c in cfg.contacts %}{% raw %}    [{% endraw %}{{c}}{% raw %}],
{% endraw %}{% endfor %}{% raw %}
  ),
)
{% endraw %}

{{clp.body}}
