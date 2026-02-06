#!/usr/bin/env bash
set -euo pipefail

pnpm lint &
LINT_PID=$!
pnpm format:check &
FORMAT_PID=$!
pnpm check &
TYPE_PID=$!
xvfb-run --auto-servernum --server-args="-screen 0 1280x1024x24" pnpm test &
TEST_PID=$!

wait $LINT_PID
wait $FORMAT_PID
wait $TYPE_PID
wait $TEST_PID
