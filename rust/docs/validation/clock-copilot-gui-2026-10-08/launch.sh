#!/bin/bash
set -uo pipefail
D=/workspace/shared/clock-gui-r2
source /workspace/shared/build-recovery/gui-runtime-env.sh
unset BELLOBOX_AI_ENDPOINT BELLOBOX_AI_MODEL BELLOBOX_AI_PROVIDER BELLOBOX_RECORDING_FIXTURE BELLOBOX_SCREENSHOT_FILE BELLOBOX_SETTINGS_CATEGORY
export BELLOBOX_CONFIG_DIR="$D/config"
export BELLOBOX_AI_KEY=synthetic-clock-gui-key
rm -f "$D/endpoint"
python3 "$D/server.py" > "$D/server.log" 2>&1 &
server=$!
trap 'kill "$server" 2>/dev/null || true' EXIT
for i in {1..50}; do test -f "$D/endpoint" && break; sleep .1; done
cp "$D/config/settings.json" "$D/config-launch.json"
"$D/bellobox-clock" > "$D/app.log" 2>&1
result=$?
printf '%s\n' "$result" > "$D/exit-code"
exit "$result"
