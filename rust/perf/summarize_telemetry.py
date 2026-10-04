#!/usr/bin/env python3
"""Summarize actual application telemetry by event and unit; never infer FPS."""
import argparse
import collections
import json
from pathlib import Path
from measure import summary

ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument('path', type=Path)
a = ap.parse_args()
groups = collections.defaultdict(list)
malformed = 0
for line in a.path.read_text().splitlines():
    try:
        event = json.loads(line)
        name = event.get('event', event.get('name', 'unknown'))
        # Accept durations only. elapsed_ms in app logs is a timestamp since
        # launch, not a duration of the event. Never summarize it as latency.
        for field in ('duration_us', 'duration_ms', 'duration_microseconds', 'last_input_microseconds', 'render_us', 'render_ms'):
            value = event.get(field)
            if isinstance(value, (int,float)) and not isinstance(value,bool) and value >= 0:
                groups[(name,field)].append(value)
    except (ValueError, TypeError, AttributeError):
        malformed += 1
print(json.dumps({'source': str(a.path), 'invalid_lines': malformed,
                  'warning': 'Interpret each event by its declared measurement: input-to-paint includes event-loop waiting. These events do not measure GPU presentation or input-to-photon latency.',
                  'events': [{'event': name, 'field': field, **summary(values)} for (name,field),values in sorted(groups.items())]}, indent=2))
