#!/bin/sh
# txtfmt: text formatting span module.
# Protocol: JSON (stdin = SpanModuleInput, stdout = ModuleOutput).
#
# Usage: [styled text]{.txtfmt color=red em=1.5 smallcaps=true underline=true mark=true}
#
# Attributes:
#   color     - Color name or #hex value
#   em        - Font size in em units
#   smallcaps - Enable small caps (true/false)
#   underline - Enable underline (true/false)
#   mark      - Enable highlight/mark (true/false)

# Read JSON input from stdin
INPUT=$(cat)

WRITER=$(echo "$INPUT" | sed -n 's/.*"writer":"\([^"]*\)".*/\1/p')
CONTENT=$(echo "$INPUT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
print(data['content'], end='')
")
# Parse attributes
ATTRS_JSON=$(echo "$INPUT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for k, v in data.get('attrs', {}).items():
    print(f'{k}={v}')
")

COLOR=""
EM=""
SMALLCAPS=""
UNDERLINE=""
MARK=""

while IFS='=' read -r key value; do
    case "$key" in
        color) COLOR="$value" ;;
        em) EM="$value" ;;
        smallcaps) [ "$value" = "true" ] && SMALLCAPS=1 ;;
        underline) [ "$value" = "true" ] && UNDERLINE=1 ;;
        mark) [ "$value" = "true" ] && MARK=1 ;;
    esac
done <<EOF
$ATTRS_JSON
EOF

# Resolve color name to hex using the bundled color dictionary
resolve_color() {
    local name="$1"
    case "$name" in
        \#*) echo "$name"; return ;;
    esac
    # Normalize: lowercase, strip non-alphanumeric
    local normalized
    normalized=$(echo "$name" | tr '[:upper:]' '[:lower:]' | tr -d ' -')
    local script_dir
    script_dir=$(cd "$(dirname "$0")" && pwd)
    local dict="$script_dir/../color_dict.csv"
    if [ -f "$dict" ]; then
        awk -F',' -v name="$normalized" '
            NR > 1 {
                gsub(/"/, "", $1)
                if ($1 == name) { gsub(/"/, "", $2); print $2; exit }
            }
        ' "$dict"
    else
        echo "$name"
    fi
}

if [ -n "$COLOR" ]; then
    COLOR=$(resolve_color "$COLOR")
fi

# Render based on writer format
case "$WRITER" in
    html)
        CLASSES=""
        STYLE=""
        # Tailwind utilities where possible
        [ -n "$SMALLCAPS" ] && CLASSES="${CLASSES} small-caps"
        [ -n "$UNDERLINE" ] && CLASSES="${CLASSES} underline"
        # Font size: use Tailwind text-[Xem] arbitrary value
        [ -n "$EM" ] && CLASSES="${CLASSES} text-[${EM}em]"
        # Color needs inline style (arbitrary hex from color dict)
        [ -n "$COLOR" ] && STYLE="color: ${COLOR};"

        RESULT="$CONTENT"
        [ -n "$MARK" ] && RESULT="<mark>${RESULT}</mark>"
        if [ -n "$CLASSES" ] || [ -n "$STYLE" ]; then
            ATTRS=""
            [ -n "$CLASSES" ] && ATTRS=" class=\"${CLASSES# }\""
            [ -n "$STYLE" ] && ATTRS="${ATTRS} style=\"${STYLE}\""
            RESULT="<span${ATTRS}>${RESULT}</span>"
        fi
        ;;
    latex)
        RESULT="$CONTENT"
        [ -n "$MARK" ] && RESULT="\\hl{${RESULT}}"
        [ -n "$UNDERLINE" ] && RESULT="\\underline{${RESULT}}"
        [ -n "$SMALLCAPS" ] && RESULT="\\textsc{${RESULT}}"
        if [ -n "$COLOR" ]; then
            HEX=$(echo "$COLOR" | sed 's/^#//' | tr '[:lower:]' '[:upper:]')
            RESULT="\\textcolor[HTML]{${HEX}}{${RESULT}}"
        fi
        if [ -n "$EM" ]; then
            SIZE_PT=$(echo "$EM" | awk '{printf "%.1f", $1 * 10}')
            SKIP_PT=$(echo "$EM" | awk '{printf "%.1f", $1 * 12}')
            RESULT="{\\fontsize{${SIZE_PT}pt}{${SKIP_PT}pt}\\selectfont ${RESULT}}"
        fi
        ;;
    typst)
        RESULT="$CONTENT"
        [ -n "$MARK" ] && RESULT="#highlight[${RESULT}]"
        [ -n "$UNDERLINE" ] && RESULT="#underline[${RESULT}]"
        [ -n "$SMALLCAPS" ] && RESULT="#smallcaps[${RESULT}]"
        ARGS=""
        [ -n "$EM" ] && ARGS="${ARGS}size: ${EM}em, "
        [ -n "$COLOR" ] && ARGS="${ARGS}fill: rgb(\"${COLOR}\"), "
        if [ -n "$ARGS" ]; then
            ARGS=$(echo "$ARGS" | sed 's/, $//')
            RESULT="#text(${ARGS})[${RESULT}]"
        fi
        ;;
    *)
        RESULT="$CONTENT"
        ;;
esac

# Output JSON response
python3 -c "
import json, sys
print(json.dumps({'action': 'rendered', 'output': sys.argv[1]}))
" "$RESULT"
