#!/usr/bin/env sh
# Crates are named <collection>_<node> because Cargo needs unique package names
# across the workspace; the collection prefix is stripped again here.
set -eu

src=target/wasm32-unknown-unknown/release
dst=app/static/nodes/max
collections="plantarium myzel"

for collection in $collections; do
  rm -rf "$dst/$collection"
done

for wasm in "$src"/*.wasm; do
  [ -e "$wasm" ] || continue
  name=$(basename "$wasm" .wasm)
  copied=false
  for collection in $collections; do
    case "$name" in
      "${collection}"_*)
        mkdir -p "$dst/$collection"
        cp "$wasm" "$dst/$collection/${name#"${collection}"_}.wasm"
        copied=true
        ;;
    esac
  done
  if [ "$copied" = false ]; then
    echo "copy-node-wasm: $name.wasm has no known collection prefix ($collections)" >&2
    exit 1
  fi
done
