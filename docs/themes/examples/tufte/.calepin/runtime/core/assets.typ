#let _image-meta-path = sys.inputs.at("calepin-image-meta", default: "")
#let _source-dir = sys.inputs.at("calepin-source-dir", default: "")

#let _resolve-asset-href(path) = {
  let base = sys.inputs.at("calepin-assets", default: "")
  if base != "" and path.starts-with("/") {
    base + path
  } else {
    path
  }
}

#let _resolve-asset-path(path) = {
  if path.starts-with("/") or path.starts-with("data:") or path.contains("://") {
    return path
  }
  if _source-dir == "" {
    "/" + path
  } else {
    "/" + _source-dir + "/" + path
  }
}

#let _image-meta-entry(path) = {
  if _image-meta-path == "" {
    return none
  }
  let images = json(_image-meta-path).at("images", default: (:))
  let entry = images.at(path, default: none)
  if entry == none and path.starts-with("/") {
    entry = images.at(path.slice(1), default: none)
  }
  entry
}
