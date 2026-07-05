#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_PARENT="${BELLOBOX_E2E_TMP_ROOT:-${TMPDIR:-/tmp}}"
RUN_ROOT="$(mktemp -d "$TMP_PARENT/bellobox-capture-recording-e2e.XXXXXX")"
HOME_DIR="$RUN_ROOT/home"
APP_LOG="$RUN_ROOT/bellobox.log"
FIXTURE_LOG="$RUN_ROOT/recording-fixture.log"
APP_PID=""
FIXTURE_PID=""

cd "$ROOT"

cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${FIXTURE_PID:-}" ]] && kill -0 "$FIXTURE_PID" >/dev/null 2>&1; then
    kill "$FIXTURE_PID" >/dev/null 2>&1 || true
    wait "$FIXTURE_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if command -v xcodegen >/dev/null 2>&1; then
  xcodegen generate >/dev/null
fi

echo "Checking Bello Box capture/recording permissions..."
BELLOBOX_E2E_REQUEST_PERMISSIONS=0 \
BELLOBOX_E2E_REQUIRE_PERMISSIONS=1 \
BELLOBOX_E2E_KEEP_APP_RUNNING=0 \
./scripts/request-e2e-permissions.sh >/dev/null

BUILD_SETTINGS="$(xcodebuild -project BelloBox.xcodeproj -scheme BelloBox -configuration Debug -showBuildSettings)"
TARGET_BUILD_DIR="$(awk -F' = ' '/ TARGET_BUILD_DIR = / {print $2; exit}' <<<"$BUILD_SETTINGS")"
FULL_PRODUCT_NAME="$(awk -F' = ' '/ FULL_PRODUCT_NAME = / {print $2; exit}' <<<"$BUILD_SETTINGS")"
APP_PATH="$TARGET_BUILD_DIR/$FULL_PRODUCT_NAME"

mkdir -p "$HOME_DIR/Library/Preferences"
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox hasCompletedSetup -bool true
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox floatingButtonEnabled -bool false
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox screenshotAutoCopy -bool false
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingAudioSource -string none
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingIncludeCursor -bool false
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingClickOverlayMode -string off
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingKeystrokeMode -string off
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingCountdownSeconds -int 0
HOME="$HOME_DIR" defaults write com.ainoob.BelloBox recordingQualityPreset -string compact

stop_existing_app() {
  osascript -e 'tell application "Bello Box" to quit' >/dev/null 2>&1 || true
  pkill -x "Bello Box" >/dev/null 2>&1 || true
}

wait_for_marker() {
  local marker="$1"
  local label="$2"
  local timeout="${3:-40}"
  for ((i = 0; i < timeout * 4; i++)); do
    if [[ -s "$marker" ]]; then
      echo
      echo "$label marker:"
      cat "$marker"
      echo
      if ! grep -q '^status=success$' "$marker"; then
        echo "$label failed." >&2
        echo "--- Bello Box log tail ---" >&2
        tail -n 120 "$APP_LOG" >&2 || true
        exit 1
      fi
      return
    fi
    sleep 0.25
  done

  echo "$label failed: marker was not written within ${timeout}s." >&2
  echo "--- Bello Box log tail ---" >&2
  tail -n 120 "$APP_LOG" >&2 || true
  exit 1
}

assert_file_min_size() {
  local path="$1"
  local min_size="$2"
  local label="$3"
  if [[ ! -f "$path" ]]; then
    echo "$label failed: expected file does not exist at $path." >&2
    exit 1
  fi
  local size
  size="$(stat -f%z "$path")"
  if (( size < min_size )); then
    echo "$label failed: file is too small ($size bytes, expected at least $min_size)." >&2
    exit 1
  fi
}

marker_value() {
  local marker="$1"
  local key="$2"
  awk -F= -v key="$key" '$1 == key {print substr($0, index($0, "=") + 1); exit}' "$marker"
}

assert_real_screenshot_marker() {
  local marker="$1"
  local label="$2"
  local count
  count="$(marker_value "$marker" "displayCount")"
  if [[ ! "$count" =~ ^[0-9]+$ ]] || (( count < 1 )); then
    echo "$label failed: marker did not include a valid displayCount." >&2
    exit 1
  fi

  for ((i = 0; i < count; i++)); do
    if [[ "$(marker_value "$marker" "display[$i].status")" != "success" ]]; then
      echo "$label failed: display[$i] did not report success." >&2
      exit 1
    fi
    if [[ "$(marker_value "$marker" "display[$i].dimensionMatches")" != "true" ]]; then
      echo "$label failed: display[$i] dimensions did not match expected crop size." >&2
      exit 1
    fi
    local path
    path="$(marker_value "$marker" "display[$i].path")"
    assert_file_min_size "$path" 1024 "$label display[$i]"
  done
}

assert_marker_value() {
  local marker="$1"
  local key="$2"
  local expected="$3"
  local label="$4"
  local actual
  actual="$(marker_value "$marker" "$key")"
  if [[ "$actual" != "$expected" ]]; then
    echo "$label failed: expected $key=$expected, got ${actual:-<missing>}." >&2
    exit 1
  fi
}

wait_for_file() {
  local path="$1"
  local label="$2"
  local timeout="${3:-15}"
  for ((i = 0; i < timeout * 4; i++)); do
    if [[ -s "$path" ]]; then
      return
    fi
    sleep 0.25
  done

  echo "$label failed: $path was not written within ${timeout}s." >&2
  return 1
}

start_recording_fixture() {
  local marker="$RUN_ROOT/recording-fixture.ready"
  local binary="$RUN_ROOT/e2e-recording-fixture"

  rm -f "$marker"
  xcrun swiftc -framework AppKit "$ROOT/scripts/e2e-recording-fixture.swift" -o "$binary"
  "$binary" "$marker" >"$FIXTURE_LOG" 2>&1 &
  FIXTURE_PID=$!
  if ! wait_for_file "$marker" "Recording fixture" 15; then
    echo "--- Recording fixture log tail ---" >&2
    tail -n 120 "$FIXTURE_LOG" >&2 || true
    exit 1
  fi
  echo
  echo "Recording fixture marker:"
  cat "$marker"
  echo
}

launch_app_for_marker() {
  local label="$1"
  local marker="$2"
  local timeout="$3"
  shift 3

  stop_existing_app
  rm -f "$marker"
  : >"$APP_LOG"
  nohup env \
    HOME="$HOME_DIR" \
    BELLOBOX_E2E_QUIT_AFTER_E2E=1 \
    "$@" \
    "$APP_PATH/Contents/MacOS/Bello Box" >"$APP_LOG" 2>&1 &
  APP_PID=$!

  wait_for_marker "$marker" "$label" "$timeout"

  for _ in {1..20}; do
    if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
      APP_PID=""
      return
    fi
    sleep 0.1
  done
  kill "$APP_PID" >/dev/null 2>&1 || true
  wait "$APP_PID" >/dev/null 2>&1 || true
  APP_PID=""
}

REAL_SCREENSHOT="$RUN_ROOT/real-screenshot.png"
REAL_SCREENSHOT_MARKER="$RUN_ROOT/real-screenshot.marker"
OVERLAY_SCREENSHOT="$RUN_ROOT/overlay-screenshot.png"
OVERLAY_SCREENSHOT_MARKER="$RUN_ROOT/overlay-screenshot.marker"
MULTI_OVERLAY_SCREENSHOT="$RUN_ROOT/overlay-multi-screenshot.png"
MULTI_OVERLAY_SCREENSHOT_MARKER="$RUN_ROOT/overlay-multi-screenshot.marker"
RECORDING_MOV="$RUN_ROOT/recording.mov"
RECORDING_MARKER="$RUN_ROOT/recording.marker"

launch_app_for_marker \
  "Real screenshot E2E" \
  "$REAL_SCREENSHOT_MARKER" \
  30 \
  BELLOBOX_E2E_REAL_SCREENSHOT_OUTPUT="$REAL_SCREENSHOT" \
  BELLOBOX_E2E_REAL_SCREENSHOT_MARKER="$REAL_SCREENSHOT_MARKER"
assert_file_min_size "$REAL_SCREENSHOT" 1024 "Real screenshot E2E"
assert_real_screenshot_marker "$REAL_SCREENSHOT_MARKER" "Real screenshot E2E"

launch_app_for_marker \
  "Capture overlay screenshot E2E" \
  "$OVERLAY_SCREENSHOT_MARKER" \
  30 \
  BELLOBOX_E2E_CAPTURE_OVERLAY_IMAGE="$REAL_SCREENSHOT" \
  BELLOBOX_E2E_CAPTURE_OVERLAY_AUTO_SELECT=1 \
  BELLOBOX_E2E_CAPTURE_OVERLAY_OUTPUT="$OVERLAY_SCREENSHOT" \
  BELLOBOX_E2E_CAPTURE_OVERLAY_MARKER="$OVERLAY_SCREENSHOT_MARKER"
assert_file_min_size "$OVERLAY_SCREENSHOT" 1024 "Capture overlay screenshot E2E"

launch_app_for_marker \
  "Capture overlay multi-display E2E" \
  "$MULTI_OVERLAY_SCREENSHOT_MARKER" \
  30 \
  BELLOBOX_E2E_CAPTURE_OVERLAY_SIMULATED_DISPLAYS=1 \
  BELLOBOX_E2E_CAPTURE_OVERLAY_OUTPUT="$MULTI_OVERLAY_SCREENSHOT" \
  BELLOBOX_E2E_CAPTURE_OVERLAY_MARKER="$MULTI_OVERLAY_SCREENSHOT_MARKER"
assert_file_min_size "$MULTI_OVERLAY_SCREENSHOT" 1024 "Capture overlay multi-display E2E"
assert_marker_value "$MULTI_OVERLAY_SCREENSHOT_MARKER" "selectionDisplayID" "9002" "Capture overlay multi-display E2E"
assert_marker_value "$MULTI_OVERLAY_SCREENSHOT_MARKER" "baseImageColorTag" "secondary-left" "Capture overlay multi-display E2E"
assert_marker_value "$MULTI_OVERLAY_SCREENSHOT_MARKER" "imageWidth" "240" "Capture overlay multi-display E2E"
assert_marker_value "$MULTI_OVERLAY_SCREENSHOT_MARKER" "imageHeight" "140" "Capture overlay multi-display E2E"

start_recording_fixture
launch_app_for_marker \
  "Real recording E2E" \
  "$RECORDING_MARKER" \
  45 \
  BELLOBOX_E2E_REAL_RECORDING_OUTPUT="$RECORDING_MOV" \
  BELLOBOX_E2E_REAL_RECORDING_MARKER="$RECORDING_MARKER" \
  BELLOBOX_E2E_RECORDING_OWN_PULSE=0 \
  BELLOBOX_E2E_RECORDING_DURATION="${BELLOBOX_E2E_RECORDING_DURATION:-1.2}"
assert_file_min_size "$RECORDING_MOV" 2048 "Real recording E2E"

echo "Capture/recording E2E passed."
echo "Artifacts are in: $RUN_ROOT"
