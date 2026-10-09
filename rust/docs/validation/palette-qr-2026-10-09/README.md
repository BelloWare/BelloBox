# Palette QR validation — 2026-10-09 UTC

Immutable source and ordinary binary identities are in final-source-manifest.json.
This report covers portable/core and actual GPUI test-platform paths. A real
Linux desktop session is a separate acceptance stage, as are native macOS IME,
clipboard, sheets, Retina output, accessibility and independent scanner decoding.
No screenshot, desktop capture or binary is published in this evidence directory.

The ordinary binary was built with default features, without movie-fixtures or
recording-fixtures, using the pinned Rust 1.99.0 toolchain and existing supported
Linux build prerequisites. Production/native admission gates were not changed.
No dependency, workflow, Clock implementation, shared editor, movie/recording or
native-host test files were changed. desktop.rs has exactly one call-signature
adaptation after removing the obsolete terminal-preview payload; the independent
core terminal/CLI export is unchanged.

## Focused evidence

The initial implementation’s sixteen actual GPUI regressions cover:

- Retained palette-owned draft across row/search navigation; exact edited and empty
  handoff; original selection retained for other tools; replacement gets a new owner.
- Actual full QR popup Open followed by real pointer/editor input and clipboard
  copy verifies the edited payload and explicit empty draft without fallback.
- Real dispatched Tab/Enter/Clear, held Return and key release prove no accidental
  newline or second activation; a fresh editor Return subsequently adds a newline.
- IME composition ownership, Escape-to-search/held Escape, arrows and Tab traversal.
- Enlarge/Compact preserve editor identity and real editor Undo history.
- Current-image copy bytes, empty/nontext Paste recovery, stale success/error,
  same-text ABA generation identity, retirement/reactivation, and byte boundaries.
- Actual test-platform Save dialog Cancel/reopen; edit while destination pending;
  file bytes match click-time PNG; existing output is not overwritten; late status
  cannot replace newer edit status. A pending Save blocks launcher Open/close.

Four added core tests compare actual encoder modules against decoded PNG pixel
blocks/quiet zones, compact/enlarged geometry, density, export equality and limits.
These tests do not establish scanner success or native rendering parity.

The full launcher/Clock regression filter passed 40 tests with one existing
isolated-child entry deliberately ignored before the terminal-payload cleanup.
The final-source and pre-hardening aggregate gates are distinguished below.

After independent review, QR was rebased without conflicts onto published PR6
25e78766d2463c186e7dbdf4d85029401290d062. Resulting code commit is
3adb503d2624033137e0b714c62e80d2033187ca, tree
a8f20071cf00303e707c1026a0ab03cb52de589d. All eight reviewed postimage hashes
remain identical; no QR source conflict resolution occurred. The initial seal
is preserved separately in interim-seal-before-rebase.txt and is not the final
GUI acceptance binary. Applicable checks are rerun on the integrated base.

## Corrected development attempts

Initial compile iterations caught a delimiter error, wildcard-import test-macro
recursion, the clipboard image reference signature, an unused test import, and a
private GPUI test dispatch return type. The dispatch test now uses the supported
VisualTestContext input path. The full-popup test initially typed without focusing
its existing editor; an actual pointer click fixed the test setup. Independent
review caught held activation after Clear and the over-500,000-byte notice; both
received explicit regression coverage. Strict Clippy found the obsolete terminal
payload, which was removed with the narrowly approved caller adaptation rather
than a warning suppression.

## Final bounded-title hardening

A final source inspection identified an avoidable whitespace scan in the render
heading for already-rejected input up to the unchanged 8 MiB editor cap. Commit
c301cc213784c3725436e2316da2196e05f6ff49 (tree
8242b8ad027ad046755c6d6e70819e07828e9e87) extracts the unchanged heading choices
and short-circuits the empty-text trim at the QR 2,000-byte admission boundary.
The new actual GPUI regression covers 2,000/2,001/500,001/8 MiB whitespace, retained
exact byte lengths and no accepted image. No encoder admission or editor cap changed.
The two affected postimages supersede the earlier review hashes; the focused
source-only delta receipt is bounded-title-delta-review.md. Remaining postimages
are identical. source-manifest-c301cc213784.json binds that intermediate source and binary. The current seal below supersedes it.

On this final source, launcher/Clock suites in default, minimal and movie-fixtures
profiles each passed 41 tests with one existing isolated-child entry ignored.
Strict workspace default, minimal App/core, and movie-fixtures App/platform
all-target Clippy passed, as did formatting. The full aggregate suites below were
run on 3adb503d before this two-file title-only hardening; they are not mislabeled
as a full aggregate rerun on c301cc21. Final relevant suites and strict checks are
recorded in bounded-title-*.log.

After those gates, cargo clean -p bellobox-app removed the prior App artifacts,
and cargo build --locked --offline -p bellobox-app --bin bellobox built the ordinary
default binary with neither fixture feature. Its read-only sealed copy hashes to
53382eb395d15f54c5689a7a4e661659e01efe4829e85beb8c4396b8d71d8bb6.
The initial pre-rebase seal is retained only as historical evidence. Actual desktop
acceptance is a separate, root-owned report and does not inherit test-platform
clipboard/dialog success.

## Results

- cargo fmt --all -- --check: pass on final source.
- cargo clippy --locked --offline --workspace --all-targets -- -D warnings: pass.
- Pre-rebase cargo build --locked --offline --workspace: pass (build-default.log). Final ordinary clean build: pass (final-clean.log, final-build.log); final seal is separately bound above.
- Rebased cargo test --locked --offline --workspace -- --test-threads=2: pass; App 422 passed/2 existing ignored, core449 passed, remaining workspace/integration suites passed (rebased-workspace.log).
- Rebased cargo test --locked --offline -p bellobox-app -p bellobox-core --no-default-features -- --test-threads=2: pass; App422/2 existing ignored, core368 passed (rebased-minimal.log).
- Rebased cargo test --locked --offline -p bellobox-app --features movie-fixtures -- --test-threads=2: pass; App422/2 existing ignored (rebased-movie-fixtures.log).
- Rebased strict workspace default, App/core minimal and App/platform movie-fixtures all-target Clippy: pass (rebased-clippy-*.log).
- Python perf harness: 4 tests passed.
- Python UI evidence harness: 14 tests passed.

The existing dependency future-incompatibility warning for proc-macro-error2 2.0.1
is unchanged; it is not a new strict-lint failure.


## Actual-desktop and native-preflight corrections

Actual Linux GUI validation of the c301cc21/3976bd86 source found two failures:

1. GPUI X11 Copy Image reported success but only text targets were externally
   advertised. Its cached in-process Image was not external PNG delivery. The
   pinned Wayland implementation similarly offers text/private MIME only. The
   palette now queries the actual compositor and disables Copy on X11, Wayland,
   headless and unknown platforms, skips it in Tab order, keeps a Save PNG notice,
   and refuses programmatic activation without changing the clipboard. The native
   macOS route is enabled only with the expected compositor and macOS target.
   TestPlatform image-copy allowance is explicitly test-only synthetic coverage.
2. Holding Return on Save handed auto-repeat into the GTK chooser and accepted the
   default filename. This was a failure, not acceptance. Keyboard Save now waits
   for matching key release. A second Enter/Space cancels the armed operation;
   ownership persists until all tracked activation keys are released. Edits,
   Escape/focus loss, Tab navigation, pointer action and retirement cancel it.
   Mouse/programmatic Save while any tracked key is held refuses without replay.
   Normal mouse Save is unchanged.

A new focus-away/back test initially failed because GPUI may coalesce blur events
within one frame; Tab now cancels at the navigation event itself. The failed
regression is retained in overlap-final-default.log. The attempted pointer-capture
method name was corrected to GPUI's supported capture_any_mouse_down; its compiler
failure is retained in overlap-final-default-api-correction.log.

Separately, the native peer’s first preflight of source-only remote candidate
3976bd8632d8ecfaba9350b750f46e6e457b68d3 built all three configurations but reported
67 passes and six test failures across configurations (29 distinct selected tests).
The two failures repeated across configs were density-toggle Undo and full-popup
text Copy. Their test inputs hard-coded Linux Control shortcuts; actual shared
editor bindings use Command on macOS. Only those test keystrokes were corrected
(cmd-z and cmd-a/cmd-c on macOS, Control on Linux). No production shortcut workaround
or weakened assertion was introduced. Original peer evidence remains with the
root-owned native receipt/comments 6073440852 and 6073460364; this local correction
is not a claim that the native retry passed.

## Intermediate freeze after those corrections

Source: a78203800a1c5d462814991da4cf65e60141a672.
Tree: 5415e0154fdf369f28e83a89a8f56e5436135449 (includes earlier text receipts).
The source-only remote/native candidate may have a different documentation tree;
compare the exact eight postimages in final-source-manifest.json.

- Default/minimal/movie-fixtures launcher suites each passed 49 tests, one existing
  isolated-child entry ignored. This includes actual dispatched hold/release,
  both overlapping-key orders and release orders, remaining-key auto-repeat,
  Escape/focus ABA, edit ABA/retirement, and mixed mouse/programmatic refusal.
- Strict App all-target Clippy in all three profiles and formatting passed.
- Clean ordinary default build succeeded with no fixture features.
- Final sealed binary SHA-256:
  d4702add5fe9b03b38ce71f98d6993d1adf9514559f8c6dd57ee88588be82069.
- Logs: final-modal-*.log and modal-seal-*.log. Earlier full aggregate results are
  deliberately retained with their earlier source scope; they are not relabeled.

The GUI worker received this new ordinary seal for changed-path retesting, and
native peer retry is root-owned. Their final outcomes are separate receipts. No
external Linux PNG clipboard success, scanner success or macOS runtime parity is
claimed by these portable tests. No screenshots are included in this directory.


## Whole-window physical-key follow-through

Independent final review found a remaining blocker in a7820380: the button-owned
key mask did not see Enter/Space held in the QR editor or search. Clicking Save
while such a key was held could still hand auto-repeat to a chooser. The earlier
GUI pass covered button-origin keys, not this editor/search-origin path; it remains
intermediate evidence, as do its 49-test results. The failed source review is
retained in final-clipboard-keyrelease-review.md.

The final two-file correction adds a window-owned, shared two-bit physical key
observer. Both root capture handlers update it before IME early returns without
consuming the input. The retained QR entity receives the same state even if it is
created after a search-field key-down. Save checks both observed physical keys and
button-owned keys. Releases clear physical state even after focus/row changes;
observing a key never arms Save. Save ignores GPUI synthetic keyboard-click fallback,
so a text-origin key-up cannot replay a declined mouse action after focus moved.
This is palette-window event observation, not a global keyboard hook.

Three new actual GPUI regressions exercise editor Return with a regenerated preview,
search Space held before QR creation, and both editor/query composition paths. They
use actual mouse down/up at the rendered Save button, prove no modal before release
or on the stale release, preserve newline/marked-text input semantics, and prove a
fresh mouse activation succeeds after release. The source-only reviewer rechecked
both exact changed-file hashes and reported no further static blockers; see
whole-window-keytracking-review.md.

Latest source freeze: 311a5717665308e3390cbc84aeb057ac5344d5a1.
Tree: ef60c94814f28d378608f3f99825682e526bc7c8 (includes prior receipts).
Default, minimal and movie-fixtures launcher suites each passed 52 tests, with one
existing isolated-child entry ignored. Strict App all-target Clippy in all three
profiles and formatting passed. Clean ordinary build succeeded; binary SHA-256:
d68fa99b81d6b00a7801b5d06137bedee5e4be6bef5a1cbd3da30507c61ea618.
See window-physical-*.log and the latest final-source-manifest.json; earlier source
manifests/seals remain preserved under their named files.

The native retry of intermediate source-only 3ae7349b independently passed three
builds and 97 selected test executions (37 distinct tests), with no failures,
ignores or compiler warnings. The corrected Command-shortcut tests passed in every
configuration. Strict native lint still had the unchanged 25e baseline blockers;
it was not a strict-lint pass. Those results precede this final two-file correction
and are not inherited as its native validation. Final native and actual GUI retry
remain separately hash-bound, coordinator-owned receipts.


## Deterministic learned-ranking fixture correction

The native retry of intermediate source-only 83ae reported 105 passes and one
movie-fixtures failure in search_space_before_qr_creation_survives_focus_change_without_save_replay.
The assertion that QR did not yet exist depended on initial ranking; earlier
explicit opens could prelearn QR as the first row. The first failure remains in
the peer receipt/comment 6073791483. This was not a production modal-guard failure.

Test-only source 3cd1404b20afa0f47ffd392abe83a8cd88d9364e now deliberately seeds four
QR uses into in-memory settings, verifies QR ranks first and exists, then switches
to a guaranteed no-match query and explicitly retires that retained session before
the Space key-down. It asserts no commands/no QR both before and after the down,
verifies the search text received the space, and keeps the actual new-session mouse
Save refusal, release-no-replay and fresh mouse recovery assertions unchanged.
The fixture never saves its seeded settings or changes production ranking behavior.

The exact regression passed three times in each default/minimal/movie-fixtures
profile (9/9 executions), including the forced prelearned state each time. Strict
App all-target Clippy and formatting passed. The native retry of this corrected
fixture is mandatory and separately coordinator-owned; local repeated success is
not native acceptance.

The entire launcher_ui.rs region preceding its final positive cfg(test) module is
byte-identical to source 311a. A clean ordinary rebuild compared equal using cmp to
the existing GUI seal; its SHA-256 remains
d68fa99b81d6b00a7801b5d06137bedee5e4be6bef5a1cbd3da30507c61ea618.
Thus the existing ordinary GUI binary remains exact, rather than merely presumed
production-equivalent. See deterministic-search-*.log, the equivalence receipt, and
the latest final-source-manifest.json. Earlier manifests, seals and failure records
remain retained under their named files.
