#!/usr/bin/env bash
set -euo pipefail

echo "Configuring rclone"

KEY_FILE="$(mktemp)"
echo "${SSH_PRIVATE_KEY}" >"${KEY_FILE}"
chmod 600 "${KEY_FILE}"

mkdir -p ~/.config/rclone
cat >~/.config/rclone/rclone.conf <<EOF
[sftp-remote]
type = sftp
host = ${SSH_HOST}
user = ${SSH_USER}
port = ${SSH_PORT}
key_file = ${KEY_FILE}
EOF

if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  TARGET_DIR="${REMOTE_DIR}"
elif [[ "${GITHUB_EVENT_NAME:-}" == "pull_request" ]]; then
  SAFE_PR_NAME="${GITHUB_HEAD_REF//\//-}"
  TARGET_DIR="${REMOTE_DIR}_${SAFE_PR_NAME}"
elif [[ "${GITHUB_REF_NAME:-}" == "main" ]]; then
  TARGET_DIR="${REMOTE_DIR}_main"
else
  SAFE_REF="${GITHUB_REF_NAME//\//-}"
  TARGET_DIR="${REMOTE_DIR}_${SAFE_REF}"
fi

echo "Deploying to ${TARGET_DIR}"

rclone sync \
  --update \
  --verbose \
  --progress \
  --exclude _astro/** \
  --stats 2s \
  --stats-one-line \
  --transfers 4 \
  ./app/build/ \
  "sftp-remote:${TARGET_DIR}"
