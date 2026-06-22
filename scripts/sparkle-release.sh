#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
PROJECT="$ROOT/BelloBox.xcodeproj"
SCHEME="BelloBox"
DMG_BACKGROUND_SCRIPT="$ROOT/scripts/generate-dmg-background.swift"
DMG_DSSTORE_SCRIPT="$ROOT/scripts/write-dmg-dsstore.py"
DMG_BACKGROUND_NAME="bello-box-dmg-background.tiff"

if [[ -z "${SPARKLE_DIR:-}" ]]; then
  DISCOVERED_GENERATE_APPCAST="$(mdfind 'kMDItemFSName == "generate_appcast"' | head -n 1 || true)"
  if [[ -z "$DISCOVERED_GENERATE_APPCAST" ]]; then
    DISCOVERED_GENERATE_APPCAST="$(find "$HOME/Library/Developer/Xcode/DerivedData" -path '*/artifacts/sparkle/Sparkle/bin/generate_appcast' -type f 2>/dev/null | head -n 1 || true)"
  fi
  if [[ -n "$DISCOVERED_GENERATE_APPCAST" ]]; then
    SPARKLE_DIR="$(cd "$(dirname "$DISCOVERED_GENERATE_APPCAST")/.." && pwd)"
  fi
fi

SPARKLE_DIR="${SPARKLE_DIR:-}"
SPARKLE_BIN="$SPARKLE_DIR/bin"

OUT_DIR="${OUT_DIR:-$ROOT/dist/updates}"
BUILD_DIR="${BUILD_DIR:-/tmp/BelloBoxReleaseBuild}"
DMG_NAME_PREFIX="${DMG_NAME_PREFIX:-BelloBox}"
APPCAST_NAME="${APPCAST_NAME:-bello_box.appcast.xml}"
DOWNLOAD_URL_PREFIX="${DOWNLOAD_URL_PREFIX:-https://belloware.com/assets/}"
TEAM_ID="${TEAM_ID:-43TXHV3TM3}"
NOTARY_KEY_ID="${NOTARY_KEY_ID:-SSYSS59Z5W}"
NOTARY_ISSUER_ID="${NOTARY_ISSUER_ID:-ed7b7d3d-c846-4e12-a37d-f216553dc5bb}"
if [[ -z "${NOTARY_KEY_PATH:-}" ]]; then
  for candidate in \
    "/Volumes/My Shared Files/repo/bellobello/AuthKey_SSYSS59Z5W.p8" \
    "$ROOT/../BelloWallProfiles/AuthKey_SSYSS59Z5W.p8"; do
    if [[ -f "$candidate" ]]; then NOTARY_KEY_PATH="$candidate"; break; fi
  done
fi
NOTARY_KEY_PATH="${NOTARY_KEY_PATH:-/Volumes/My Shared Files/repo/bellobello/AuthKey_SSYSS59Z5W.p8}"
NOTARIZE="${NOTARIZE:-1}"
SIGN_IDENTITY="${SIGN_IDENTITY:-Developer ID Application}"
ENTITLEMENTS_PATH="${ENTITLEMENTS_PATH:-$ROOT/BelloBox/BelloBox.Release.entitlements}"
SPARKLE_ED_KEY_ACCOUNT="${SPARKLE_ED_KEY_ACCOUNT:-ed25519}"
SPARKLE_ED_KEY_SERVICE="${SPARKLE_ED_KEY_SERVICE:-https://sparkle-project.org}"
SPARKLE_ED_PRIVATE_KEY="${SPARKLE_ED_PRIVATE_KEY:-}"

if [[ ! -x "$SPARKLE_BIN/generate_appcast" ]]; then
  echo "Sparkle tools not found at $SPARKLE_BIN. Set SPARKLE_DIR to your Sparkle folder."
  exit 1
fi

if [[ ! -f "$DMG_BACKGROUND_SCRIPT" || ! -f "$DMG_DSSTORE_SCRIPT" ]]; then
  echo "DMG helper scripts are missing."
  exit 1
fi

if ! /usr/bin/python3 -c 'import ds_store, mac_alias' >/dev/null 2>&1; then
  echo "Python packages \"ds_store\" and \"mac_alias\" are required to create the styled DMG."
  echo "Install them via: /usr/bin/pip3 install --user dmgbuild"
  exit 1
fi

rm -rf "$BUILD_DIR"
mkdir -p "$OUT_DIR" "$BUILD_DIR"

BUILD_SETTINGS="$(
  xcodebuild -project "$PROJECT" -scheme "$SCHEME" -configuration Release -showBuildSettings
)"
FULL_PRODUCT_NAME="$(awk -F ' = ' '/FULL_PRODUCT_NAME/ {print $2; exit}' <<<"$BUILD_SETTINGS")"
FULL_PRODUCT_NAME="${FULL_PRODUCT_NAME:-$SCHEME.app}"
MARKETING_VERSION="$(awk -F ' = ' '/MARKETING_VERSION/ {print $2; exit}' <<<"$BUILD_SETTINGS")"
CURRENT_PROJECT_VERSION="$(awk -F ' = ' '/CURRENT_PROJECT_VERSION/ {print $2; exit}' <<<"$BUILD_SETTINGS")"
APP_DISPLAY_NAME="${FULL_PRODUCT_NAME%.app}"

echo "Building Release app..."
XCODEBUILD_ARGS=(
  -project "$PROJECT"
  -scheme "$SCHEME"
  -configuration Release
  -derivedDataPath "$BUILD_DIR"
  build
)

if [[ "$NOTARIZE" == "1" ]]; then
  if ! security find-identity -v -p codesigning | grep -q "$SIGN_IDENTITY"; then
    echo "Missing signing identity \"$SIGN_IDENTITY\" for team $TEAM_ID."
    exit 1
  fi
  XCODEBUILD_ARGS+=(
    CODE_SIGN_STYLE=Manual
    CODE_SIGN_IDENTITY="$SIGN_IDENTITY"
    DEVELOPMENT_TEAM="$TEAM_ID"
  )
else
  XCODEBUILD_ARGS+=(
    CODE_SIGNING_ALLOWED=NO
  )
fi

xcodebuild "${XCODEBUILD_ARGS[@]}" >/dev/null

APP_PATH="$BUILD_DIR/Build/Products/Release/$FULL_PRODUCT_NAME"
if [[ ! -d "$APP_PATH" ]]; then
  echo "Release app not found at $APP_PATH"
  exit 1
fi

normalize_bundle_timestamps() {
  local bundle_path="$1"
  local timestamp
  timestamp="$(/usr/bin/python3 - <<'PY'
from datetime import datetime, timedelta
print((datetime.now() - timedelta(minutes=10)).strftime("%Y%m%d%H%M.%S"))
PY
)"
  /usr/bin/find "$bundle_path" -exec /usr/bin/touch -ch -t "$timestamp" {} + 2>/dev/null || true
  /usr/bin/touch -ch -t "$timestamp" "$bundle_path" 2>/dev/null || true
}

codesign_runtime_with_retry() {
  local target_path="$1"
  shift
  local attempt
  for attempt in 1 2 3; do
    if codesign --force --sign "$SIGN_IDENTITY" --timestamp --options runtime "$@" "$target_path"; then
      return 0
    fi
    if [[ "$attempt" -lt 3 ]]; then
      sleep 2
    fi
  done
  echo "Error: failed to sign $target_path with a secure timestamp."
  return 1
}

codesign_with_timestamp_retry() {
  local target_path="$1"
  shift
  local attempt
  for attempt in 1 2 3; do
    if codesign --force --sign "$SIGN_IDENTITY" --timestamp "$@" "$target_path"; then
      return 0
    fi
    if [[ "$attempt" -lt 3 ]]; then
      sleep 2
    fi
  done
  echo "Error: failed to sign $target_path with a secure timestamp."
  return 1
}

if [[ "$NOTARIZE" == "1" ]]; then
  if [[ ! -f "$NOTARY_KEY_PATH" ]]; then
    echo "Notarization key not found at $NOTARY_KEY_PATH"
    exit 1
  fi

  echo "Re-signing app bundle for notarization..."

  find "$APP_PATH" -name "*.xpc" -type d 2>/dev/null | sort -r | while read -r xpc; do
    normalize_bundle_timestamps "$xpc"
    codesign_runtime_with_retry "$xpc"
  done

  find "$APP_PATH" -name "*.app" -type d ! -path "$APP_PATH" 2>/dev/null | sort -r | while read -r nested_app; do
    normalize_bundle_timestamps "$nested_app"
    codesign_runtime_with_retry "$nested_app"
  done

  find "$APP_PATH" -name "*.framework" -type d 2>/dev/null | sort -r | while read -r framework; do
    framework_binary_name="$(basename "$framework" .framework)"
    find "$framework" -type f \( -name "*.dylib" -o -perm -111 \) \
      ! -path "*/Updater.app/*" \
      ! -path "*/XPCServices/*" \
      ! -name "$framework_binary_name" | while read -r binary; do
      codesign_runtime_with_retry "$binary"
    done
    framework_sign_target="$framework"
    if [[ -d "$framework/Versions/Current" ]]; then
      framework_sign_target="$framework/Versions/Current"
    fi
    normalize_bundle_timestamps "$framework_sign_target"
    codesign_with_timestamp_retry "$framework_sign_target"
  done

  normalize_bundle_timestamps "$APP_PATH"
  codesign_runtime_with_retry "$APP_PATH" --entitlements "$ENTITLEMENTS_PATH"

  echo "Verifying code signature..."
  codesign --verify --deep --strict --verbose=2 "$APP_PATH"
else
  echo "Skipping code signature verification (NOTARIZE=0)."
fi

DMG_VOLNAME="${DMG_VOLNAME:-${APP_DISPLAY_NAME} ${MARKETING_VERSION}}"
DMG_PATH="$OUT_DIR/${DMG_NAME_PREFIX}-${MARKETING_VERSION}.dmg"

echo "Version: $MARKETING_VERSION ($CURRENT_PROJECT_VERSION)"
echo "Creating DMG: $DMG_PATH"

APP_ICON_NAME="$(defaults read "$APP_PATH/Contents/Info" CFBundleIconFile 2>/dev/null || echo "AppIcon")"
if [[ "$APP_ICON_NAME" != *.icns ]]; then
  APP_ICON_NAME="${APP_ICON_NAME}.icns"
fi
APP_ICON_PATH="$APP_PATH/Contents/Resources/$APP_ICON_NAME"
if [[ ! -f "$APP_ICON_PATH" ]]; then
  echo "App icon not found at $APP_ICON_PATH"
  exit 1
fi

DMG_STAGING="$(mktemp -d)"
DMG_RENDER="$(mktemp -d)"
RW_DMG_DIR="$(mktemp -d "$OUT_DIR/${DMG_NAME_PREFIX}-${MARKETING_VERSION}.rw.XXXXXX")"
RW_DMG_PATH="$RW_DMG_DIR/${DMG_NAME_PREFIX}-${MARKETING_VERSION}.rw.dmg"
ATTACH_PLIST="$(mktemp)"
DMG_MOUNT_POINT=""
DMG_DEVICE=""

cleanup_dmg_build() {
  if [[ -n "$DMG_DEVICE" ]]; then
    hdiutil detach "$DMG_DEVICE" >/dev/null 2>&1 || hdiutil detach -force "$DMG_DEVICE" >/dev/null 2>&1 || true
  elif [[ -n "$DMG_MOUNT_POINT" ]]; then
    hdiutil detach "$DMG_MOUNT_POINT" >/dev/null 2>&1 || hdiutil detach -force "$DMG_MOUNT_POINT" >/dev/null 2>&1 || true
  fi
  rm -rf "$DMG_STAGING" "$DMG_RENDER" "$ATTACH_PLIST" "$RW_DMG_DIR"
}
trap cleanup_dmg_build EXIT

cp -R "$APP_PATH" "$DMG_STAGING/"
ln -s /Applications "$DMG_STAGING/Applications"
mkdir -p "$DMG_STAGING/.background"
swift "$DMG_BACKGROUND_SCRIPT" "$DMG_RENDER/background.png"
swift "$DMG_BACKGROUND_SCRIPT" "$DMG_RENDER/background@2x.png" 2
/usr/bin/tiffutil -cathidpicheck \
  "$DMG_RENDER/background.png" \
  "$DMG_RENDER/background@2x.png" \
  -out "$DMG_STAGING/.background/$DMG_BACKGROUND_NAME"
cp "$APP_ICON_PATH" "$DMG_STAGING/.VolumeIcon.icns"

hdiutil create -srcfolder "$DMG_STAGING" -volname "$DMG_VOLNAME" -fs HFS+ -format UDRW -ov "$RW_DMG_PATH" >/dev/null
hdiutil attach -readwrite -noverify -noautoopen -nobrowse -plist "$RW_DMG_PATH" >"$ATTACH_PLIST"
DMG_DEVICE="$(
  /usr/bin/python3 - "$ATTACH_PLIST" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as handle:
    data = plistlib.load(handle)

for entity in data.get("system-entities", []):
    mount_point = entity.get("mount-point")
    if mount_point:
        print(entity["dev-entry"])
        break
PY
)"
DMG_MOUNT_POINT="$(
  /usr/bin/python3 - "$ATTACH_PLIST" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as handle:
    data = plistlib.load(handle)

for entity in data.get("system-entities", []):
    mount_point = entity.get("mount-point")
    if mount_point:
        print(mount_point)
        break
PY
)"
if [[ -z "$DMG_DEVICE" || -z "$DMG_MOUNT_POINT" ]]; then
  echo "Failed to attach temporary DMG for layout configuration."
  exit 1
fi

/usr/bin/SetFile -a C "$DMG_MOUNT_POINT" 2>/dev/null || true
/usr/bin/python3 "$DMG_DSSTORE_SCRIPT" \
  --mount-point "$DMG_MOUNT_POINT" \
  --app-name "$(basename "$APP_PATH")" \
  --background-relative-path ".background/$DMG_BACKGROUND_NAME"

sync
hdiutil detach "$DMG_DEVICE" >/dev/null
DMG_DEVICE=""
DMG_MOUNT_POINT=""

rm -f "$DMG_PATH"
hdiutil convert "$RW_DMG_PATH" -format UDZO -imagekey zlib-level=9 -o "${DMG_PATH%.dmg}" >/dev/null
rm -rf "$DMG_STAGING" "$DMG_RENDER" "$ATTACH_PLIST" "$RW_DMG_PATH"
trap - EXIT

if [[ "$NOTARIZE" == "1" ]]; then
  echo "Signing DMG..."
  codesign_with_timestamp_retry "$DMG_PATH"

  echo "Notarizing DMG..."
  xcrun notarytool submit "$DMG_PATH" --key "$NOTARY_KEY_PATH" --key-id "$NOTARY_KEY_ID" --issuer "$NOTARY_ISSUER_ID" --team-id "$TEAM_ID" --wait

  echo "Stapling DMG..."
  xcrun stapler staple -v "$DMG_PATH" >/dev/null

  echo "Validating stapled DMG..."
  xcrun stapler validate -v "$DMG_PATH"
else
  echo "Notarization skipped (NOTARIZE=0)."
fi

APPCAST_STAGING="$(mktemp -d)"
trap 'rm -rf "$APPCAST_STAGING"' EXIT
ditto "$DMG_PATH" "$APPCAST_STAGING/$(basename "$DMG_PATH")"
echo "Generating appcast: $APPCAST_NAME"
if [[ -z "$SPARKLE_ED_PRIVATE_KEY" ]]; then
  SPARKLE_ED_PRIVATE_KEY="$(security find-generic-password -a "$SPARKLE_ED_KEY_ACCOUNT" -s "$SPARKLE_ED_KEY_SERVICE" -w 2>/dev/null || true)"
fi

if [[ -z "$SPARKLE_ED_PRIVATE_KEY" && "$SPARKLE_ED_KEY_SERVICE" != "ed25519" ]]; then
  SPARKLE_ED_PRIVATE_KEY="$(security find-generic-password -a "$SPARKLE_ED_KEY_ACCOUNT" -s "ed25519" -w 2>/dev/null || true)"
fi

if [[ -n "$SPARKLE_ED_PRIVATE_KEY" ]]; then
  printf '%s' "$SPARKLE_ED_PRIVATE_KEY" | "$SPARKLE_BIN/generate_appcast" -o "$OUT_DIR/$APPCAST_NAME" --download-url-prefix "$DOWNLOAD_URL_PREFIX" --ed-key-file - "$APPCAST_STAGING"
else
  "$SPARKLE_BIN/generate_appcast" -o "$OUT_DIR/$APPCAST_NAME" --download-url-prefix "$DOWNLOAD_URL_PREFIX" "$APPCAST_STAGING"
fi
rm -rf "$APPCAST_STAGING"
trap - EXIT

APPCAST_PATH="$OUT_DIR/$APPCAST_NAME"
DMG_MTIME="$(stat -f %m "$DMG_PATH" 2>/dev/null || true)"
if [[ -n "${DMG_MTIME:-}" ]] && command -v /usr/bin/python3 >/dev/null; then
  PUBDATE="$(date -r "$DMG_MTIME" "+%a, %d %b %Y %H:%M:%S %z")"
  /usr/bin/python3 - "$APPCAST_PATH" "$MARKETING_VERSION" "$PUBDATE" "$OUT_DIR" "$APP_DISPLAY_NAME" <<'PY'
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from urllib.parse import urlparse

path = Path(sys.argv[1])
version = sys.argv[2]
pubdate_value = sys.argv[3]
out_dir = Path(sys.argv[4])
channel_title = sys.argv[5]

ns_uri = "http://www.andymatuschak.org/xml-namespaces/sparkle"
ET.register_namespace("sparkle", ns_uri)
ns = {"sparkle": ns_uri}

tree = ET.parse(path)
channel = tree.getroot().find("./channel")
if channel is None:
    raise SystemExit(0)

changed = False
title = channel.find("title")
if title is None:
    title = ET.SubElement(channel, "title")
if (title.text or "").strip() != channel_title:
    title.text = channel_title
    changed = True
for item in channel.findall("item"):
    title = (item.findtext("title") or "").strip()
    short = (item.findtext("sparkle:shortVersionString", namespaces=ns) or "").strip()
    if title != version and short != version:
        continue
    pubdate = item.find("pubDate")
    if pubdate is None:
        pubdate = ET.SubElement(item, "pubDate")
    if (pubdate.text or "").strip() != pubdate_value:
        pubdate.text = pubdate_value
        changed = True
    break

# Prune stale items whose enclosure DMG is no longer present in the output directory.
for item in list(channel.findall("item")):
    enclosure = item.find("enclosure")
    if enclosure is None:
        continue
    enclosure_url = enclosure.attrib.get("url", "")
    basename = Path(urlparse(enclosure_url).path).name
    if not basename:
        continue
    if not (out_dir / basename).exists():
        channel.remove(item)
        changed = True

if changed:
    tree.write(path, encoding="utf-8", xml_declaration=True)
PY
fi

echo "Done."
echo "DMG: $DMG_PATH"
echo "Appcast: $OUT_DIR/$APPCAST_NAME"
