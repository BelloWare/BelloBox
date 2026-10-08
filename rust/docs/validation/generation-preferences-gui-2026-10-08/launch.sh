#!/bin/bash
set -uo pipefail
D=/workspace/shared/generation-gui-r1
source /workspace/shared/build-recovery/gui-runtime-env.sh
unset BELLOBOX_AI_ENDPOINT BELLOBOX_AI_MODEL BELLOBOX_AI_PROVIDER BELLOBOX_RECORDING_FIXTURE BELLOBOX_SCREENSHOT_FILE
export BELLOBOX_CONFIG_DIR="$D/config" BELLOBOX_AI_KEY=synthetic-gui-key BELLOBOX_SETTINGS_CATEGORY=ai
python3 "$D/server.py" > "$D/server.log" 2>&1 &
server=$!
trap 'kill "$server" 2>/dev/null || true' EXIT
for i in {1..50}; do test -f "$D/endpoint" && break; sleep .1; done
/workspace/shared/box-generation-evidence/bellobox-generation-r1 > "$D/app.log" 2>&1
echo $? > "$D/exit-code"
