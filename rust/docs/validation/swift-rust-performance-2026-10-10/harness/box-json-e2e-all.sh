#!/bin/zsh
# Full JSON Tools end-to-end series, one app at a time, interleaved per input.
W=/Users/admin/Library/Caches/BelloRustWork/claude-2026-10-10
OUT=$W/perf/box-e2e/json-r1; mkdir -p $OUT
I=$W/microbench/inputs
for input in json-30k json-100k; do
  $W/harness/box-json-e2e.sh rust $OUT $I/$input.json 10 30000
  $W/harness/box-json-e2e.sh swift $OUT $I/$input.json 10 60000
done
$W/harness/box-json-e2e.sh rust $OUT $I/json-300k.json 10 30000
$W/harness/box-json-e2e.sh swift $OUT $I/json-300k.json 2 240000
echo ALL-DONE
