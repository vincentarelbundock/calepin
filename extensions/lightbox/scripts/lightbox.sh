#!/bin/sh
# Lightbox: wrap <img> tags inside .figure divs with clickable zoom overlay.
# Protocol: text (stdin = full document, stdout = transformed document).
# Only applies to HTML output; other formats pass through unchanged.

if [ "$CALEPIN_FORMAT" != "html" ]; then
    cat
    exit 0
fi

# Use awk: scan for <img inside .figure divs, wrap in <a class="lightbox">,
# and append CSS/JS if any images were wrapped.
awk '
BEGIN { in_figure = 0; has_lightbox = 0 }

/<div[^>]*class="[^"]*figure[^"]*"/ { in_figure = 1 }

/<\/div>/ {
    if (in_figure) in_figure = 0
    print
    next
}

{
    if (in_figure && match($0, /<img[^>]*src="([^"]*)"[^>]*\/?>/, arr)) {
        src = arr[1]
        sub(/<img/, "<a href=\"" src "\" class=\"lightbox\"><img", $0)
        # Close the <a> after the <img> tag
        if (match($0, /\/>/) || match($0, /<\/img>/)) {
            sub(/\/>/, "/></a>", $0)
            sub(/<\/img>/, "</img></a>", $0)
        } else if (match($0, />/)) {
            # Self-closing without slash
            sub(/>([^>]*)$/, "></a>", $0)
        }
        has_lightbox = 1
    }
    print
}

END {
    if (has_lightbox) {
        print "<style>"
        print ".lightbox { cursor: zoom-in; }"
        print ".lightbox-overlay {"
        print "  position: fixed; top: 0; left: 0; width: 100%; height: 100%;"
        print "  background: rgba(0,0,0,0.85); display: flex; align-items: center; justify-content: center;"
        print "  z-index: 9999; cursor: zoom-out;"
        print "}"
        print ".lightbox-overlay img { max-width: 95%; max-height: 95%; object-fit: contain; }"
        print "</style>"
        print "<script>"
        print "document.querySelectorAll(\".lightbox\").forEach(function(a) {"
        print "  a.addEventListener(\"click\", function(e) {"
        print "    e.preventDefault();"
        print "    var overlay = document.createElement(\"div\");"
        print "    overlay.className = \"lightbox-overlay\";"
        print "    var img = document.createElement(\"img\");"
        print "    img.src = a.href || a.querySelector(\"img\").src;"
        print "    overlay.appendChild(img);"
        print "    overlay.addEventListener(\"click\", function() { overlay.remove(); });"
        print "    document.body.appendChild(overlay);"
        print "  });"
        print "});"
        print "</script>"
    }
}
'
