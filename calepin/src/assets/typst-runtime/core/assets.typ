#import "state.typ": _site-root-prefix
#import "config.typ": _runtime-config

#let _resolve-asset-href(path, config: none) = {
  let base = sys.inputs.at("calepin-assets", default: "")
  if base != "" and path.starts-with("/") {
    base + path
  } else if sys.inputs.at("calepin-current-href", default: "") != "" and path.starts-with("/") {
    _site-root-prefix() + path.slice(1)
  } else {
    path
  }
}

#let _resolve-asset-path(path, config: none) = {
  let source-dir = _runtime-config(bound: config).at("source-dir", default: "")
  if path.starts-with("/") or path.starts-with("data:") or path.contains("://") {
    return path
  }
  if source-dir == "" {
    "/" + path
  } else {
    "/" + source-dir + "/" + path
  }
}

#let _image-meta-entry(path, config: none) = {
  let image-meta-path = _runtime-config(bound: config).at("image-meta", default: none)
  if image-meta-path == none or image-meta-path == "" {
    return none
  }
  let images = json(image-meta-path).at("images", default: (:))
  let entry = images.at(path, default: none)
  if entry == none and path.starts-with("/") {
    entry = images.at(path.slice(1), default: none)
  }
  entry
}
