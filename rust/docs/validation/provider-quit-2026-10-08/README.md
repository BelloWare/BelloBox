# Provider setup: physical app-Quit blocker

Base: `dec241107f359146d0e63fb051dfafe94e1425e1`.

## Reproduced integration defect

The provider setup controller retained its own single-worker admission lease
across cancellation and Settings closure, but that lease did not participate in
the app-owned Quit policy. An app-controlled Quit could therefore latch while
Load or Test still had a physically held HTTP request.

Before changing production behavior, two deterministic GPUI regressions on this
exact base each started a numeric-loopback request, waited for the server to
receive it, then dispatched app Quit without releasing the response. Both failed
at the assertion that Quit must refuse while setup transport is physically held.
`red.log` and `red-regression.patch` preserve that proof (apply the latter
with `git apply --unidiff-zero` against the exact base). The red source used action
dispatch; the final green tests additionally exercise the actual platform
Ctrl-Q/Cmd-Q binding from the active Settings window.

## Narrow repair

`Setup::start` now requires UI App context. After pure request validation and
private single-worker admission, it acquires the existing `shutdown::QuitBlocker`
on the UI thread. The owned blocker moves into the actual worker closure and
remains alive until the HTTP operation returns. Cancel, editor changes, receiver
destruction and Settings closure cannot release it. It retires together with the
private transport lease before result publication; stale UI results still fail
the existing identity/cancellation checks.

Failed request validation does not acquire a blocker. Failed thread creation drops
the owned closure and both admission guards. A test-only failed-spawn injector
exercises that exact closure-disposal path without exhausting OS resources or
changing production permission/settings behavior.

The existing nonmedia policy refuses the current Quit attempt and shows a notice.
It does not preserve approval or automatically resume Quit when work ends. A new
deliberate Quit succeeds after physical retirement. The transport's existing
bounded cooperative-cancellation/timeout behavior is unchanged.

## Acceptance scope

Four focused GPUI tests cover:

- Load and Test held independently: actual Ctrl-Q/Cmd-Q refuses while the request
  is active; closing Settings while an admitted Home remains open still refuses
  a repeated Quit until the server releases the request.
- Retained, closed Settings entities cannot publish late models or a successful
  Test; polling completion twice cannot republish it.
- No refused Quit resumes itself. A later deliberate Quit succeeds exactly once.
- Invalid Test configuration sends nothing and leaves no Quit blocker.
- Injected thread-spawn failure sends nothing, releases both guards, allows
  admission to be retried, and leaves no Quit blocker.

Fixtures use only numeric loopback, fixed synthetic keys, isolated configuration
files and explicit provider/key environment removal. No real provider request,
user credential, native capture, permission, signing or release operation occurs.
This is synthetic GPUI test-platform evidence, not manual GUI acceptance.

No shutdown/media registry code was changed. Native/Dock/forced termination and
the separately documented last-window nonmedia close/drain gap remain outside
this repair; closing Settings with Home alive is the tested nonmedia scenario.

## Late-admission regression

A second review found that registering a Quit blocker cannot undo an already
latched shutdown. With the first repair in place, two additional tests retained
the Settings entity while the existing synthetic media-drain ticket held the
application in closing state. Both Load and Test still reached the listening
loopback trap. `late-red.log` preserves those two expected failures.

The controller now rejects `shutdown::requested(cx)` before validating a new
request, acquiring either lease or creating a worker. Both retained-entity cases
assert zero HTTP connections and a visible closing-state error. The existing
media-drain ticket is a test fixture only; its production policy is unchanged.

## First repair validation (superseded by final r2 below)

- Pre-fix exact-base reproducer: both held-request Quit tests failed as expected.
- Final focused shortcut/failure suite: 4 passed.
- Settings suite before the final shortcut-route strengthening: 17 passed.
- Final full recording-fixture App suite: 382 passed, zero failed; one isolated
  subprocess-only proxy fixture is ignored in the main harness and is explicitly
  executed by its passing parent test.
- Strict recording-fixture App/test and default production App Clippy passed.
- Formatting, diff checks and independent four-path source review passed.
- No core/platform source changed; their separate unit suites were not rerun for
  this narrow App ownership repair.

`receipt.json` binds the exact base, four source postimages and validation logs.
No test success is substituted for manual pointer or native runtime acceptance.

## Final r2 validation

- Final Settings suite: 19 passed, including the two shutdown-latch listener traps.
- Final full recording-fixture App suite: 384 passed, zero failed; the one
  subprocess-only ignored marker remains explicitly exercised by its parent.
- Strict recording-fixture App/test and default production App Clippy passed.
- Formatting, diff checks and independent four-path r2 source review passed.
- The initial held-request red proof and first repair results remain recorded
  separately from the later admission red proof and final r2 checks.

No core/platform source or native/last-window shutdown policy changed.
