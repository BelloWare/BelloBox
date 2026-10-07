# Recording host: immutable pre-repair GUI evidence

## Result

**This candidate is not accepted for complete shutdown cleanup.** Actual Linux
process exit during held finalization leaves a private capture file. This is a
reproduced lifecycle defect, not an inferred native-framework limitation. The
source fixes being prepared after this pass require a rebuilt immutable binary
and their own verification; these screenshots cannot be transferred to it.

The tested executable SHA256 is
`77b9b088e5a748d33735742cac8a0e46e59a13de41da666dca75ab52d7440549`.
All 174 source/asset/manifest entries in `source-inputs.json` matched the staging
checkout before the recovery pass. `validation-pre-repair.json` records the exact
hashes, original screenshots, file outputs, timing and unexecuted scope. Later
source patches must retain that manifest and corresponding pre-edit snapshots.

## Scope and inherited evidence

The nondefault recording fixture supplies an existing generated raw-RGB MOV and
sealed known frames. It exercises the real GPUI host, owned retained-file bridge,
system Save dialog and portable GIF encoder. It does **not** perform screen
capture, native movie decoding, audio, permission grants or paid provider calls.
Normal native recording admission remains closed.

Screenshots 01–13 were recovered unchanged from the previous run. Their visible
states were reviewed; this continuation did not repeat every earlier gesture:

- 01: ordinary Start reports native recording unavailable, with no capture claim.
- 02–05: supplied Start/Stop, owned review at 760×520, system MOV Save and success.
- 06: review Escape states.
- 07–10: 0.2–0.9-second trim, Once export, existing-destination refusal preserving
  the prior result, Loop export and usable 640×440 review.
- 11–13 and `held-start-times.json`: held Start, close/reopen receiving physical
  Busy, then successful retry/review. This is process-alive window retirement
  evidence; it does not establish process-exit cleanup.

The original file outputs were independently re-read during this continuation:

- `gui-saved.mov` and `gui-recovery.mov` are byte-identical generated MOVs,
  221,877 bytes; ffprobe reports 96×64 rawvideo, 12 frames and 1.2 seconds.
- `gui-once.gif` is 1,987 bytes; `gui-loop.gif` is 2,006 bytes. Both decode to
  seven 96×64 frames, each 100 ms, for the requested 0.7-second trim. Once has no
  loop extension; Loop has repeat value 0. Neither contains the magenta terminal
  sentinel from outside the trimmed interval. Full hashes are in the manifest.

## Fresh interaction after recovery

The existing private-portal launch recipe was reused on the cloud desktop with
the verified executable. No implementation source was edited by this GUI pass.

1. Fail finish → Start → Stop returned an explicit writer failure and re-enabled
   Start. Screenshot 14 records the failure.
2. In that same host, Recovery → Start → Stop successfully reached review. The
   warning explicitly says final publication failed but the completed movie was
   preserved. The existing conflicting destination stayed intact (25-byte fixture
   sentinel); the separate retained `capture.mov` stayed readable. Screenshot 15.
3. System Save Movie As wrote `gui-recovery.mov` and reported success. Its complete
   hash equals both the retained fixture and the earlier saved movie. Screenshot
   16. Closing review kept both files.
4. A supplied session left idle past its existing 120-second limit failed with a
   completion-limit message and re-enabled Start. Screenshot 17. Retry succeeded.
   This is observed bound/recovery behavior, not source recording-duration parity.
5. Home was closed while the idle recording window was minimized, then recording
   was restored. Inventory verified it was the application's sole remaining
   window before the held-finalization test.

## Actual process-exit failure

The sole recording window used Hold finish → Start → Stop. Screenshot 18 shows
“Finishing movie…” with the Hold finish case selected. The fixed fixture hold is
five seconds. The window was then closed with its window-manager Close button.

The launch terminal prompt was visible 823 ms after Stop and 313 ms after the
close input. The recording window and application dock item were gone. Screenshot
19 and `last-window-exit-times.json` preserve the original observations; these
timestamps establish that exit preceded the hold, not a general latency benchmark.

A new private `capture.mov` remained after exit, at the exact relative path and
hash in `validation-pre-repair.json`. It is 221,877 bytes and equals the generated
fixture. It was never published/adopted as a completed recording. The previously
saved MOV and recovery copy remained unchanged. A private file surviving abrupt
shutdown is not a successful ordinary completion or cleanup outcome.

This confirms the review finding: the current immediate-ready `on_app_quit`
observer signals cancellation but does not keep the process alive for physical
worker retirement. Tests that continue a TestAppContext after quit prove logical
teardown only. Separately, review found the inherited GIF cancellation path could
wait on its publication filesystem mutex; that concern is not established or
cleared by these MOV screenshots.

## Required next acceptance

- Rebuild after the App-owned shutdown registry and atomic GIF publication changes.
- Verify last-window close during held Start and held Finalize retains process
  liveness until actual drain, does not leave private stages, and does not revive
  closed windows. Test a new/reopened window during pending quit intent.
- Verify successful completed-MOV close still preserves the movie.
- Execute bounded mid-Save/GIF cancellation and close, including the publication
  claim race and truthful post-claim result. Physical drain must include prepared
  images and file owners, not just cancellation flags or the UI task lifetime.
- Run the final source tests/lints and exact-commit Linux/macOS CI. Generated native
  AVAssetWriter execution remains unexecuted by this Linux GUI pass.

No real native capture, TCC/AppKit/Spaces behavior, continuous movie playback,
audio, source target selection, countdown, pause, cursor or privacy workflow is
accepted by this document. Delayed-GIF and mid-MOV-Save interaction remain pending;
the pass stopped after the proven process-exit defect and released the desktop.
