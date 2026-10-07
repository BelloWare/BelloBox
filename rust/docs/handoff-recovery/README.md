# Recovery source inventory

The current rust branch is authoritative. Historical worktrees were not reset or
deleted. Their source/index bytes were compared by Git blob identity against
published checkpoints. `source-survival.json` records exact matches and the few
remaining historical variants. The five tiny variant patches preserve pre-merge
wording, stale evidence text, module ordering or missing later integration edges.
They are superseded: **DO NOT APPLY them to current rust**. They are retained only
to prevent loss of otherwise uncommitted source versions.

A variant is reconstructible from its reference commit plus the listed patch.
No credentials, machine configuration, private conversation, internal notes,
compiled binaries, package caches or raw diagnostic logs are included here.
The separate alpha validation WIP, if present, is intentionally uncompiled and
must be reviewed/tested before integration. It must never enable native capture.
