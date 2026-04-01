#!/bin/sh
# imgur: upload local images to imgur and return format-appropriate markup.
# Protocol: JSON (stdin = SpanModuleInput, stdout = ModuleOutput).
#
# Usage: [path/to/image.png]{.imgur alt="description"}
#
# Requires curl. Set IMGUR_CLIENT_ID env var or defaults to a public key.

# Check for curl
if ! command -v curl >/dev/null 2>&1; then
    python3 -c "
import json
print(json.dumps({
    'action': 'rendered',
    'output': '<!-- imgur error: curl is not installed. Install curl to use the imgur extension. -->'
}))
"
    echo "Error: imgur extension requires curl but it is not installed." >&2
    echo "Install curl (e.g., brew install curl, apt install curl) and try again." >&2
    exit 0
fi

# Read JSON input from stdin
INPUT=$(cat)

# Parse with python3
eval "$(echo "$INPUT" | python3 -c "
import sys, json, shlex
data = json.load(sys.stdin)
print(f'WRITER={shlex.quote(data[\"writer\"])}')
print(f'CONTENT={shlex.quote(data[\"content\"])}')
attrs = data.get('attrs', {})
print(f'ALT={shlex.quote(attrs.get(\"alt\", \"\"))}')
print(f'CLIENT_ID_ATTR={shlex.quote(attrs.get(\"client_id\", \"\"))}')
")"

IMAGE_PATH="$CONTENT"

# Resolve relative paths from project root
if [ -n "$CALEPIN_ROOT" ] && [ ! -f "$IMAGE_PATH" ]; then
    IMAGE_PATH="${CALEPIN_ROOT}/${CONTENT}"
fi

if [ ! -f "$IMAGE_PATH" ]; then
    python3 -c "
import json, sys
print(json.dumps({'action': 'rendered', 'output': '<!-- imgur: file not found: ' + sys.argv[1] + ' -->'}))
" "$CONTENT"
    exit 0
fi

# Determine client ID: attribute > env var > default
CLIENT_ID="${CLIENT_ID_ATTR:-${IMGUR_CLIENT_ID:-019b80d1dc7a38b}}"

# Base64-encode the image and upload
B64=$(base64 < "$IMAGE_PATH" | tr -d '\n')

RESPONSE=$(curl -s -X POST "https://api.imgur.com/3/image" \
    -H "Authorization: Client-ID ${CLIENT_ID}" \
    -F "image=${B64}" \
    -F "type=base64" 2>&1)

# Extract the link from the response
URL=$(echo "$RESPONSE" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    link = data.get('data', {}).get('link', '')
    if link:
        print(link, end='')
    else:
        err = data.get('data', {}).get('error', 'unknown error')
        print(f'ERROR:{err}', end='')
except:
    print('ERROR:failed to parse imgur response', end='')
")

case "$URL" in
    ERROR:*)
        ERR_MSG=$(echo "$URL" | sed 's/^ERROR://')
        python3 -c "
import json, sys
print(json.dumps({'action': 'rendered', 'output': '<!-- imgur upload failed: ' + sys.argv[1] + ' -->'}))
" "$ERR_MSG"
        exit 0
        ;;
esac

# Generate format-appropriate output
case "$WRITER" in
    html)
        OUTPUT="<img src=\"${URL}\" alt=\"${ALT}\" />"
        ;;
    latex)
        OUTPUT="\\includegraphics{${CONTENT}} % imgur: ${URL}"
        ;;
    typst)
        OUTPUT="#image(\"${URL}\")"
        ;;
    *)
        OUTPUT="![${ALT}](${URL})"
        ;;
esac

python3 -c "
import json, sys
print(json.dumps({'action': 'rendered', 'output': sys.argv[1]}))
" "$OUTPUT"
