#!/bin/bash
# Assemble an isolated, ad-hoc-signed development preview only.
# This deliberately does not download frameworks, install the app, or publish it.
set -euo pipefail
[[ "$(uname -s)" == Darwin ]] || { echo 'Run this packaging script on macOS.' >&2; exit 1; }
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT_DIR:-$ROOT/dist}"
APP="$OUT/BelloBox Rust.app"
[[ ! -e "$APP" ]] || { echo "Refusing to replace existing app: $APP" >&2; exit 1; }
[[ -z "${SIGN_IDENTITY:-}" ]] || { echo 'Preview packaging does not accept a signing identity.' >&2; exit 1; }
cd "$ROOT"
# Dependencies must already be cached by the normal build; never fetch here.
PROFILE="${PACKAGE_PROFILE:-release}"
case "$PROFILE" in
  release) cargo build --offline --locked --release -p bellobox-app ;;
  debug) cargo build --offline --locked -p bellobox-app ;;
  *) echo 'PACKAGE_PROFILE must be debug or release.' >&2; exit 1 ;;
esac
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp "$TARGET/$PROFILE/bellobox" "$APP/Contents/MacOS/bellobox"
python3 "$ROOT/scripts/validate-bellobox-macos.py" "$APP" --write-info
if [[ -n "${SPARKLE_FRAMEWORK:-}" ]]; then
  [[ -d "$SPARKLE_FRAMEWORK" && -f "$SPARKLE_FRAMEWORK/Resources/Info.plist" ]] || { echo 'SPARKLE_FRAMEWORK must point to the official Sparkle.framework.' >&2; exit 1; }
  VERSION=$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$SPARKLE_FRAMEWORK/Resources/Info.plist")
  [[ "$VERSION" == 2.8.1 ]] || { echo "Expected Sparkle 2.8.1, found $VERSION" >&2; exit 1; }
  ditto "$SPARKLE_FRAMEWORK" "$APP/Contents/Frameworks/Sparkle.framework"
fi
python3 "$ROOT/scripts/validate-bellobox-macos.py" "$APP"
# Ad-hoc signing needs no certificate, Keychain or timestamp service.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
printf 'Created %s\nNot notarized or installed. Native UI and updates remain unverified.\n' "$APP"
