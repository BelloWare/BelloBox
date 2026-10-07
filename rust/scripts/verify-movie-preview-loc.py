#!/usr/bin/env python3
"""Verify the reviewed movie-preview delta and exact source hashes, not a cfg parser."""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--repo', type=Path, default=Path('.'))
args = parser.parse_args()
repo = args.repo.resolve()
manifest = json.loads((repo / 'rust/docs/validation/movie-preview-2026-10-07/loc-delta.json').read_text())
sums = {name: 0 for name in manifest['totals']}
for entry in manifest['files']:
    path = entry['path']
    before = subprocess.run(['git', 'show', f"{manifest['baseline_commit']}:{path}"], cwd=repo, capture_output=True)
    before = before.stdout if before.returncode == 0 else b''
    after = (repo / path).read_bytes()
    for label, data in [('before', before), ('after', after)]:
        assert hashlib.sha256(data).hexdigest() == entry[label + '_sha256'], (path, label, 'hash')
    def classify(data, ranges):
        lines = data.decode().splitlines()
        assert all(1 <= start <= end <= len(lines) for start, end in ranges)
        support = {line for start, end in ranges for line in range(start, end + 1)}
        return [(i, line.strip(), 'support' if i in support else 'production')
                for i, line in enumerate(lines, 1) if line.strip()]
    old = classify(before, entry['before_reviewed_support_ranges_1based'])
    new = classify(after, entry['after_reviewed_support_ranges_1based'])
    assert len(old) == entry['before_nonblank'] and len(new) == entry['after_nonblank']
    actual = {name: 0 for name in sums}
    for tag, a, b, c, d in difflib.SequenceMatcher(a=[x[1] for x in old], b=[x[1] for x in new], autojunk=False).get_opcodes():
        if tag == 'equal':
            for left, right in zip(old[a:b], new[c:d]):
                if left[2] != right[2]:
                    actual[left[2] + '_to_' + right[2]] += 1
        else:
            for line in old[a:b]: actual['removed_' + line[2]] += 1
            for line in new[c:d]: actual['added_' + line[2]] += 1
    for category in ['production', 'support']:
        actual[category + '_delta'] = sum(x[2] == category for x in new) - sum(x[2] == category for x in old)
    assert all(entry[name] == value for name, value in actual.items()), (path, actual)
    for name, value in actual.items(): sums[name] += value
assert sums == manifest['totals']
assert sums['production_delta'] == sums['added_production'] - sums['removed_production'] + sums['support_to_production'] - sums['production_to_support']
assert sums['support_delta'] == sums['added_support'] - sums['removed_support'] - sums['support_to_production'] + sums['production_to_support']
for category in ['production', 'support']:
    assert manifest['baseline_counts'][category] + sums[category + '_delta'] == manifest['result_counts'][category]
print(json.dumps({'verified_files': len(manifest['files']), 'totals': sums, 'counts': manifest['result_counts']}, indent=2))
