{%- set callout_type = cfg.classes | replace("callout-", "") -%}
{%- set title = cfg.title -%}
{%- set icon = cfg.icon -%}
{%- set appearance = cfg.appearance -%}
{%- if not cfg.title -%}
  {%- if callout_type == "note" -%}{%- set title = "Note" -%}
  {%- elif callout_type == "tip" -%}{%- set title = "Tip" -%}
  {%- elif callout_type == "warning" -%}{%- set title = "Warning" -%}
  {%- elif callout_type == "caution" -%}{%- set title = "Caution" -%}
  {%- elif callout_type == "important" -%}{%- set title = "Important" -%}
  {%- else -%}{%- set title = "Note" -%}
  {%- endif -%}
{%- endif -%}
{%- if not cfg.icon -%}
  {%- if callout_type == "tip" -%}{%- set icon = "💡" -%}
  {%- elif callout_type == "warning" -%}{%- set icon = "⚠️" -%}
  {%- elif callout_type == "caution" -%}{%- set icon = "🔥" -%}
  {%- elif callout_type == "important" -%}{%- set icon = "❗" -%}
  {%- else -%}{%- set icon = "ℹ️" -%}
  {%- endif -%}
{%- endif -%}
{%- if not cfg.appearance -%}{%- set appearance = "default" -%}{%- endif -%}
> **{{icon}} {{title}}**
>
> {{clp.children}}
