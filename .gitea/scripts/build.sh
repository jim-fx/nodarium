#!/usr/bin/env bash
set -euo pipefail

mkdir -p app/static

cp CHANGELOG.md app/static/CHANGELOG.md

# Derive branch/tag info
REF_TYPE="${GITHUB_REF_TYPE:-branch}"
REF_NAME="${GITHUB_REF_NAME:-$(basename "$GITHUB_REF")}"
BRANCH="${GITHUB_HEAD_REF:-}"
BASE_BRANCH="${GITHUB_BASE_REF:-}"
if [[ -z "$BRANCH" && "$REF_TYPE" == "branch" ]]; then
  BRANCH="$REF_NAME"
fi

cat >app/static/git.json <<EOF
{
  "ref": "${GITHUB_REF:-}",
  "ref_name": "${REF_NAME}",
  "ref_type": "${REF_TYPE}",
  "sha": "${GITHUB_SHA:-}",
  "run_number": "${GITHUB_RUN_NUMBER:-}",
  "server_url": "${GITHUB_SERVER_URL:-}",
  "event_name": "${GITHUB_EVENT_NAME:-}",
  "workflow": "${GITHUB_WORKFLOW:-}",
  "job": "${GITHUB_JOB:-}",
  "commit_message": "$(git log -1 --pretty=%B)",
  "commit_timestamp": "$(git log -1 --pretty=%cI)",
  "branch": "${BRANCH}",
  "base_branch": "${BASE_BRANCH}"
}
EOF

pnpm build

cp -R packages/ui/build app/build/ui
