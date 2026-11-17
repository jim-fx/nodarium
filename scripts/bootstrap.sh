#!/usr/bin/env sh
# Sets up a fresh checkout or worktree: dependencies, the ui and planty builds the app's
# tsconfig needs, and the node wasm in app/static/nodes.
#
# --reuse-nodes copies app/static/nodes from the main checkout instead of building them,
# but only when no Rust (or anything compiled into a node) differs from the commit the
# main checkout is on. Otherwise it builds as usual.
set -eu

reuse=false
for arg in "$@"; do
  case "$arg" in
    --reuse-nodes) reuse=true ;;
    *)
      echo "bootstrap: unknown argument $arg (only --reuse-nodes)" >&2
      exit 1
      ;;
  esac
done

rust_paths="nodes packages/macros packages/myzel packages/types packages/utils Cargo.toml Cargo.lock .cargo"

reuse_nodes() {
  main=$(git worktree list --porcelain | sed -n '1s/^worktree //p')
  here=$(git rev-parse --show-toplevel)
  if [ "$main" = "$here" ]; then
    echo "bootstrap: this is the main checkout, building the nodes"
    return 1
  fi
  if [ ! -d "$main/app/static/nodes/max" ]; then
    echo "bootstrap: $main has no built nodes, building them"
    return 1
  fi
  main_head=$(git -C "$main" rev-parse HEAD)
  # shellcheck disable=SC2086
  if ! git diff --quiet "$main_head" -- $rust_paths \
    || [ -n "$(git ls-files --others --exclude-standard -- $rust_paths)" ] \
    || ! git -C "$main" diff --quiet HEAD -- $rust_paths; then
    echo "bootstrap: Rust differs from the main checkout ($main_head), building the nodes"
    return 1
  fi
  rm -rf app/static/nodes
  mkdir -p app/static
  cp -R "$main/app/static/nodes" app/static/nodes
  echo "bootstrap: copied app/static/nodes from $main"
}

pnpm install
pnpm _postinstall

if [ "$reuse" = false ] || ! reuse_nodes; then
  pnpm build:nodes
fi
