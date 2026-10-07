# Native Window catalog bridge: scoped acceptance

Date: 2026-10-07 UTC. Baseline rust commit
505e8f193de858163474e2af269d2e3ca51e475a, tree
8477ad4e3b0a7da76db42de83530faccc5cb6f6a. Application Area/Window, platform Window
and native movie activation remain closed. No live native acquisition or permission
call was performed.

## Exact candidate

The immutable Linux GUI binary is SHA-256
`d328600a3a4c11a0532ea16ab3c545a96290d61b1ec04cf14c826b02974a414d`.
[source-inputs.json](source-inputs.json) records exact changed-source and manifest
hashes relative to the baseline. All supplied GUI routes enter the same normal-
compiled coordinator, pending decision, acquisition, masking and delivery path.
Only records/pixels and explicit delay/failure controls are generated fixtures.

## Final automated checks

- 103 screenshot application regressions passed.
- The exact app-only debug-assertions-disabled test executable passed 95 screenshot
  regressions. Dependencies retained ordinary debug settings. This is not a full
  release-workspace execution claim.
- 135 platform regressions passed, with 3 existing ignored subprocess tests.
- 169 core screenshot regressions passed, including the consuming deferred-decision
  original base/context/cancellation ownership test.
- Strict Linux app/platform all-target Clippy, minimal-feature app check, normal
  app build and app-only no-debug metadata compile passed.
- Apple Silicon-target normal and synthetic platform targets passed metadata/type
  Clippy with --all-targets --no-deps and -D warnings. This was cross-target checking
  on Linux, not Apple SDK linking or native execution. Exact new-commit macOS and
  Linux CI remains pending publication.

Commands and scope are preserved in [validation.json](validation.json). The
pre-existing proc-macro-error2 future-compatibility notice is not a source lint
failure. Focused native --no-deps scope excludes an unrelated pre-existing macOS
core/settings needless-return warning; no platform warning is suppressed.

## Independent review and detecting regression changes

1. The public closed Window gate now precedes deadline arithmetic. A malformed
   Duration::MAX request returns Unavailable without overflowing Instant addition.
2. The first Window catalog shares native admission with the full-display freeze.
   Review identified that Display Job completion precedes callback/encoder lease
   release. A Window-only freeze entry now physically drains this dependency before
   catalog admission; ordinary Display/Area return timing remains unchanged.
3. A successful result held during physical drain is deadline/cancellation checked
   again afterward. Catalog also rechecks boundary cancellation; independent Window
   also rechecks its original selection and session. The held-lease test receives
   an explicit successful-Job-result-consumed signal before changing the deadline
   or cancellation, so scheduler delay cannot hide a missing post-drain check.
4. UI apply performs no observation. Final worker observation happens after masking.
   Its proof is fresh at that observation, with cheap owned guards at delivery;
   no OS incarnation/revision feed or atomic/live-native freshness is claimed.
5. Fake native contexts cover successful, cancelled and expired dispatch ownership;
   constructed CF rows cover strict/raw separation, malformed/missing fields,
   exact fractional geometry, numeric overflow, 512-row bounds, ordering and
   ownership after CF teardown. No native catalog or screenshot API is invoked.
6. The actual GPUI coordinator mounts usable frozen pixels before queued optional
   observation, retains edits/one-shot Undo rejection, refuses successor admission
   while cancelled physical work is pending, then permits reopen after drain.
   Final-observation success arriving after deadline keeps frozen pixels.

Three new-test setup errors were corrected without weakening production guards:
retain the selector owner while materializing its commit; expect the initial
prepared-font revision; and compare native availability against the new platform
closed-gate message. All final checks above include those corrections.

## Actual supplied-pixel Linux interaction

All interaction used the immutable binary above and real desktop pointer/keyboard
input. The process-local FileChooser launch used the exact outer command:

    dbus-run-session -- bash /workspace/scratch/8b6fda578834/build-environment/native-window-qa/launch.sh window

The retained [launch-recipe.sh](launch-recipe.sh) is that inner script, not a command
to run on the ordinary desktop bus. The same outer invocation used
observation-delayed, failure and area in place of window. Official package hashes,
FileChooser-only portals.conf and XDG_DESKTOP_PORTAL_DIR prerequisite are documented
in [cloud-dialog-acceptance.md](../../cloud-dialog-acceptance.md). The private portal
reported FileChooser version 4. Its trap retires only its own child processes;
no global desktop service/settings, permissions or capture interface changed.

1. Blank clicks and long drags remained in Window selection. Front selection
   retained frozen orange RGB with independent rounded alpha. The chosen frame
   remained fixed. [01-front-window.jpg](01-front-window.jpg).
2. Cropped to the front Window body. One Undo restored the full image and Redo
   restored the same body-only crop. The image aspect-fit in clean well bands,
   with no Area resize handles. [02-fixed-crop-redo.jpg](02-fixed-crop-redo.jpg).
3. Actual GTK Save opened, cancelled with no file, reopened and completed. The
   editor reported Saved PNG. [03-save-dialog.jpg](03-save-dialog.jpg),
   [04-cropped-save.jpg](04-cropped-save.jpg). The preserved
   [PNG](saved-cropped-window.png) is exactly 401×161 RGBA; every one of its 64,561
   pixels is [255,224,186,255]. It contains no excluded header, full-display or
   letterbox pixels. Fractional pointer coordinates were not traced; the observed
   401×161 result is recorded rather than rounded to an assumed 400×160.
4. Copy & Finish retired the overlay and cleared capture busy. Paste Image
   reimported the actual clipboard as 401×161, showing only the cropped orange
   body. [05-clipboard-reimport.jpg](05-clipboard-reimport.jpg). It closed cleanly.
5. Reopened and selected the exposed back Window. Independent blue pixels replaced
   the orange frozen overlap. [06-back-window.jpg](06-back-window.jpg).
6. observation-delayed paused the first optional observation for eight seconds.
   The usable frozen back editor mounted before it returned. A rectangle was
   completed 908 ms after selection, while observation was still pending. At
   21,533 ms the rectangle and original frozen overlap remained. Undo removed
   the mark without reviving the rejected one-shot refresh.
   [07-observation-pending-edit.jpg](07-observation-pending-edit.jpg),
   [08-observation-edit-preserved.jpg](08-observation-edit-preserved.jpg),
   [09-one-shot-undo.jpg](09-one-shot-undo.jpg).
7. With failure, an edit completed 942 ms after selection, before the controlled
   eight-second acquisition failure. At 23,531 ms the frozen pixels, orange
   overlap and edit remained usable. [10-failure-edit-preserved.jpg](10-failure-edit-preserved.jpg).
8. A fresh delayed-observation editor closed 465 ms after selection, before the
   observation returned. At 23,503 ms the chooser was idle and no late editor
   reappeared. [11-close-no-late-return.jpg](11-close-no-late-return.jpg). This
   fixture cooperatively cancels; native physical Busy retention is established
   separately by the held-context/lease regressions, not inferred from this quick
   GUI closure. Subsequent fixture launches opened normally.
9. On the same binary, Area selection retained eight handles. Dragging its lower
   corner resized the selection; one Undo restored it. Select then moved the
   selection without changing its size. Settled pixels remained correct and
   Copy & Finish retired the overlay, returning the chooser to idle.
   [12-area-resize-undo-move.jpg](12-area-resize-undo-move.jpg).

The timing values above establish that edits/closure occurred inside the explicit
fixture delay; they are not native latency or responsiveness measurements. Initial
and changed previews transiently showed an empty well while asynchronous tiles
loaded, then settled correctly. Keyboard-only restoration of the notification-style
overlay after a system dialog is not claimed; pointer refocus was used.

All twelve JPEGs are original app/dialog-only screenshot bytes, with no cropping,
transcoding or image editing. The PNG is the actual file written by the app. No
raw desktop/terminal capture or user screenshot is retained. The matrix ended with
all BelloBox/overlay/dialog windows closed, its private portal session retired,
and the shared terminal idle. Source and immutable binary hashes were verified
unchanged after interaction.

## Source accounting and limits

The independently verified scoped net delta is +1,386 production and +387 support
nonblank physical Rust lines, including comments; benchmark delta is zero. Totals
are 46,738 / 24,200 / 105. The manifest separately reports 659 preserved support
lines promoted to production and 22 in reverse, plus actual added/removed lines.
The existing native Window candidate is explicitly reclassified rather than
counted as unchanged support. See [loc-delta.json](loc-delta.json). These counts
are not feature-completion percentages or engineering-effort measurements.

Native catalog still performs two CG queries and four topology passes on the main
queue. Bounded parsing and asynchronous ownership do not establish native UI
responsiveness. Real callback lifetimes, queue delivery, TCC/AX, AppKit/Spaces,
Retina/hotplug, protected content and original-capture color/ICC/HDR remain open.
The portable mask sampler remains the previously documented approximation for
±1-pixel sampling; this work does not broaden normalized SDR oracle claims.
