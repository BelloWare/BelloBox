#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "Checking Bello Box E2E permissions..."
BELLOBOX_E2E_REQUEST_PERMISSIONS="${BELLOBOX_E2E_REQUEST_PERMISSIONS:-0}" \
BELLOBOX_E2E_REQUIRE_PERMISSIONS=1 \
BELLOBOX_E2E_KEEP_APP_RUNNING=0 \
./scripts/request-e2e-permissions.sh

echo
echo "Running Bello Box hotkey E2E..."
./scripts/run-hotkey-e2e.sh

echo
echo "Running Bello Box capture/recording E2E..."
./scripts/run-capture-recording-e2e.sh

echo
echo "Bello Box E2E passed."
