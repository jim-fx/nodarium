#!/usr/bin/env bash
set -euo pipefail

mkdir -p app/static

cp CHANGELOG.md app/static/CHANGELOG.md

# Derive branch/tag info
REF_TYPE="${GITHUB_REF_TYPE:-branch}"
REF_NAME="${GITHUB_REF_NAME:-$(basename "$GITHUB_REF")}"
BRANCH="${GITHUB_HEAD_REF:-}"
if [[ -z "$BRANCH" && "$REF_TYPE" == "branch" ]]; then
  BRANCH="$REF_NAME"
fi

# Determine last tag and commits since
LAST_TAG="$(git describe --tags --abbrev=0 2>/dev/null || true)"
if [[ -n "$LAST_TAG" ]]; then
  COMMITS_SINCE_LAST_RELEASE="$(git rev-list --count "${LAST_TAG}..HEAD")"
else
  COMMITS_SINCE_LAST_RELEASE="0"
fi

# jq escapes the commit message, a backslash in it made git.json invalid
jq -n \
  --arg ref "${GITHUB_REF:-}" \
  --arg ref_name "$REF_NAME" \
  --arg ref_type "$REF_TYPE" \
  --arg sha "${GITHUB_SHA:-}" \
  --arg run_number "${GITHUB_RUN_NUMBER:-}" \
  --arg event_name "${GITHUB_EVENT_NAME:-}" \
  --arg workflow "${GITHUB_WORKFLOW:-}" \
  --arg job "${GITHUB_JOB:-}" \
  --arg commit_message "$(git log -1 --pretty=%B | tr -d '\n')" \
  --arg commit_timestamp "$(git log -1 --pretty=%cI)" \
  --arg branch "$BRANCH" \
  --arg commits_since_last_release "$COMMITS_SINCE_LAST_RELEASE" \
  '$ARGS.named' >app/static/git.json

# the analytics tag, Umami rejects one longer than 50 characters
if [[ "$REF_TYPE" == "tag" ]]; then
  CHANNEL="release"
else
  CHANNEL="${BRANCH:-unknown}"
fi
export PUBLIC_CHANNEL="${CHANNEL:0:50}"

pnpm -C app exec tsx scripts/fetch-roadmap.ts static/roadmap.json

pnpm build

cp -R packages/ui/build app/build/ui
