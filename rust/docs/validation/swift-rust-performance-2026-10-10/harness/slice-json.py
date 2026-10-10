#!/usr/bin/env python3
"""Cut the 296 KB benchmark JSON array into smaller valid documents at top-level
element boundaries, so every number keeps its exact text.
Usage: python3 -I slice-json.py json-300k.json  (writes json-30k.json, json-100k.json)"""
import json, sys

src = open(sys.argv[1], encoding="utf-8").read()
depth, in_str, esc, ends = 0, False, False, []
for i, ch in enumerate(src):
    if in_str:
        if esc: esc = False
        elif ch == "\\": esc = True
        elif ch == '"': in_str = False
        continue
    if ch == '"': in_str = True
    elif ch in "[{": depth += 1
    elif ch in "]}":
        depth -= 1
        if depth == 1: ends.append(i + 1)
for target, name in [(30_000, "json-30k.json"), (100_000, "json-100k.json")]:
    text = src[: max(e for e in ends if e <= target - 1)] + "]"
    json.loads(text)
    open(name, "w", encoding="utf-8").write(text)
