#let _mode = sys.inputs.at("calepin-mode", default: "render")
#let _target = sys.inputs.at("calepin-target", default: "paged")

#let _is-query() = _mode == "query"
#let _is-render() = _mode == "render"
#let _is-html() = _target == "html"
#let _is-paged() = not _is-html()
