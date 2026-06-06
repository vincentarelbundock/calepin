== Options

Calepin options fall into three groups: **execution**, **result display**, and
**figure output**. Set defaults with `#calepin.setup` and override per call with
`#calepin.chunk(...)` or `#calepin.inline(...)`.

```toml
# Global/default values
[setup]
# Execution

echo   = true      # show source before output
eval   = true      # execute chunk
output = true      # include stdout/stderr output
error  = false     # record execution errors instead of failing
warning = true
message = true
lang   = "python"  # optional: scope defaults to one language

# Result rendering
results = "verbatim"  # "verbatim", "asis", or "hide"
format  = "auto"     # preferred result format(s)
item    = "all"      # all, first, last, or an index
kind    = "auto"     # output-kind hint for structured results

# Figures
fig-device-format     = "svg"     # png, pdf, html where supported
fig-device-dpi        = 150
fig-device-width      = 6
fig-device-height     = "auto"
fig-device-aspect     = 0.618
fig-display-width     = "70%"
fig-display-height    = "auto"
fig-display-align     = "center"
fig-display-responsive = true
fig-caption           = "none"
fig-caption-position  = "auto"
fig-alt-text          = "none"
fig-subcaptions       = "none"
fig-layout-columns    = "auto"
fig-layout-rows       = "auto"
fig-layout-design     = "auto"

# calepin.chunk-only
engine = "python"  # force an engine instead of inferring from fence
body   = "..."      # raw code body when not in a fenced block
label  = "fit-summary"  # stable reference id for results/figures
```
