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

commit_message=$(git log -1 --pretty=%B | tr -d '\n' | sed 's/"/\\"/g')

cat >app/static/git.json <<EOF
{
  "ref": "${GITHUB_REF:-}",
  "ref_name": "${REF_NAME}",
  "ref_type": "${REF_TYPE}",
  "sha": "${GITHUB_SHA:-}",
  "run_number": "${GITHUB_RUN_NUMBER:-}",
  "event_name": "${GITHUB_EVENT_NAME:-}",
  "workflow": "${GITHUB_WORKFLOW:-}",
  "job": "${GITHUB_JOB:-}",
  "commit_message": "${commit_message}",
  "commit_timestamp": "$(git log -1 --pretty=%cI)",
  "branch": "${BRANCH}",
  "commits_since_last_release": "${COMMITS_SINCE_LAST_RELEASE}"
}
EOF

pnpm build

cp -R packages/ui/build app/build/ui
