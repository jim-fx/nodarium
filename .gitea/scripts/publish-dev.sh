#!/bin/sh
# Publishes @nodarium/runtime under the dist-tag dev as <next patch>-dev.<UTC time>.<short sha>,
# like 0.0.11-dev.202611131200.ac7774d. The version only goes into the published package, the
# package.json is restored afterwards. DRY_RUN=1 packs the package without publishing it.
set -eu

cd "$(dirname "$0")/../.."
pkg=packages/runtime/package.json

next=$(jq -r .version "$pkg" | awk -F. '{ printf "%d.%d.%d", $1, $2, $3 + 1 }')
stamp=$(date -u +%Y%m%d%H%M)
sha=$(git rev-parse --short=7 HEAD)
# a prerelease part made of digits only must not start with 0, git describe adds a g for the same reason
case "$sha" in
  *[!0-9]*) ;;
  *) sha="g$sha" ;;
esac
version="$next-dev.$stamp.$sha"

trap 'git checkout -- "$pkg"' EXIT
jq --arg v "$version" '.version = $v' "$pkg" >"$pkg.tmp"
mv "$pkg.tmp" "$pkg"

echo "📦 Publishing @nodarium/runtime@$version under the dist-tag dev"
if [ "${DRY_RUN:-0}" = 1 ]; then
  pnpm --filter @nodarium/runtime publish --tag dev --no-git-checks --dry-run
else
  pnpm --filter @nodarium/runtime publish --tag dev --no-git-checks
fi
