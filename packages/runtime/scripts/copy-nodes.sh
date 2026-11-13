#!/usr/bin/env sh
# The package ships the nodes the app serves, `pnpm build:nodes` in the repo root builds them.
set -eu

cd "$(dirname "$0")/.."
src=../../app/static/nodes

if ! ls "$src"/*/*/*.wasm >/dev/null 2>&1; then
  echo "copy-nodes: no nodes in $src, run 'pnpm build:nodes' in the repo root first" >&2
  exit 1
fi

rm -rf nodes
mkdir nodes
cp -R "$src"/. nodes/
