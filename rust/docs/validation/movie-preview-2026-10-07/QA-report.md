# Paused movie review: actual cloud GUI acceptance

First (broad) candidate: `cd5e9337da64da0575737607ece52ab66f4bf7a966d7505c7e7982f822b8a876`.
Its exact `source-inputs-first-candidate.json` hashes and all 20 input files under
`pre-fix-source/` remain available. Screenshots01–20 belong to that candidate.
It was stopped after the cancellation caption defect described below was found.

Final repaired candidate: `5d9a996ced75be919acf97474fe59ba224df268afb935746e38a4e1f20b9edba`.
`source-inputs-final-gui.json` pins its exact GUI-run inputs. The current
`source-inputs.json` additionally records the post-GUI test-only normalization
noted below. `post-cua-repair.diff` records the exact narrow
change. Final scoped/remainder interaction is complete; screenshots 21–35 belong to this
repaired binary and are listed separately in validation.json.
Both candidates were copied read-only before interaction; no source was changed
within a GUI run. Baseline is `3f370a45b9baca8101b82b2c6b8c924b1096a1b7`.

The private FileChooser portal recipe is retained alongside this report. No
system service/configuration, capture permissions, credentials or paid service
was touched. These are generated pixels on Linux, not native movie decoding.
The following broad observations belong specifically to the first candidate.

## Normal review, trim, Save and Open

- Initial preview is visibly paused at requested/source PTS 0.000. Start and End
  handles update the source time marker. End seek is visible at 1.500 seconds.
  Precise numeric editing and Tab between fields apply a 0.5–1.5-second trim.
- Actual system Save Cancel creates no GIF and retains source/trim. Reopening Save
  writes `once.gif` through the real staged exporter. Result appears paused; Play
  animates it and Once stops at its final frame. Movie switches back to the retained
  0.5-second source frame and GIF switches back to the separate result.
- A Loop export at 15 fps with 0.5–1.251-second trim includes the partial final delay.
  Explicit GIF Play continues across a source trim seek, keeping GIF selected.
  The source's Movie/GIF choice is not silently changed by trimming.
- Actual Open Cancel retains source, trim, result and independent GIF playback.
  Reopening and selecting the known generated H.264 MP4 reaches the explicit native
  unavailable gate. Format choices remain, trim/result reset for the new source,
  and previously saved files are byte-unchanged. Escape closes cleanly.

Independent Pillow decoding (separate from the app's reader):

- `saved-once.gif`: 320×180, 10 frames, ten 100ms delays, 1,000ms total, no repeat
  property, 10,407 bytes. Original output mode was 0600.
- `saved-loop.gif`: 320×180, 12 frames, delays 70,60,70,70,60,70,70,60,70,70,60,20ms,
  750ms total, repeat 0, 12,284 bytes. Original output mode was 0600.
- Both decoded files contain zero pixels matching the magenta-only final-source
  sentinel (R>240, G<20, B>240), which occurs after these selected trim ranges.
  Exact hashes and decoded metadata are retained in `decoded-once.json` and
  `decoded-loop.json`. No staged sibling remains.

## Controlled inspection retirement

The DEBUG inspection delay holds real request execution for eight seconds; it
supplies no native object and is not a latency measurement.

- Reloaded generated source is visibly Reading movie metadata, then Escape closes
  during the hold at 10:26:10.924 UTC. Later reopen has a clean new source with no
  late old result or popup resurrection.
- Three rapid source choices at 10:28:07.690 UTC leave only the latest source
  pending. Opening the real chooser during that inspection remains usable.
  After inspection settles under the modal chooser, Cancel restores the initial
  paused source frame. This exercises the initial-preview/modal completion race.
- Physical one-active/one-latest bounds are established by controller and callback
  lifetime regressions; screenshots establish actual interaction and visible
  recovery, not native callback behavior or atomic timing.

## Controlled seek/export retirement

- Six rapid Start/End requests at 10:31:26–27 UTC while the first seek was held
  showed only the latest pending End=3.000. It eventually displayed the actual
  final source frame2.950, including the generated magenta sentinel.
- Convert during a held source seek at 10:33:13–14 UTC, repeated Cancel and attempted
  rechoose remained busy until physical retirement. Cancellation published no GIF
  or stage, and a later retry exported successfully.
- This exposed a real caption/state defect: the idle source retained actual frame
  2.950 while labeling requested0.642 from the cancelled newer seek. The screenshot
  was not accepted as completed source review. The repair retains a separate
  completed request and pending intent, keeps the older image under a loading
  caption, and requeues the intended time after terminal export. It also restores
  that intent when an earlier saved GIF exists, without changing review selection.
- The deterministic regression covers cancel, success and prior-GIF retention:
  completed request 3.000/ actual 2.950 → pending 0.642 → completed 0.642/ actual 0.600.
  Both ordinary and app-only nonDEBUG converter suites now pass 44 tests.

## Final repaired-candidate interaction

- Initial paused source and rapid End requests were repeated. Latest End 3.000
  completed at actual 2.950, a legitimate requested/actual distinction.
- The exact discovered race was replayed: request 0.642 → Convert while its seek
  retires → repeated Cancel. The older magenta frame remains only under the
  truthful Seeking 0.642 caption. After retirement, the source completes requested
  0.642 / actual0.600. No cancelled output or staged sibling appears.
- Actual Save Cancel/reopen and retry wrote `saved-final.gif`: 320×180, ten 100 ms
  frames, no repeat metadata, zero post-trim magenta sentinel pixels, 0600 original
  mode. Independent Pillow decoding and SHA256 show byte equality to the earlier
  correct Once export. `decoded-final.json` identifies the repaired binary.
- Movie/GIF switching shows the intended 0.500 source frame. Rechoose during a
  held preview clears prior trim/result, preserves 10 fps/320/Once settings, and
  eventually shows fresh source 0.000. Close during another held preview suppresses
  late presentation; a new process opens clean default state.
- An eight-second pre-encoding export hold establishes a previous successful
  45-frame GIF. Repeated Cancel of a held retry and attempted rechoose preserve
  that result and its exact bytes. A later retry adds exactly one byte-identical
  valid GIF. Close during the next held export adds neither output nor stage;
  fresh reopening has no stale result or later popup. The final window closes
  normally and exclusive desktop ownership is released.
- EXPORT delay is before core export_gif/staging. These are held export-task
  lifecycle checks, not a claim of GUI interruption during actual frame writing.
  Core cancellation/error/panic regressions independently establish staged cleanup.

No source changed during the final GUI run. The final hash set was rechecked after
all interactions. No pre-fix screenshot or saved file is relabeled as coming from
the repaired candidate. The broader pre-fix and final scoped coverage are distinct;
Apple runtime acceptance still requires exact native CI.

## Scope limits

The normal arbitrary-file native gate stays closed, including the MP4 chosen
through the real dialog. Native generated MOV/AVAssetReader/controller execution
is a separate exact Apple CI requirement. The core tests inject terminal decoder
failure at finish; the native tests exercise actual Reading/success, cancellation
and source mutation, without claiming a real AVAssetReader failure was induced
after its last sample. Existing status-classification tests cover Failed/Unknown
separately.

The Xfwm titlebar renders the em dash incorrectly; the app body is readable. Bound
X11 coordinate input did not affect GPUI, so acceptance uses supported full-desktop
pointer/keyboard events. Terminal/GPUI Paste is unsupported in this tool route;
commands and numeric fields use documented key events. These tool limitations
were not treated as application failures or worked around with another automation
technology. Original tool screenshot bytes are retained as JPEGs without editing.
Continuous movie playback/audio, native GUI/IME/accessibility/Spaces, arbitrary
codecs, ICC/HDR, hostile-filesystem immutability and native performance remain open.

## Post-GUI native-test expectation normalization

After the GUI binary was frozen and tested, a one-line macOS-only host-test fix
canonicalized its temporary output directory before comparison. Core publication
already returns canonical paths; macOS can resolve /var through /private/var.
The old exact test file is retained as `native-host-tests-final-gui.rs.txt`, matching
`source-inputs-final-gui.json`. All 19 other GUI input hashes and the ordinary runtime
binary are unchanged. The current manifest records the corrected test hash.
Formatting and whitespace checks pass. The portable converter suite was rerun:
44 passed, 0 failed. Exact native execution remains pending.
