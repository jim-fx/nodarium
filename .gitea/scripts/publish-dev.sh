#!/bin/sh
# Publishes the packages under the dist-tag dev as <next patch>-dev.<UTC time>.<short sha>,
# like 0.0.11-dev.202611131200.ac7774d. All of them get the same version, so the
# `workspace:` ranges between them point at the packages of the same commit. The version only
# goes into the published packages, the package.json files are restored afterwards.
# DRY_RUN=1 packs the packages without publishing them.
set -eu

cd "$(dirname "$0")/../.."
pkgs="packages/runtime/package.json packages/ui/package.json packages/graph-interface/package.json"

next=$(jq -r .version packages/runtime/package.json | awk -F. '{ printf "%d.%d.%d", $1, $2, $3 + 1 }')
stamp=$(date -u +%Y%m%d%H%M)
sha=$(git rev-parse --short=7 HEAD)
# a prerelease part made of digits only must not start with 0, git describe adds a g for the same reason
case "$sha" in
  *[!0-9]*) ;;
  *) sha="g$sha" ;;
esac
version="$next-dev.$stamp.$sha"

# shellcheck disable=SC2086
trap 'git checkout -- $pkgs' EXIT
for pkg in $pkgs; do
  jq --arg v "$version" '.version = $v' "$pkg" >"$pkg.tmp"
  mv "$pkg.tmp" "$pkg"
done

echo "📦 Publishing @nodarium/runtime, ui and graph-interface@$version under the dist-tag dev"
filters="--filter @nodarium/runtime --filter @nodarium/ui --filter @nodarium/graph-interface"
if [ "${DRY_RUN:-0}" = 1 ]; then
  # shellcheck disable=SC2086
  pnpm $filters publish --tag dev --no-git-checks --dry-run
else
  # shellcheck disable=SC2086
  pnpm $filters publish --tag dev --no-git-checks
fi
