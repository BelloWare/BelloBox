# App-controlled recording/converter shutdown validation

Scope: registered recording/converter media jobs. This is not live capture,
complete native recording parity, or a drain guarantee for unrelated OCR/capture
and utility workers. Native platform Quit/Cmd-Q remains an unverified route into
GPUI's non-vetoable termination callback; forced termination/OS shutdown retain
best-effort cancellation and recovery only.

## Defect and correction

The historical `77b9…` GUI candidate exited about 823 ms after Stop during a fixed
five-second injected finalization. A capture stage survived process exit. Its
original screenshots 18–19, timestamps and 174-input manifest remain in
`../recording-host-2026-10-07/`. Twenty-three pre-edit source snapshots preserve
changed inputs for that earlier binary.

An immediately-ready `on_app_quit` future does not prove physical drain. The new
App-global one-shot latch signals cancellation and rejects new media admissions.
Worker-owned tickets retire only after native callback/pool and terminal owner
cleanup. Converter/MOV/image jobs reserve admission before constructing their
owning closure; their completed payload stays in a bounded worker-owned slot until
explicit current-owner adoption. Rejection, close and panic dispose the slot before
releasing the ticket. Readiness or a display timer never owns that transfer.

Pinned GPUI 0.2.2 X11/Wayland independently stop their event loop at zero native
windows. Native titlebar close and every production programmatic close therefore
share a final-window veto: the existing window renders “Closing…” until registered
media retirement finishes. No replacement window is created inside X11's borrowed
native close callback. Default Home/settings/launcher/clock windows also use this
veto, covering a recording that closed before another tool became the last window.
Existing screenshot discard/export decisions stay ahead of the veto. There is no
timeout that turns unfinished physical work into a successful drain.

## Completed checks

- 23/23 platform recording tests, including callback-held app-retirement owners
  and 64 registration-vs-physical-drain races: `platform-recording-tests.txt`
- Strict App/platform all-target Clippy with recording-fixtures after the final
  close routing: `app-platform-clippy.txt`
- Apple-target platform all-target Clippy with recording-fixtures passed as
  metadata/type checking only: `apple-platform-metadata.txt`
- 80/80 focused-module tests, 30.92 seconds: `focused-module-tests.txt`
- Production-only recording-fixture whole-app build and strict Clippy passed:
  `production-app-build.txt`, `production-app-platform-clippy.txt`
- All 179 source/asset/config/script input paths and hashes were unchanged across
  the production build; full compiler and flag provenance is in
  `production-source-inputs.json` and the before/after files

The production binary SHA256 is
`bdfc718a20ad84be9f59dc52b459ef0f02c275fa0525c2f6b82dd60ad1d28a54`
(143,488,824 bytes; default developer-tools plus recording-fixtures; codegen units
256; one build job). The GUI worker uses this unmodified staging build.

A later literal-include audit found two additional inputs outside the earlier
crate-root scope: `project.yml` and the application icon. Both match tracked
baseline `0041012edc718a53f9f7bdf7e9889507d2a9817f`, and each complete byte sequence
is embedded in the unchanged frozen binary. This later evidence is recorded in
`later-include-input-audit.json`; it does not rewrite the original 179-input
before/after observation as a retroactive 181-input build check.

## Focused harness boundary

The temporary harness contains 17 original module/test/helper files. Production
media, shutdown, disposal and theme modules are byte-identical to staging. To
satisfy the unchanged theme reexports, exact pure Home label/icon helper item
ranges were copied with byte-range/hash attribution. Registry dependency versions
and checksums match the original lock. Full source and copied-test identities are
checked again after execution in `focused-module-provenance.json`.

The only edited included test file omits exactly
`shutdown::tests::latched_quit_rejects_new_tool_and_launcher_windows`, which depends
on the complete desktop router. Unrelated desktop, screenshot/OCR, launcher,
settings, clock and utility module trees are omitted. Native/movie fixture cfg
exclusions are unchanged. The normal App test inclusion and CI are untouched.
Harness crate-local unused/dead-code warnings are permitted for omitted call sites;
normal App strict Clippy remains enforced. A complete harness/source archive is
retained in the execution workspace as `recording-shutdown-focused-validated.tar.gz`
(SHA256 `6e40c022099938d61c30ae18f5514418436d64de4d75e6f386edb88912a15161`).

These 80 results are focused-module evidence, not a full App test pass. They cover
held workers and retained entities, physical callback ownership, final-window
retention, replacement-before-latch behavior, rejected terminal disposal, precise
adoption, wakeups, panic recovery/retry, Save/GIF preservation and preview disposal.

## Detecting controls and restoration

Two mutations were applied only to the temporary harness's shutdown module:

- Bypassing physical-drain await failed the held-worker regression because quit
  was called once while the test required zero calls.
- Bypassing the final-window veto failed the same regression because zero windows
  remained while the test required one retained native window.

The exact baseline module hash
`4600a62a8d07ce56be21f77f7861f2d904bc2997bd515f4ca701bf99499fe2f3`
was restored. All 80 focused tests passed again (30.44 seconds), and both staging
source and the frozen GUI executable were verified unchanged. Mutation sources,
expected assertion failures, restored positive output and exact commands are in
`detecting-controls/`. These controls strengthen focused-module evidence; actual
whole-app GUI and exact full CI remain separate requirements.

## Blocked and pending checks

The complete App fixture test executable was SIGKILLed during code generation with
default settings, with package-only 256 codegen partitions, and through the
approved cloud-terminal retry. These are failed build attempts, not test failures
or test passes. Logs are retained as `app-*-sigkill.txt`; the partitioned attempt
reported maximum child RSS 589,824 KiB. Available-memory snapshots and command
arguments are in `app-partitioned-resource.json`. The exact cause is not asserted.
The failed terminal attempt's source path set/hashes were verified unchanged.

Repaired whole-app Linux process replay now passes the specified generated-media
paths on the exact frozen binary. Held Start exited after3.195s, held Finish after
5.094s, recording-first/Home-last after5.083s and delayed GIF close after5.446s,
all with a visible retained Closing window and exit0. Incomplete stages were
absent; completed MOV and prior GIF hashes were preserved. Repeated Close and
pending Save dialog retirement also passed. A deterministic GUI Close during the
physical MOV copy itself was not exercised; this remains distinct from pending
dialog close and the platform/owned-review Save tests. See `validation-gui.json` and its
original screenshot/process evidence. These are observed generated Linux paths,
not a hard native latency bound. Exact full App CI execution and generated native
Apple execution remain gates. Earlier green OCR or movie CI does not apply to this
unpublished shutdown correction.


## Requested CI gates

Both `.github/workflows/rust.yml` and `rust-macos.yml` are based on published
`1f5874ce741b8de21fa09c747a5bada7488e2982` and preserve public-repository-only job
guards. New jobs/steps use one Cargo job and run the complete recording-feature
App suite plus recording-feature platform tests. The macOS platform suite includes
real generated-writer finalization and held callback destruction; ordinary capture
admission remains closed. Existing OCR transport/controller/inline filters are
unchanged. YAML syntax and exact command/guard assertions passed locally in
`ci-workflow-checks.json`; no execution of the new CI checkpoint is claimed.

## Final independent source accounting

The [sealed LOC audit and reproducible archive](../recording-loc-2026-10-07/README.md)
report 52,593 production / 31,183 support / 105 benchmark physical nonblank Rust
lines, with deltas +2,540 / +2,795 / 0 against1f587. Complete source classification,
163-file/binary binding and24 detecting controls passed. This final overlay adds
only docs/evidence to the sealed implementation handoff, with no production change.
