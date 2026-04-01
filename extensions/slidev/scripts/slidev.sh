#!/bin/sh
# Slidev: convert rendered markdown body to Slidev slide format.
# Protocol: text (stdin = full document, stdout = transformed document).

# Read the full document from stdin
BODY=$(cat)

# Extract title from CALEPIN_TITLE env var if set, else use "Untitled"
TITLE="${CALEPIN_TITLE:-Untitled}"

# Build the output using awk
echo "$BODY" | awk -v title="$TITLE" '
BEGIN {
    # Print the title slide with Slidev YAML frontmatter
    print "---"
    print "theme: default"
    print "title: " title
    print "---"
    print ""
    print "# " title
    slide_break = 0
}

/^## / {
    # Each ## heading starts a new slide
    if (slide_break) print ""
    print ""
    print "---"
    print ""
    print $0
    slide_break = 1
    next
}

{ print }
'
