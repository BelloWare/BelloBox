# Candidate 1 actual desktop acceptance

Date: 2026-10-07 UTC. Immutable binary SHA256:
`4a4cf2f4f55ea798292674be4c0cbefc6f57b100024b2e439d7c1da3c2c78e53`.
The 181 compiled inputs are preserved by `source-inputs-candidate-1.json`.
All64 capture originals were verified as unmodified JPEG bytes. Selected meaningful
originals are retained in `gui-screenshots.json`; the full chronology and explicit
local-only transient/redundant statuses are in `all-captures-index.json`. Ranges
below describe the capture sequence, not a claim that every transient is published. `gui-events.json` records close submission times.
Only generated fixture pixels, fake credentials and an owned numeric-loopback
listener were used. Runtime logs for the OCR launches contained no output.

## Verified outcomes and representative originals

- 03–05: actual destination/provider/API/model, dimensions/bytes/background/no-hint
  notices; modal shields keyboard actions; initial Enter cancels with zero requests.
- 06–11: crop 731 × 357 plus opaque mask; local decorative rectangle is excluded
  from the retained PNG preview. Reopening confirmation retained identical bytes.
- 12–13: one explicit Upload, extra Enter cannot dispatch twice; literal Text and
  Markdown. No rendered HTML, link navigation or image loading.
- 16–19: explicit Save dialog cancellation retains valid result; reopened explicit
  Markdown Save produces 43 literal bytes, mode 0600. `saved-popup-result.json`
  records the file hash and content proof. 21 shows an explicit Copy Markdown
  pasted into an unsaved local label (the one-line field displays its last line).
  The GUI paste does not prove a complete clipboard hash; exact string equality
  is covered by the controller test. 20 is the empty field before focus and 22
  does not reveal any additional prefix.
- 25, 28, 30: recoverable HTTP 429, bounded oversize refusal and refused 307;
  redirect trap remains zero. Screenshot 37 proves successful retry after these failures.
- 32 at 13:10:55.001: real delayed request 5 still reading; 33 at 13:10:57.078:
  Crop plus Undo occurred before its eight-second reply, restoring image geometry
  but clearing reader. 34 after drain has no revived text or copy payload.
- 39 at 13:17:43.949 and 40 at 13:17:44.300: popup close/discard while request 7 is
  reading; close submitted at 13:17:44.532 and no popup resurrected. This popup launch
  subsequently exited; it is not the same-process coordinator-drain proof.
- 41–48: real Area drag/resize; exact 521 × 351 px, 4,931-byte preview; default Enter cancels
  at zero requests without Copy & Finish; explicit Upload yields one result;
  scrolling exposes Save in the constrained pane.
- 49 at 13:30:51.966: actual Area request 2 still reading; 50 at 13:30:52.319: Escape
  removed editor while chooser remained Capturing. 51 after drain: the same
  process's chooser is idle and no old editor/result was published. Deterministic
  actual-coordinator tests additionally prove blocked reopen during held ownership.
- 52–58: fixed generated Window source, no Area resize handles. Crop preserves
  fixed frame and prepares a 521 × 291 px, 4,108-byte PNG. Initial Enter cancels at zero;
  one explicit approval produces a literal result. This is not native Window
  catalog/capture/materialization/refresh acceptance.
- 59–64, already published in `../preview-disposal-2026-10-07/`: same binary, separate converter fixture launch. Start 0.642 remains fully
  visible at end caret, End 3 ordinary. Actual generated export review, retained
  Movie/GIF switch, generated-source rechoose clears old review, close/reopen
  clears stale state. GUI does not measure GPU atlas memory or native decode.

## Findings requiring a later candidate

1. OCR Save uses the screenshot-export busy caption (14–18).
2. Provider warnings collapse in the short inline reader (47–48,57–58).
3. Full-frame Window status overlays toolbar (54,57–58).

Candidate 1 verifies no repair for these findings. Pre-repair source copies for
the four affected files are in `pre-fix-source/`. Final affected checks require a
new immutable binary and separate source/screenshot attribution.

## Evidence limits

08 is a transient pending canvas render, not a stable rendered-output proof.
15 records the popup's lock behind the chooser, not the chooser itself; 17 is the
actual GTK dialog. Other preparation screenshots intentionally show intermediate
work. The finite UI response controls exercise the real parser/transport; canned
provider output is not a live OCR quality or paid-provider availability claim.
Ordinary upload, credential, capture and native Window/Area gates remain closed.
