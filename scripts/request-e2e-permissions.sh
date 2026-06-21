#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_PARENT="${BELLOBOX_E2E_TMP_ROOT:-${TMPDIR:-/tmp}}"
RUN_ROOT="$(mktemp -d "$TMP_PARENT/bellobox-permission-e2e.XXXXXX")"
HOME_DIR="$RUN_ROOT/home"
MARKER="$RUN_ROOT/permission.marker"
APP_LOG="$RUN_ROOT/bellobox.log"
KEEP_APP_RUNNING="${BELLOBOX_E2E_KEEP_APP_RUNNING:-1}"
REQUEST_PERMISSIONS="${BELLOBOX_E2E_REQUEST_PERMISSIONS:-1}"
REQUIRE_PERMISSIONS="${BELLOBOX_E2E_REQUIRE_PERMISSIONS:-0}"
WAIT_SECONDS="${BELLOBOX_E2E_PERMISSION_WAIT_SECONDS:-12}"

cd "$ROOT"

if command -v xcodegen >/dev/null 2>&1; then
  xcodegen generate >/dev/null
fi

xcodebuild build \
  -project BelloBox.xcodeproj \
  -scheme BelloBox \
  -configuration Debug \
  -destination 'platform=macOS' >/dev/null

BUILD_SETTINGS="$(xcodebuild -project BelloBox.xcodeproj -scheme BelloBox -configuration Debug -showBuildSettings)"
TARGET_BUILD_DIR="$(awk -F' = ' '/ TARGET_BUILD_DIR = / {print $2; exit}' <<<"$BUILD_SETTINGS")"
FULL_PRODUCT_NAME="$(awk -F' = ' '/ FULL_PRODUCT_NAME = / {print $2; exit}' <<<"$BUILD_SETTINGS")"
APP_PATH="$TARGET_BUILD_DIR/$FULL_PRODUCT_NAME"

mkdir -p "$HOME_DIR/Library/Preferences"
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox hasCompletedSetup -bool true
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox floatingButtonEnabled -bool false
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox globalHotkeyEnabled -bool true
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox screenshotHotkeyEnabled -bool true
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingHotkeyEnabled -bool true

osascript -e 'tell application "Bello Box" to quit' >/dev/null 2>&1 || true
pkill -x "Bello Box" >/dev/null 2>&1 || true

echo "Launching Bello Box permission bootstrap..."
echo "App: $APP_PATH"
echo "Log: $APP_LOG"
echo "Marker: $MARKER"

nohup env \
  HOME="$HOME_DIR" \
  BELLOBOX_E2E_REQUEST_PERMISSIONS="$REQUEST_PERMISSIONS" \
  BELLOBOX_E2E_PERMISSION_MARKER="$MARKER" \
  "$APP_PATH/Contents/MacOS/Bello Box" >"$APP_LOG" 2>&1 &
APP_PID=$!

echo "Bello Box PID: $APP_PID"

stop_app_if_requested() {
  if [[ "$KEEP_APP_RUNNING" != "1" ]]; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi
}

for ((i = 0; i < WAIT_SECONDS * 4; i++)); do
  if [[ -s "$MARKER" ]]; then
    echo
    echo "Permission bootstrap marker:"
    cat "$MARKER"
    echo
    break
  fi
  sleep 0.25
done

if [[ ! -s "$MARKER" ]]; then
  echo
  echo "No permission marker was written within ${WAIT_SECONDS}s." >&2
  echo "--- Bello Box log tail ---" >&2
  tail -n 80 "$APP_LOG" >&2 || true
fi

permission_value() {
  local name="$1"
  awk -F= -v name="$name" '$1 ~ ("\\." name "$") { value = $2 } END { print value }' "$MARKER"
}

if [[ "$REQUIRE_PERMISSIONS" == "1" ]]; then
  if [[ ! -s "$MARKER" ]]; then
    echo "Cannot verify permissions because no marker was written." >&2
    stop_app_if_requested
    exit 1
  fi

  missing=()
  for permission in accessibility screenRecording inputMonitoring microphone; do
    value="$(permission_value "$permission")"
    if [[ "$value" != "granted" ]]; then
      missing+=("$permission=${value:-missing}")
    fi
  done

  if (( ${#missing[@]} > 0 )); then
    echo "Missing required Bello Box permissions: ${missing[*]}" >&2
    stop_app_if_requested
    exit 1
  fi

  echo "Required Bello Box permissions are granted."
fi

if [[ "$REQUEST_PERMISSIONS" == "1" ]]; then
  cat <<EOF

Permission prompts/settings may now be visible for Bello Box.

Grant the app permissions needed for screenshot/recording E2E:
- Accessibility
- Screen Recording
- Input Monitoring, if you want recording click/key overlays
- Microphone, if microphone recording is tested

Note: hotkey E2E scripts synthesize keyboard events through System Events, so the
terminal/test runner may also need Accessibility/Automation permission separately.

Temporary E2E home/logs are in:
$RUN_ROOT
EOF
else
  cat <<EOF

Permission prompts were not requested; this run only checked current status.

Temporary E2E home/logs are in:
$RUN_ROOT
EOF
fi

if [[ "$KEEP_APP_RUNNING" == "1" ]]; then
  if kill -0 "$APP_PID" >/dev/null 2>&1; then
    echo "Leaving Bello Box running so you can grant permissions. Stop it with: kill $APP_PID"
  else
    echo "Bello Box exited after starting the permission requests."
  fi
else
  stop_app_if_requested
  echo "Stopped Bello Box because BELLOBOX_E2E_KEEP_APP_RUNNING=$KEEP_APP_RUNNING."
fi
