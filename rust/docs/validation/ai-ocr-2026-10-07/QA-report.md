# Image OCR controller: final local acceptance

## Result and provenance

The image-specific ScreenshotEditor consent/controller slice is implemented and
locally verified with ordinary production upload admission closed. Candidate 2
fixes all three UI findings from the first desktop pass. No real provider,
credential, user image, capture permission or paid endpoint was used.

- Final immutable binary SHA256:
  `136076adc9c0d3f21ca967102efc2f20e49ef5cfab02b66c12c1f0a0b8346af5`
- Size: 142,435,240 bytes; mode 0500; local path:
  `/workspace/scratch/8b6fda578834/build-environment/ocr-qa/bellobox-immutable-candidate2`
- Build: `cargo build --locked -j1 -p bellobox-app --bin bellobox`, default dev
  profile, established shared warm target. Launch recipe:
  [launch-recipe-candidate2.sh](launch-recipe-candidate2.sh).
- [Candidate 2 source manifest](source-inputs-candidate-2.json) covers 181 source,
  Cargo, project and artwork inputs. All matched before and after actual GUI use.
  Exactly four files changed from the first candidate, listed with both hashes
  in [candidate2-repair.json](candidate2-repair.json).
- [Candidate 1 report](candidate-1-report.md), its independent source manifest,
  selected originals from01–64 and four historical `.rs.txt` snapshots remain attributed to
  binary `4a4cf2f4…c78e53`. They are not evidence of the subsequent UI fixes.
- [Screenshot manifest](gui-screenshots.json) records the original CUA JPEG bytes,
  capture UTC, byte length, SHA256 and exact candidate binary for53 retained
  representative originals. The [complete capture index](all-captures-index.json)
  identifies all83 verified captures: six converter originals are already in
  `../preview-disposal-2026-10-07/`, and24 transient/redundant frames remain local
  only and are not additional acceptance evidence. Originals65–83 belong to
  candidate2. The unretained70 was an unsuccessful resize attempt; retained71–73
  and82–83 are the actual640×440 view. Images were not edited.

## Checks on the final source

[Machine-readable local checks](candidate2-local-checks.json) preserve commands,
log digests and the exact no-debug-assertions test artifact digest.

- Full ordinary app suite: **287 passed**.
- Exact app-only debug-assertions-disabled test executable: **279 passed**.
  Only app rustc receives that override; this is not an optimized release or
  whole-workspace release test.
- Focused controller tests: **18 passed**, including real modal input routing,
  Save dialog Cancel/reopen/private literal file publication, distinct PNG
  captions, lifecycle clearing, warning viewport bounds and chrome geometry.
- Actual inline fixture regressions: **12 passed** within the full suite, including
  canceled-between-check/count ownership, missing/successor transaction rejection,
  retained entities, actual prepared-byte/result/disposal holds, shutdown and the
  held Save worker with a dropped result receiver.
- Image transport regressions: **11 passed** within the full suite. Numeric IPv4
  and IPv6, all provider formats, redirect/proxy traps, size bounds, independent
  read and absolute deadlines, cancellation and revocation use actual sockets.
- Combined app/platform all-target strict Clippy with `movie-fixtures`, minimal
  app feature check and normal app build passed. Unchanged core screenshot source
  previously passed 187 tests and core test-target strict Clippy.

The new warning test initially attempted an unsupported detached editor-element
render. That test-authoring panic was corrected to render the actual editor
entity before the successful focused and aggregate runs above. It was not a
passing check and required no additional production repair.

## Candidate 2 actual desktop outcomes

Actual CUA ran from 14:21 to 14:28 UTC on 2026-10-07 with exclusive desktop ownership.

1. **Explicit Save and correct busy ownership** (65–69). The generated image was
   approved once through the real modal and actual owned numeric-loopback HTTP.
   Literal Markdown remains literal. OCR Save shows “Saving the OCR result…”.
   Cancel clears the lock and retains the current result. Reopened explicit Save
   writes `repaired-result.md`: 43 bytes, mode 0600, exactly
   `# Synthetic OCR result\nGenerated image only`, SHA256
   `ca777a2ecedec66f65ffe1bdaad2ca4d55d04c68fbc591deedc9bbd594341e78`.
   A subsequent PNG Save with the OCR reader still open correctly displays
   “Exporting the current screenshot…”. Its chooser was canceled.
2. **Minimum size and modal containment** (71–73, 82–83). Actual native window
   inventory confirms 640×440. Ctrl+Shift+C does not reach Copy & Finish;
   Tab/Shift-Tab/default Enter stay in the modal and cancel without another
   request. The card scrolls to reveal both complete buttons. A fresh popup
   starts with zero requests and clicking its visible Cancel leaves zero.
3. **Constrained inline warnings and toolbar** (74–78). In the actual 960×540
   fixed-generated Window host, default Enter cancels at zero. Status now occupies
   a bounded strip above the toolbar and to the left of the reader; every toolbar
   control remains visible. After one real request the provider warning is visible.
   Scrolling the reader exposes Copy/Save while retaining the warning. The large
   warning's separate 60px viewport bound is covered by the rendered test, not by
   pretending the short canned warning is a long-output GUI test. Inline OCR Save
   also uses the OCR caption, and chooser cancellation restores the usable reader.
4. **Actual pending HTTP close and physical drain** (79–81). Request 2 is visibly
   reading at 14:27:01.983 during the fixture's eight-second delayed response.
   Escape removes the Window editor by 14:27:02.304. The same process's chooser
   remains “Capturing…” while physical work owns the coordinator; after drain it
   is idle, with no late editor or result. This complements the deterministic
   blocked-reopen/count tests and the actual Area close/drain from candidate 1.

The first broad candidate additionally demonstrated crop/mask sanitization,
transparent RGB flattening disclosure, decorative exclusion, identical reopened
preview bytes, explicit literal Copy/Save, HTTP 429/oversize/307 recovery, no
redirect trap connection, and late-result suppression after Crop plus Undo.
Those byte/privacy paths did not change in the four-file UI repair.

The same first candidate also checked the copied movie trim/disposal UI: Start
0.642 fully visible at the end caret, ordinary End 3, generated export review,
retained Movie/GIF switching, generated-source rechoose and close/reopen. These
are scoped converter UI checks, not native decode or GPU-memory measurement.

## Limits and cleanup

All scoped popup/Area/Window runtime log files were empty; no provider text,
image/Base64 or keys were logged. No OCR staging files remained. All test windows
and choosers closed normally. The desktop was explicitly returned to Skills at
14:28:20 UTC. One minimum-size edge drag briefly resized the background Chromium
frame; its original 1188×848 geometry was restored immediately without navigation,
then focused window-manager resize was used for the actual popup evidence.

Ordinary upload, credential and native capture gates remain false. The fixed
Window fixture starts from a generated frame and makes no native Window catalog,
physical extraction, capture or refresh claim. Real credential disconnect signaling
is still absent from the existing environment-key path. Local hints/hybrid regions
remain visibly unavailable. This is not full screenshot parity or paid-provider
OCR-quality validation. [The scoped LOC ledger](loc-review.json) separates runtime
code from tests and positive fixture-support ranges; no parity percentage is used.

Exact published OCR Linux/macOS CI is owned by the parent publication task and
was still pending when this local report was written. Local tests, CUA and earlier
native movie CI must not be substituted for that exact published result.

## Post-GUI fixture portability correction

A final cross-platform audit found that Darwin accepted sockets inherit a
nonblocking listener's flag, unlike Linux. The generated loopback fixture now
explicitly restores blocking mode before its existing50ms read/500ms write
timeouts, including the redirect trap. A deterministic Unix test first forces
O_NONBLOCK and then verifies its removal and exact timeouts. Production image
transport, consent, UI and payload code are unchanged.

This support-only change is compiled in DEBUG fixtures, so it is not mislabeled
as cfg(test)-only or as an unchanged complete GUI source manifest. Linux accept
already produced blocking sockets. Candidate2's exact181-input manifest and both
affected source snapshots are preserved in `post-gui-fixture-normalization/`.
`source-inputs-publication.json` identifies the two later hashes separately.
All12 focused transport tests, strict original app all-target Clippy and workspace
format passed after this correction. The first test expectation used an exact
timeout getter; Linux canonicalized requested50ms to52ms. The final regression
compares kernel-canonical values configured before normalization, with no invented
tolerance. Exact published macOS execution remains pending.
