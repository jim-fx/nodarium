#!/usr/bin/env bash
set -euo pipefail

# Posts (or updates) a comment on the pull request linking to the
# deployed preview of its branch. Mirrors the target naming of deploy-files.sh.

PR_NUMBER="${PR_NUMBER:?PR_NUMBER is required}"
API="${GITHUB_SERVER_URL}/api/v1/repos/${GITHUB_REPOSITORY}"
AUTH="Authorization: token ${GITEA_TOKEN}"

SAFE_BRANCH="${GITHUB_HEAD_REF//\//-}"
PREVIEW_URL="https://${SAFE_BRANCH}.nodes.max-richter.dev"
MARKER="<!-- nodarium-preview -->"

BODY="${MARKER}
🚀 Preview deployed: ${PREVIEW_URL}

Built from ${GITHUB_SHA:0:7}"

PAYLOAD="$(jq -n --arg body "$BODY" '{body: $body}')"

EXISTING_ID="$(
  curl -fsS -H "$AUTH" "${API}/issues/${PR_NUMBER}/comments" |
    jq -r --arg m "$MARKER" '[.[] | select(.body | startswith($m))][0].id // empty'
)"

if [[ -n "$EXISTING_ID" ]]; then
  echo "Updating preview comment ${EXISTING_ID}"
  curl -fsS -X PATCH -H "$AUTH" -H "Content-Type: application/json" \
    -d "$PAYLOAD" "${API}/issues/comments/${EXISTING_ID}" >/dev/null
else
  echo "Creating preview comment"
  curl -fsS -X POST -H "$AUTH" -H "Content-Type: application/json" \
    -d "$PAYLOAD" "${API}/issues/${PR_NUMBER}/comments" >/dev/null
fi

echo "Preview: ${PREVIEW_URL}"
