#!/bin/bash
set -eu
umask 077
BASE=/workspace/scratch/8b6fda578834/build-environment
QA="$BASE/ocr-qa"
P="$BASE/portal-sysroot/usr/libexec"
source "$BASE/gui-env.sh"
export DISPLAY=:0 XDG_CURRENT_DESKTOP=XFCE GTK_USE_PORTAL=0
export XDG_CONFIG_HOME="$QA/config" XDG_DATA_HOME="$QA/data" XDG_CACHE_HOME="$QA/cache"
export XDG_DATA_DIRS="$BASE/portal-sysroot/usr/share:/usr/local/share:/usr/share"
export XDG_DESKTOP_PORTAL_DIR="$BASE/portal-sysroot/usr/share/xdg-desktop-portal/portals"
export BELLOBOX_CONFIG_DIR="$QA/app-config" BELLOBOX_APPEARANCE=light TMPDIR="$QA/exports"
unset BELLOBOX_AI_KEY BELLOBOX_AI_ENDPOINT BELLOBOX_AI_MODEL BELLOBOX_AI_PROVIDER
unset BELLOBOX_WINDOW_FIXTURE BELLOBOX_AREA_FIXTURE BELLOBOX_SCROLL_FIXTURE BELLOBOX_INLINE_WINDOW_FIXTURE BELLOBOX_INLINE_AREA_FIXTURE BELLOBOX_GIF_FIXTURE BELLOBOX_AI_OCR_FIXTURE
children=()
cleanup() { for pid in "${children[@]}"; do kill "$pid" 2>/dev/null || true; done; wait 2>/dev/null || true; }
trap cleanup EXIT INT TERM
"$P/xdg-permission-store" > "$QA/permission-store.log" 2>&1 & children+=("$!")
"$P/xdg-desktop-portal-gtk" > "$QA/backend.log" 2>&1 & children+=("$!")
"$P/xdg-desktop-portal" > "$QA/portal.log" 2>&1 & children+=("$!")
for attempt in $(seq 1 50); do
 if dbus-send --session --dest=org.freedesktop.DBus --type=method_call --print-reply /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner string:org.freedesktop.portal.Desktop 2>/dev/null | grep -q 'boolean true'; then break; fi
 sleep 0.1
done
dbus-send --session --dest=org.freedesktop.portal.Desktop --type=method_call --print-reply /org/freedesktop/portal/desktop org.freedesktop.DBus.Properties.Get string:org.freedesktop.portal.FileChooser string:version > "$QA/probe.log" 2>&1
case "${1:-popup}" in
 popup) export BELLOBOX_TOOL=screenshot BELLOBOX_AI_OCR_FIXTURE=1 ;;
 area) export BELLOBOX_TOOL=screenshot BELLOBOX_AI_OCR_FIXTURE=area ;;
 window) export BELLOBOX_TOOL=screenshot BELLOBOX_AI_OCR_FIXTURE=window ;;
 gif) export BELLOBOX_TOOL=videoToGIF BELLOBOX_GIF_FIXTURE=1 ;;
 *) exit 2 ;;
esac
cd "$QA/exports"
"$QA/bellobox-immutable-candidate2" > "$QA/candidate2-app-${1:-popup}.log" 2>&1
