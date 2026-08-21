#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bellobox-world-clock-e2e.XXXXXX")"
HOME_DIR="$TMP_ROOT/home"
MARKER="$TMP_ROOT/world-clock.marker"
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
APP_PATH="${BELLOBOX_E2E_APP_PATH:-}"
if [[ -z "$APP_PATH" ]]; then
  command -v xcodegen >/dev/null 2>&1 && xcodegen generate >/dev/null
  xcodebuild build \
    -project BelloBox.xcodeproj \
    -scheme BelloBox \
    -configuration Debug \
    -destination 'platform=macOS' \
    CODE_SIGNING_ALLOWED=NO >/dev/null

  BUILD_SETTINGS="$(xcodebuild -project BelloBox.xcodeproj -scheme BelloBox -configuration Debug -showBuildSettings CODE_SIGNING_ALLOWED=NO)"
  TARGET_BUILD_DIR="$(awk -F' = ' '/ TARGET_BUILD_DIR = / {print $2; exit}' <<<"$BUILD_SETTINGS")"
  FULL_PRODUCT_NAME="$(awk -F' = ' '/ FULL_PRODUCT_NAME = / {print $2; exit}' <<<"$BUILD_SETTINGS")"
  APP_PATH="$TARGET_BUILD_DIR/$FULL_PRODUCT_NAME"
fi

EXECUTABLE="$APP_PATH/Contents/MacOS/Bello Box"
if [[ ! -x "$EXECUTABLE" ]]; then
  echo "Bello Box executable not found in $APP_PATH." >&2
  exit 1
fi

mkdir -p "$HOME_DIR/Library/Preferences"
osascript -e 'tell application "Bello Box" to quit' >/dev/null 2>&1 || true
pkill -x "Bello Box" >/dev/null 2>&1 || true

HOME="$HOME_DIR" \
BELLOBOX_DISABLE_KEYCHAIN=1 \
BELLOBOX_E2E_OPEN_WORLD_CLOCK=1 \
BELLOBOX_E2E_WORLD_CLOCK_SEED=1704067200 \
BELLOBOX_E2E_WORLD_CLOCK_MARKER="$MARKER" \
"$EXECUTABLE" >"$APP_LOG" 2>&1 &
APP_PID=$!

for _ in {1..60}; do
  [[ -s "$MARKER" ]] && break
  sleep 0.2
done

if [[ ! -s "$MARKER" ]]; then
  echo "World Clock E2E failed: the app did not publish its window marker." >&2
  cat "$APP_LOG" >&2 || true
  exit 1
fi

grep -q '^kind=world-clock-window$' "$MARKER"
grep -q '^title=World Clock$' "$MARKER"
grep -q '^visible=true$' "$MARKER"
grep -q '^canJoinAllSpaces=true$' "$MARKER"
grep -q '^fullScreenAuxiliary=true$' "$MARKER"
grep -q '^hidesOnDeactivate=false$' "$MARKER"
grep -q '^selectedInstant=1704067200' "$MARKER"
grep -Eq '^timelineBandCount=(92|96|100)$' "$MARKER"

WINDOW_STATUS="$(osascript <<'APPLESCRIPT'
tell application "System Events"
    repeat 40 times
        if exists process "Bello Box" then
            tell process "Bello Box"
                if exists window "World Clock" then return "visible"
            end tell
        end if
        delay 0.2
    end repeat
end tell
return "missing"
APPLESCRIPT
)"

if [[ "$WINDOW_STATUS" != "visible" ]]; then
  echo "World Clock E2E failed: macOS Accessibility did not expose the real window." >&2
  echo "Grant Accessibility to the terminal/test runner, then retry." >&2
  cat "$MARKER" >&2
  exit 1
fi

visible_app_window_count() {
  swift -e '
    import CoreGraphics
    import Foundation
    let windows = CGWindowListCopyWindowInfo(
      [.optionOnScreenOnly, .excludeDesktopElements],
      kCGNullWindowID
    ) as? [[String: Any]] ?? []
    print(windows.filter { $0[kCGWindowOwnerName as String] as? String == "Bello Box" }.count)
  '
}

WINDOW_COUNT_BEFORE_PICKER="$(visible_app_window_count)"
INTERACTION_STATUS="$(osascript <<'APPLESCRIPT'
tell application "System Events"
    tell process "Bello Box"
        set frontmost to true
        tell window "World Clock"
            set timeSlider to slider 1 of group 1
            set beforeValue to value of timeSlider
            perform action "AXDecrement" of timeSlider
            delay 0.2
            set afterValue to value of timeSlider
            if afterValue is not less than beforeValue then return "slider-stuck"

            set addButton to missing value
            repeat with candidate in buttons of group 1
                try
                    if value of attribute "AXIdentifier" of candidate is "worldClockAddLocationButton" then
                        set addButton to candidate
                        exit repeat
                    end if
                end try
            end repeat
            if addButton is missing value then return "add-location-button-missing"
            click addButton
            delay 0.3
        end tell
    end tell
end tell
return "interactive"
APPLESCRIPT
)"

if [[ "$INTERACTION_STATUS" != "interactive" ]]; then
  echo "World Clock E2E failed: $INTERACTION_STATUS." >&2
  cat "$APP_LOG" >&2 || true
  exit 1
fi

WINDOW_COUNT_AFTER_PICKER="$(visible_app_window_count)"
if (( WINDOW_COUNT_AFTER_PICKER <= WINDOW_COUNT_BEFORE_PICKER )); then
  echo "World Clock E2E failed: the location picker did not open." >&2
  exit 1
fi

echo "World Clock E2E passed: the persistent window opened, its timeline moved, and its location picker opened."
