#!/bin/bash
set -eu
D=/workspace/shared/palette-qr-gui
P="$D/portal-sysroot/usr/libexec"
source /workspace/shared/build-recovery/gui-runtime-env.sh
export XDG_CURRENT_DESKTOP=XFCE GTK_USE_PORTAL=0
export XDG_CONFIG_HOME="$D/config" XDG_DATA_HOME="$D/data" XDG_CACHE_HOME="$D/cache"
export XDG_DATA_DIRS="$D/portal-sysroot/usr/share:/usr/local/share:/usr/share"
export XDG_DESKTOP_PORTAL_DIR="$D/portal-sysroot/usr/share/xdg-desktop-portal/portals"
export BELLOBOX_CONFIG_DIR="$D/app-config" BELLOBOX_APPEARANCE=light TMPDIR="$D/exports"
unset BELLOBOX_AI_KEY BELLOBOX_AI_ENDPOINT BELLOBOX_AI_MODEL BELLOBOX_AI_PROVIDER
children=()
cleanup(){ for pid in "${children[@]}"; do kill "$pid" 2>/dev/null || true; done; wait 2>/dev/null || true; }
trap cleanup EXIT INT TERM
"$P/xdg-permission-store" > "$D/evidence/a78203800a1c/permission-store.log" 2>&1 & children+=("$!")
"$P/xdg-desktop-portal-gtk" > "$D/evidence/a78203800a1c/backend.log" 2>&1 & children+=("$!")
"$P/xdg-desktop-portal" --verbose > "$D/evidence/a78203800a1c/portal.log" 2>&1 & children+=("$!")
for attempt in $(seq 1 50); do
 if dbus-send --session --dest=org.freedesktop.DBus --type=method_call --print-reply /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner string:org.freedesktop.portal.Desktop 2>/dev/null | grep -q 'boolean true'; then break; fi
 sleep .1
done
dbus-send --session --dest=org.freedesktop.portal.Desktop --type=method_call --print-reply /org/freedesktop/portal/desktop org.freedesktop.DBus.Properties.Get string:org.freedesktop.portal.FileChooser string:version > "$D/evidence/a78203800a1c/probe.txt" 2>&1
export BELLOBOX_TOOL=qr BELLOBOX_INPUT=synthetic-original
cd "$D/exports"
/workspace/shared/box-palette-qr-sealed/bellobox-a78203800a1c > "$D/evidence/a78203800a1c/app-portal.log" 2>&1
