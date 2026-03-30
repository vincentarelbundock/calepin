{%- if config.filename %}
#block(stroke: 0.5pt + luma(180), radius: 3pt, clip: true)[
#block(width: 100%, fill: luma(240), inset: (x: 8pt, y: 4pt))[#text(size: 0.85em)[{{config.filename}}]]
{%- endif %}
#srcbox[#raw("{{calepin.code}}", block: true, lang: "{{config.lang}}")]
{%- if config.filename %}
]
{%- endif %}
