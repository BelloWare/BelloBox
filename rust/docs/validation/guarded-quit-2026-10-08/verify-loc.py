#!/usr/bin/env python3
"""Verify this exact reviewed delta. Explicit spans are not a general cfg parser."""
import hashlib
import json
from pathlib import Path
import subprocess

record = Path(__file__).resolve().parent / 'loc-delta.json'
repo = record.parents[4]
data = json.loads(record.read_text())
totals = dict(production=0, support=0, benchmark=0)
for entry in data['files']:
    path = entry['path']
    old = subprocess.run(['git', 'show', data['baseline_commit'] + ':' + path],
                         cwd=repo, capture_output=True)
    if old.returncode:
        assert entry['before_sha256'] == hashlib.sha256(b'').hexdigest(), path
    for label, source in [('before', old.stdout), ('after', (repo / path).read_bytes())]:
        assert hashlib.sha256(source).hexdigest() == entry[label + '_sha256'], (path, label)
        lines = source.decode().splitlines()
        spans = entry['reviewed_support_ranges_1based'][label]
        assert all(1 <= first <= last <= len(lines) for first, last in spans)
        support = {line for first, last in spans for line in range(first, last + 1)}
        counts = {category: sum(bool(text.strip()) and
                               ((number in support) == (category == 'support'))
                               for number, text in enumerate(lines, 1))
                  for category in ('production', 'support')}
        assert counts == entry['counts'][label], (path, label, counts)
    for category in ('production', 'support'):
        delta = entry['counts']['after'][category] - entry['counts']['before'][category]
        assert delta == entry['delta'][category]
        totals[category] += delta
assert totals == data['delta'], totals
print(json.dumps({'verified_paths': len(data['files']), 'delta': totals}, indent=2))
