#import "config.typ": _runtime-config
#import "target.typ": _is-query
#import "results.typ": _results-document

#let _validate-key(key, caller) = {
  if type(key) != str {
    panic(caller + ": store key must be a string")
  }
  if key == "" {
    panic(caller + ": store key must not be empty")
  }
  if key.len() > 256 {
    panic(caller + ": store key exceeds the 256-byte limit")
  }
}

#let _snapshot(config: none) = {
  let runtime-config = _runtime-config(bound: config)
  let store-path = runtime-config.at("store", default: none)
  if _is-query() and store-path != none and store-path != "" {
    json(store-path)
  } else {
    let document = _results-document(config: runtime-config)
    if document == none or document.at("schema", default: 1) == 1 {
      (:)
    } else {
      document.at("store", default: (:))
    }
  }
}

#let _get(config, key, ..args) = {
  _validate-key(key, "calepin.store.get")
  let positional = args.pos()
  let named = args.named()
  if positional.len() > 0 {
    panic("calepin.store.get: unexpected positional argument")
  }
  for name in named.keys() {
    if name != "default" {
      panic("calepin.store.get: unexpected named argument `" + name + "`")
    }
  }
  let snapshot = _snapshot(config: config)
  if key in snapshot {
    snapshot.at(key)
  } else if "default" in named {
    named.at("default")
  } else if _is-query() {
    none
  } else {
    panic("calepin.store.get: store key `" + key + "` has not been set")
  }
}

#let get(key, ..args) = _get(none, key, ..args)

#let set_(key, value) = {
  _validate-key(key, "calepin.store.set")
  if _is-query() {
    [#metadata((key: key, value: value)) <calepin-store-initializer>]
  }
}

#let _bind(config) = (
  get: (key, ..args) => _get(config, key, ..args),
  "set": set_,
)
