# Recording shutdown: repaired whole-app Linux GUI acceptance

## Result and exact candidate

The repaired immutable executable passed the generated-media GUI paths below.
The pre-repair premature process-exit defect did not reproduce: the last native
window remained visibly **Closing…** until the held physical work retired, then
the application exited successfully without incomplete capture/export stages.

Executable SHA256:
`bdfc718a20ad84be9f59dc52b459ef0f02c275fa0525c2f6b82dd60ad1d28a54`.
This is the production-config app with the explicit nondefault
`recording-fixtures` feature, not a substitute test application. All interaction
used the actual cloud Linux desktop, generated MOV/known-frame input and the real
private-session system file dialog. No native capture or permissions were used.

`validation-gui.json` binds the executable, original screenshots, exact process
exit observations and verified output bytes. The earlier immutable candidate and
its reproduced defect remain separately recorded under
`../recording-host-2026-10-07/QA-report-pre-repair.md`.

## Provenance

The original 179-input build manifest matched completely before GUI testing.
After GUI testing, all recorded repository inputs and other build inputs still
matched. One external verification helper had changed to discover additional
literal include files; its original complete bytes are preserved in
`provenance-initial179.py.txt`, with the exact hash from the build manifest.
The original 179-entry before/after records remain unchanged.

A later audit identified two inputs omitted from that original collector:
the app icon PNG and `project.yml`. Both match the tracked source baseline and
their complete exact byte sequences are embedded in the unchanged executable.
See `later-include-input-audit.json` and the independent complete-byte offsets in
`supplemental-input-byte-proof.json`. This is later evidence, not a retroactive
claim that 181 inputs were checked before and after the build.

All 36 screenshots are original, unedited JPEG bytes. The one full-desktop
capture initially had a `.png` name; only its suffix was corrected to `.jpg`.
`image-filename-map.json` records the original/published names and unchanged
SHA256. No image was cropped, transcoded or redrawn. Screenshots 17 and 23 are
explicitly transient observations; settled acceptance uses their successors.

## Actual process lifecycle tests

Each launch used the same frozen binary in isolated QA configuration/output
directories. The launch supervisor saved the actual child PID, launch timestamp,
wait result, exit timestamp, empty app log, and before/after output-file inventory.
The six original supervisor runs are under `runtime/`. `interaction-times.json`
contains the CUA action/observation timestamps.

The deliberate hold durations below establish overlap and retirement ordering.
They are not native performance measurements or general responsiveness claims.

### Sole window during held Start

Screenshots 01–05: select Hold start, Start, observe Starting, then close the sole
recording window during the three-second worker hold. The same window became
Closing and remained there after another titlebar Close. The process exited 0 at
3,195 ms from the Start input. The initially empty output directory was empty
after exit, and the recording window did not return.

### Sole window during held Finalize

Screenshots 06–10: Start a Hold finish session, verify the supplied session, Stop,
observe Finishing movie, then close during the five-second finalization hold.
Closing remained visible after repeated Close and Escape. The process exited 0
at 5,094 ms from Stop. The private capture present before closing was gone after
exit. No completed publication or orphan stage appeared.

This directly repeats the pre-repair failing workflow, which had returned the
terminal prompt within 823 ms and left its private capture behind.

### Recording first, Home last

Screenshots 11–16: with both Home and Recording present, Stop a held-finalization
session and close Recording first. Home remained visible. Closing Home next
replaced that final window with Closing until physical retirement. Clicking the
former Screen Recording card position while Closing did not create a new window.
The process exited 0 at 5,083 ms from Stop, with an empty output directory.

That stale click verifies the actual replaced UI cannot reopen a tool; it is not
called a direct test of every internal post-quit admission function. Those entry
points have separate focused test coverage.

### Sole review during held GIF export

Screenshots 30–33: after retaining a completed movie and successful previous GIF,
start another export to `closed-held.gif`. The actual options UI showed an active
job at Encoding 0% during its five-second pre-encoding fixture hold. Closing the
sole window retained Closing, including after another titlebar Close. The process
exited 0 at 5,446 ms from the Save input. `closed-held.gif` was absent; no incomplete
stage remained. The completed movie, saved movie copy and prior GIF hashes were
unchanged.

This hold is before encoding. It does not substitute for deterministic staged
publication/CAS race tests or proof of a live AVFoundation callback.

## Movie review, saving and cancellation

Screenshots 17–22 show the completed owned movie reaching a real paused known-frame
preview, system Save Movie As cancellation, reopen and successful save to
`repaired-movie.mov`. The file is 221,877 bytes and byte-identical to the generated
source; ffprobe reports rawvideo, 96×64, 12 frames and 1.2 seconds. The selected
source stayed available throughout.

Screenshots 34–35 exercise a separate interruption: while Save Movie As was still
awaiting a destination, close the parent recording window. The application and
its private portal dialog exited cleanly with code 0. The completed recording
remained, and no late destination copy appeared.

**Physical mid-MOV-Save overlap is not claimed.** This immutable fixture supplies
a 221 KB file and exposes no runtime Save hold; the actual copy completes faster
than an observable overlap can be established here. Dialog cancellation/closure
and successful Save were tested, while worker/publication races remain in the
separate deterministic tests.

## GIF cancellation, retry and retained outputs

Screenshots 22–29:

1. Set 10 fps, Once and exact trim 0.2–0.9 seconds. The UI planned seven frames.
2. Start a held export to `cancelled-held.gif`, then use its Cancel control while
   the actual job was active. The UI first showed Cancelling conversion and then
   explicitly reported cancellation before publication, keeping the source and
   any prior GIF. No canceled destination was created.
3. Retry to `repaired-once.gif`. It succeeded at 1,987 bytes. Independent decoding
   proves seven 96×64 frames, seven 100 ms delays, no loop extension, and no magenta
   sentinel from outside the chosen trim. Its bytes match the earlier validated
   Once result exactly.
4. Switch to Movie and back to GIF; the source preview and completed result remain.
   Explicit Play GIF changed to Pause GIF while playing, then returned to Play GIF
   at the final Once frame. GIF playback is distinct from unimplemented continuous
   movie playback/audio.
5. The subsequent held-export window close preserved those prior files as described
   above. The retained completed `recording.mov` is intentional output, not an
   incomplete `capture.mov` stage.

Full file hashes, decoded timing and final retained-file inventory are in
`validation-gui.json`. The saved movie and GIF are included alongside this report.

## Normal gate and remaining limits

Screenshot 36: with the recording fixture disabled, ordinary Start continued to
report native recording unavailable. The process exited 0 and its output-file
inventory was unchanged. No production gate was opened by this work.

The complete App test executable was locally compile-blocked by SIGKILL. The
production app build, strict Clippy and scoped focused harness evidence are
recorded separately in this directory; this GUI pass is not a full automated-suite
pass. Exact final-commit CI remains a separate gate.

No macOS generated AVAssetWriter execution, real SCK capture, TCC/AppKit/Spaces,
audio, arbitrary movie decoding, target selection, countdown, pause, cursor or
privacy behavior is established here. Native Quit/Cmd-Q, forced process death,
or platform termination may bypass the app-controlled close route and retain
their separately documented best-effort recovery semantics. A native callback
that never drains was not simulated as a claim of guaranteed eventual exit.

All app windows were confirmed closed and the shared cloud desktop was released
after the final gated launch. This pass made no implementation-source edits.
