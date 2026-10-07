#!/bin/bash
exec dbus-run-session bash /workspace/scratch/8b6fda578834/build-environment/recording-shutdown-qa/gui/launch-inner.sh "${1:-standard}" "${2:-recording}"
