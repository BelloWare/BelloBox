#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bellobox-hotkey-e2e.XXXXXX")"
HOME_DIR="$TMP_ROOT/home"
MARKER="$TMP_ROOT/toolbar.marker"
SCREENSHOT_MARKER="$TMP_ROOT/screenshot-hotkey.marker"
RECORDING_MARKER="$TMP_ROOT/recording-hotkey.marker"
APP_LOG="$TMP_ROOT/bellobox.log"
APP_PID=""

cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMP_ROOT"
}
trap cleanup EXIT

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
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox globalHotkeyKeyCode -int 11
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox globalHotkeyModifiers -int 1835008
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox screenshotHotkeyEnabled -bool true
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox screenshotHotkeyKeyCode -int 1
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox screenshotHotkeyModifiers -int 1835008
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingHotkeyEnabled -bool true
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingHotkeyKeyCode -int 15
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingHotkeyModifiers -int 1835008

osascript -e 'tell application "Bello Box" to quit' >/dev/null 2>&1 || true
pkill -x "Bello Box" >/dev/null 2>&1 || true

HOME="$HOME_DIR" \
BELLOBOX_E2E_TOOLBAR_MARKER="$MARKER" \
BELLOBOX_E2E_SCREENSHOT_HOTKEY_MARKER="$SCREENSHOT_MARKER" \
BELLOBOX_E2E_RECORDING_HOTKEY_MARKER="$RECORDING_MARKER" \
BELLOBOX_E2E_HOTKEY_MARKERS_ONLY=1 \
BELLOBOX_E2E_SELECTION_TEXT="Bello Box shortcut e2e" \
"$APP_PATH/Contents/MacOS/Bello Box" >"$APP_LOG" 2>&1 &
APP_PID=$!

sleep 1.5

fail() {
  echo "$1" >&2
  echo "This test requires permission for the test runner to synthesize shortcuts." >&2
  echo "If Bello Box permissions are missing, run ./scripts/request-e2e-permissions.sh first." >&2
  echo "--- Bello Box log ---" >&2
  cat "$APP_LOG" >&2 || true
  exit 1
}

press_hotkey() {
  local key_code="$1"
  osascript -e 'tell application "Bello Box" to activate' >/dev/null 2>&1 || true
  sleep 0.2
  osascript <<APPLESCRIPT
tell application "System Events"
    key code $key_code using {control down, option down, command down}
end tell
APPLESCRIPT
}

wait_for_marker() {
  local path="$1"
  local expected="$2"
  local label="$3"
  for _ in {1..40}; do
    if [[ -s "$path" ]]; then
      if grep -q "$expected" "$path"; then
        echo "Hotkey E2E passed: $label."
        return
      fi
      echo "Hotkey E2E failed: $label marker was wrong." >&2
      cat "$path" >&2
      exit 1
    fi
    sleep 0.25
  done
  fail "Hotkey E2E failed: no $label marker was written."
}

press_hotkey 11
wait_for_marker "$MARKER" "Bello Box shortcut e2e" "global shortcut showed the toolbar"

press_hotkey 1
wait_for_marker "$SCREENSHOT_MARKER" "kind=screenshot" "screenshot shortcut callback fired"

press_hotkey 15
wait_for_marker "$RECORDING_MARKER" "kind=recording" "recording shortcut callback fired"
