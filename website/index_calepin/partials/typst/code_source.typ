{%- if filename %}
#block(stroke: 0.5pt + luma(180), radius: 3pt, clip: true)[
#block(width: 100%, fill: luma(240), inset: (x: 8pt, y: 4pt))[#text(size: 0.85em)[{{filename}}]]
{%- endif %}
#srcbox[#raw("{{code}}", block: true, lang: "{{lang}}")]
{%- if filename %}
]
{%- endif %}
