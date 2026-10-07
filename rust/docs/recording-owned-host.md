# Owned silent-recording workflow (gated)

This slice connects a production-compiled session coordinator, silent AVAssetWriter
owner and retained-movie review/Save/Make GIF workflow. Ordinary Start returns
Unavailable before recording filesystem, thread, native or permission work. Real
ScreenCaptureKit capture, audio, target selection, countdown, pause/resume, cursor
and privacy controls remain unimplemented or closed. No live recording is claimed.

Swift anchors are RecordingCoordinator 69–153/185–275, RecordingEngine
214–232/560–649/845–878, RecordingOutputTransaction 3–56, RecordingFileStore 21–40,
RecordingModels 265–292 and RecordingReviewView 96–290. The completed movie is
retained through GIF cancellation/failure and review close. No Discard operation
is offered. Save refuses existing destinations rather than inheriting Swift's
replacement behavior without a separately implemented confirmation contract.

## Ownership and cancellation

The native writer, input, pixel/sample buffers and blocks remain on one worker.
Finish submission prevents any later cancelWriting call. The worker observes the
finish callback, releases local native references and block, exits its autorelease
pool, and only then waits for copied callback-context destruction. Physical
admission remains held throughout. A 30-second finish deadline is logical only:
callback invocation/destruction may take longer, and cancellation cannot make
that native work disappear. This is not a hard native completion deadline.

Public Stop/Cancel/Drop use bounded atomic state changes and notification. One CAS
chooses Open→Cancelled or Open→PublicationClaimed. A winning cancellation prevents
publication. Once publication has claimed ownership, a later cancellation returns
PublicationClaimed, which means neither successful completion nor a guarantee
that a new file exists. The filesystem operation can still fail and produce a
recovery outcome. Stale UI adoption is suppressed independently. This explicit
publication-start boundary differs from waiting on Swift's filesystem mutex.

A bounded Started/terminal slot remains worker-owned until terminal transfer or
owner drop. UI polling uses a nonblocking lock attempt. Closed consumers cannot
force rejected movie owners to drop on the UI thread, and admission is not released
before that disposal. Completed ordinary and injected fixture MOVs survive the
last adopted owner being dropped. Generated test cleanup is explicit and separate
from closing a review.

Finalization opens the completed file once and retains that descriptor through
publication and the SelectedMovie bridge. Identity is sampled after intentional
link/unlink changes, then revalidated against the named path. Ordinary changes are
detected; this is not hostile-filesystem or change-and-revert atomic identity.
A failed final publication preserves the playable capture with a recovery warning.
Save copies the retained descriptor into a private sibling stage on the destination
volume, rechecks identity and atomically publishes without overwrite. A failed
cross-filesystem Save preserves the source. Its cancellation uses the same explicit
one-use publication claim, without blocking UI on filesystem work.

## Deliberate current limits

The writer currently bounds sessions to 120 seconds, 3,600 frames and 512 MiB
completed files. These are implementation limits, not Swift recording parity:
Swift's 120-second cap belongs to GIF export. Swift recording quality presets have
maximum long edges 1,600/2,560/3,840 and frame rates 24/30/30; preset bitrate and
full configuration behavior are not connected here.

Frame validation accepts even dimensions from 2 through 3,840 per axis, subject
to a 32 MiB RGBA backing/capacity bound. Thus 3,840×2,160 fits, while
3,840×3,840 does not. Native pixel-buffer visible storage is bounded separately
at 64 MiB. PTS use integer 600-timescale ticks, begin at zero, increase strictly,
and have positive durations no larger than one second. Input must be opaque.
Visible buffer bounds exclude AVFoundation's opaque codec allocations and do not
establish an RSS ceiling. File-size sampling during writing is followed by a final
size check; opaque codec buffering can exceed a sampled disk size before failure.

## Sealed generated acceptance

The nondefault recording-fixtures feature exposes finite cases, never caller
paths, RGBA bytes or producer closures. Native generated input passes through the
same normally compiled writer. Only its private finalization proof can confer the
existing generated-reader exception; ordinary reader admission stays closed.

Linux materializes one checked-in raw RGB24 MOV and a known-frame recipe bound to
that exact retained identity. Its 96×64, 10 fps, 12-frame asset is generated offline
with FFmpeg; no runtime FFmpeg dependency or product fallback is introduced.
Injected Start/Stop/review tests demonstrate lifecycle and real Save/GIF output,
not native recording or movie decoding. The asset recipe/hash are beside the file.

Portable tests and Apple-target metadata checks have passed, including atomic
claim races, callback-held admission, dropped/retained owners and exact identity.
Actual generated-media Apple execution and full App CI remain pending. Repaired
immutable Linux whole-app acceptance passes for the generated-media review and
app-controlled held-close paths; no live recording or native parity is claimed.


## App-controlled media retirement

Last-window close and the other Rust-controlled quit path now enter a one-shot
App-global media quit coordinator before GPUI shutdown begins. Registered media
jobs receive cancellation immediately; new jobs/tool windows are refused. The
last existing window is retained with a responsive Closing message until native
Admission and worker-owned completion tickets
physically retire; only then does the app quit. This is necessary because GPUI
X11/Wayland stop their native loop as soon as the final window is removed. Native
titlebar close and application-owned programmatic close share the same veto.
A different tool window closed last also waits for already-retiring media.
There is no timeout that silently converts pending work into
drain. An indefinitely retained native callback can therefore delay this quit.

Native tickets remain owned through callback-context and rejected terminal
payload destruction. Converter requests, MOV Save and image/GIF preparation
reserve a ticket before constructing their owning worker closures. Readiness
leaves a completed payload in a bounded background-owned slot, including during
GIF display delay. Only final current-generation UI adoption transfers it; close,
replacement or rejected delivery disposes it on the worker before ticket release.

The coordinator covers these registered media jobs only. It does not establish
drain for unrelated capture/OCR or utility workers. Platform termination reaches
GPUI's non-vetoable shutdown hook: native menu Quit/Cmd-Q has not been intercepted
and verified here. Normal native platform Quit, process kill, forced quit and OS
shutdown remain best-effort cancellation/recovery boundaries. Existing completed
movies remain retained; this is not a crash-recovery or hard native-drain claim.

The 80-test focused media/shutdown harness, 23 platform recording tests, strict
App/platform Clippy and production-only fixture build now pass. Complete App
test codegen remained SIGKILL-blocked; focused results are not a full App pass.
Repaired immutable Linux process replay now passes held Start, held Finish,
recording-first/Home-last, delayed GIF close, repeated Close and pending Save
dialog retirement. The final window stays visible until worker cleanup; observed
held-close paths exited successfully after their physical holds. See
[shutdown validation](validation/recording-shutdown-2026-10-07/README.md).
Historical candidate `77b9…` failed the held-finalize last-window process test;
its source manifest/evidence are preserved, including pre-edit input snapshots.
