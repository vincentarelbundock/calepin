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
> **{{icon}} {{title}}**
>
> {{children}}
