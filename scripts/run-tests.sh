#!/usr/bin/env bash
set -euo pipefail

# Regenerates the Xcode project from project.yml (if xcodegen is available) and
# runs the BelloBox unit tests.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if command -v xcodegen >/dev/null 2>&1; then
  xcodegen generate >/dev/null
fi

DESTINATION="${DESTINATION:-platform=macOS}"

xcodebuild test \
  -project BelloBox.xcodeproj \
  -scheme BelloBox \
  -destination "$DESTINATION" \
  -only-testing:BelloBoxTests \
  "$@"
