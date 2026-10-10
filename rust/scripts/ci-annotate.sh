#!/usr/bin/env bash
# Run one CI command. On failure, repeat the failing test names and their first
# panic/assertion lines as a GitHub annotation: job logs need an authenticated
# client, while check-run annotations are publicly readable.
#   bash scripts/ci-annotate.sh "Title" cargo test ...
set -uo pipefail
title=$1
shift
log=$(mktemp "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/ci-annotate.XXXXXX")
"$@" 2>&1 | tee "$log"
code=${PIPESTATUS[0]}
if [ "$code" -ne 0 ]; then
  summary=$(awk '/^failures:$/ { found = 1 } found' "$log" |
    grep -E '^---- |panicked at|assertion|left:|right:|^    [A-Za-z0-9_:]+$' |
    head -40 | cut -c1-240 | tr '\r\n' '  ')
  if [ -z "$summary" ]; then
    summary=$(grep -E 'error(\[[A-Z0-9]+\])?:|FAILED|panicked at' "$log" |
      head -20 | cut -c1-240 | tr '\r\n' '  ')
  fi
  summary=${summary//%/%25}
  echo "::error title=${title}::exit ${code}: ${summary:0:3500}"
fi
rm -f "$log"
exit "$code"
