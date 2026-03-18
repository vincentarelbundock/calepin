{%- set callout_type = classes | replace("callout-", "") -%}
{%- if not title -%}
  {%- if callout_type == "note" -%}{%- set title = "Note" -%}
  {%- elif callout_type == "tip" -%}{%- set title = "Tip" -%}
  {%- elif callout_type == "warning" -%}{%- set title = "Warning" -%}
  {%- elif callout_type == "caution" -%}{%- set title = "Caution" -%}
  {%- elif callout_type == "important" -%}{%- set title = "Important" -%}
  {%- else -%}{%- set title = "Note" -%}
  {%- endif -%}
{%- endif -%}
{%- if not icon -%}
  {%- if callout_type == "tip" -%}{%- set icon = "💡" -%}
  {%- elif callout_type == "warning" -%}{%- set icon = "⚠️" -%}
  {%- elif callout_type == "caution" -%}{%- set icon = "🔥" -%}
  {%- elif callout_type == "important" -%}{%- set icon = "❗" -%}
  {%- else -%}{%- set icon = "ℹ️" -%}
  {%- endif -%}
{%- endif -%}
{%- if not appearance -%}{%- set appearance = "default" -%}{%- endif -%}
{%- if callout_type == "note" %}
#block(fill: rgb("#dbeafe"), stroke: (left: 3pt + rgb("#3b82f6")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
{%- elif callout_type == "tip" %}
#block(fill: rgb("#dcfce7"), stroke: (left: 3pt + rgb("#22c55e")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
{%- elif callout_type == "warning" %}
#block(fill: rgb("#fef9c3"), stroke: (left: 3pt + rgb("#eab308")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
{%- elif callout_type == "caution" %}
#block(fill: rgb("#fee2e2"), stroke: (left: 3pt + rgb("#ef4444")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
{%- elif callout_type == "important" %}
#block(fill: rgb("#ede9fe"), stroke: (left: 3pt + rgb("#8b5cf6")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
{%- else %}
#block(fill: rgb("#dbeafe"), stroke: (left: 3pt + rgb("#3b82f6")), inset: (x: 10pt, y: 8pt), radius: 0pt, width: 100%)[
{%- endif %}
  #text(weight: "bold")[{{icon}} {{title}}] \
  {{children}}
]{% if id %} <{{id}}>{% endif %}
