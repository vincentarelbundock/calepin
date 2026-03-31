root := justfile_directory()

export TYPST_ROOT := root

[private]
default:
  @just --list --unsorted

# generate manual
doc:
  typst compile docs/manual.typ docs/manual.pdf
  # typst compile docs/thumbnail.typ thumbnail-light.svg
  # typst compile --input theme=dark docs/thumbnail.typ thumbnail-dark.svg

# run test suite
test *args:
  tt run {{ args }}

# update test cases
update *args:
  tt update {{ args }}

# package the library into the specified destination folder
package target:
  ./scripts/package "{{target}}"

# install the library with the "@local" prefix
install: (package "@local")

# install the library with the "@preview" prefix (for pre-release testing)
install-preview: (package "@preview")

[private]
remove target:
  ./scripts/uninstall "{{target}}"

# uninstalls the library from the "@local" prefix
uninstall: (remove "@local")

# uninstalls the library from the "@preview" prefix (for pre-release testing)
uninstall-preview: (remove "@preview")

thumbnail:
  typst compile -f png --pages 1 --ppi 250 template/main.typ thumbnail.png
  oxipng -o 4 --strip safe --alpha thumbnail.png

# run ci suite
ci: test
