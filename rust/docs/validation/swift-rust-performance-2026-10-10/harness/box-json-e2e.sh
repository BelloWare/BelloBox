#!/bin/zsh
# End-to-end JSON Tools (Pretty) timing in one Box app. Each sample: Clear, then
# click Paste with INPUT on the clipboard; records the first change of the result
# area and when the footer reaches its finished state ("Copy Result" enabled),
# matched against a reference captured after a warm-up paste.
# Usage: box-json-e2e.sh rust|swift OUT INPUT [COUNT] [TIMEOUT_MS]
# One app at a time (macOS 14 cooperative activation keeps another active app
# frontmost). Swift Box runs with Keychain IPC denied; the caller backs up and
# restores its preferences. The clipboard is cleared afterwards.
set -u
W=/Users/admin/Library/Caches/BelloRustWork/claude-2026-10-10
P=$W/harness/belloperf
APP=$1; OUT=$2; INPUT=$3; COUNT=${4:-10}; TIMEOUT=${5:-30000}
NAME=$(basename $INPUT .json)
mkdir -p $OUT
if [ $APP = rust ]; then
  BIN=$W/bin/bellobox-d3fd43a
  rm -rf $OUT/rust-config; mkdir -p $OUT/rust-config
  (BELLOBOX_CONFIG_DIR=$OUT/rust-config $BIN > $OUT/rust-$NAME-stdout.log 2> $OUT/rust-$NAME-stderr.log &)
  TILE=(345 418); PASTE=(750 183); CLEAR=(790 183); RESULT=(20 365 780 250)
  MATCH="$BIN"
else
  # SWIFT_BOX_APP selects another build of the Swift app (default: the shipped one).
  BIN="${SWIFT_BOX_APP:-$W/swift-apps/box/Bello Box.app}/Contents/MacOS/Bello Box"
  (/usr/bin/sandbox-exec -f $W/harness/no-keychain.sb "$BIN" > $OUT/swift-$NAME-stdout.log 2> $OUT/swift-$NAME-stderr.log &)
  TILE=(345 400); PASTE=(757 159); CLEAR=(791 159); RESULT=(20 340 780 270)
  MATCH="$BIN"
fi
FOOTER=(0 632 820 54)
sleep 4
PID=$(pgrep -n -f "$MATCH")
read HOME_ID HX HY <<< "$($P windows $PID | sed -n 's/^window id=\([0-9]*\).*bounds=(\([0-9.]*\), \([0-9.]*\),.*/\1 \2 \3/p' | head -1)"
HX=${HX%.*}; HY=${HY%.*}
$P input --pid $PID --kind key --count 0 --click $((HX + TILE[1])),$((HY + TILE[2])) --region 0,0,1,1 > /dev/null 2>&1
sleep 2
read ID X Y <<< "$($P order $PID | sed -n 's/^pid=[0-9]* window=\([0-9]*\) layer=0 bounds=\([0-9.]*\),\([0-9.]*\),820,688/\1 \2 \3/p' | head -1)"
X=${X%.*}; Y=${Y%.*}
echo "app=$APP pid=$PID tool=$ID origin=$X,$Y input=$NAME"
pbcopy < $INPUT
# Warm-up: paste once, wait until the app has been idle for 3 s, keep that footer.
$P input --pid $PID --window $ID --kind key --count 0 --click $((X + PASTE[1])),$((Y + PASTE[2])) --region 0,0,1,1 > /dev/null 2>&1
START=$(date +%s); IDLE=0
while [ $IDLE -lt 3 ] && [ $(( $(date +%s) - START )) -lt $((TIMEOUT / 1000 + 60)) ]; do
  sleep 1
  CPU=$(ps -p $PID -o %cpu= | tr -d ' '); CPU=${CPU%.*}
  if [ "${CPU:-0}" -lt 5 ]; then IDLE=$((IDLE + 1)); else IDLE=0; fi
done
echo "warm-up done after $(( $(date +%s) - START )) s"
$P capture-region $ID $((X + FOOTER[1])),$((Y + FOOTER[2])),${FOOTER[3]},${FOOTER[4]} $OUT/$APP-$NAME-footer-done.png > /dev/null
$P capture $ID $OUT/$APP-$NAME-warmup.png > /dev/null
$P input --pid $PID --window $ID --kind click --at $((X + PASTE[1])),$((Y + PASTE[2])) --reset $((X + CLEAR[1])),$((Y + CLEAR[2])) \
  --reset-ms 1500 --interval-ms 300 --count $COUNT --timeout-ms $TIMEOUT \
  --region $((X + RESULT[1])),$((Y + RESULT[2])),${RESULT[3]},${RESULT[4]} \
  --done-ref $OUT/$APP-$NAME-footer-done.png --done-region $((X + FOOTER[1])),$((Y + FOOTER[2])),${FOOTER[3]},${FOOTER[4]} \
  --out $OUT/$APP-$NAME.json --label "$APP $NAME"
$P capture $ID $OUT/$APP-$NAME-last.png > /dev/null
pbcopy < /dev/null
kill $PID
sleep 1
pgrep -f "$MATCH" > /dev/null && echo "WARNING: $APP still running" || echo "stopped $APP"
