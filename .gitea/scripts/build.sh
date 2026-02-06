#!/usr/bin/env bash
set -euo pipefail
echo "=== GITHUB_* Variables ==="
printenv | grep '^GITHUB_'
echo
echo "=== GITEA_* Variables ==="
printenv | grep '^GITEA_'

mkdir -p app/static

cp CHANGELOG.md app/static/CHANGELOG.md

cat >app/static/git.json <<EOF
{
  "ref": "${GITEA_REF:-}",
  "ref_name": "${GITEA_REF_NAME:-}",
  "ref_type": "${GITEA_REF_TYPE:-}",
  "sha": "${GITEA_SHA:-}",
  "run_number": "${GITEA_RUN_NUMBER:-}",
  "server_url": "${GITEA_SERVER_URL:-}",
  "event_name": "${GITEA_EVENT_NAME:-}",
  "workflow": "${GITEA_WORKFLOW:-}",
  "job": "${GITEA_JOB:-}",
  "commit_message": "${GITEA_COMMIT_MESSAGE:-}",
  "commit_timestamp": "${GITEA_COMMIT_TIMESTAMP:-}",
  "branch": "${GITEA_HEAD_REF:-}",
  "base_branch": "${GITEA_BASE_REF:-}"
}
EOF

pnpm build

cp -R packages/ui/build app/build/ui
