#!/bin/bash
# Build an unsigned development .app, or sign it when SIGN_IDENTITY is supplied.
# This deliberately does not download frameworks, install the app, or publish it.
set -euo pipefail
[[ "$(uname -s)" == Darwin ]] || { echo 'Run this packaging script on macOS.' >&2; exit 1; }
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT_DIR:-$ROOT/dist}"
APP="$OUT/BelloBox Rust.app"
[[ ! -e "$APP" ]] || { echo "Refusing to replace existing app: $APP" >&2; exit 1; }
cd "$ROOT"
cargo build --locked --release -p bellobox-app
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp "$TARGET/release/bellobox" "$APP/Contents/MacOS/bellobox"
export APP
python3 - <<'PY'
import os, plistlib
app=os.environ['APP']
info={
 'CFBundleIdentifier':'com.ainoob.BelloBox', 'CFBundleName':'BelloBox Rust',
 'CFBundleDisplayName':'BelloBox Rust', 'CFBundleExecutable':'bellobox',
 'CFBundlePackageType':'APPL', 'CFBundleShortVersionString':os.environ.get('APP_VERSION','0.1.0'),
 'CFBundleVersion':os.environ.get('APP_BUILD','1'), 'LSMinimumSystemVersion':'13.0',
 'NSPrincipalClass':'NSApplication', 'LSUIElement':False,
 'NSHighResolutionCapable':True, 'SUEnableAutomaticChecks':False,
 'SUAutomaticallyUpdate':False,
 'NSHumanReadableCopyright':'Copyright © 2026 Belloware. All rights reserved.',
}
# The Swift production feed must not overwrite this unfinished Rust application.
# A release engineer must explicitly supply a Rust feed before updater testing.
if feed:=os.environ.get('RUST_SPARKLE_FEED_URL'):
 if not feed.startswith('https://'): raise SystemExit('RUST_SPARKLE_FEED_URL must use HTTPS')
 info['SUFeedURL']=feed
 info['SUPublicEDKey']=os.environ.get('SPARKLE_PUBLIC_ED_KEY','slSJ7z2j8RDa266+E/7To5AOOloc2YtiMUZUVEIhwNA=')
with open(app+'/Contents/Info.plist','wb') as f: plistlib.dump(info,f)
PY
if [[ -n "${SPARKLE_FRAMEWORK:-}" ]]; then
  [[ -d "$SPARKLE_FRAMEWORK" && -f "$SPARKLE_FRAMEWORK/Resources/Info.plist" ]] || { echo 'SPARKLE_FRAMEWORK must point to the official Sparkle.framework.' >&2; exit 1; }
  VERSION=$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$SPARKLE_FRAMEWORK/Resources/Info.plist")
  [[ "$VERSION" == 2.8.1 ]] || { echo "Expected Sparkle 2.8.1, found $VERSION" >&2; exit 1; }
  ditto "$SPARKLE_FRAMEWORK" "$APP/Contents/Frameworks/Sparkle.framework"
fi
if [[ -n "${SIGN_IDENTITY:-}" ]]; then
  # Sparkle's XPC services/helpers need nested signing according to its current
  # distribution guide; official pre-signed framework retains those signatures.
  codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$APP"
  codesign --verify --deep --strict --verbose=2 "$APP"
else
  codesign --force --deep --sign - "$APP"
fi
printf 'Created %s\nNot notarized or installed. Verify on macOS before distribution.\n' "$APP"
