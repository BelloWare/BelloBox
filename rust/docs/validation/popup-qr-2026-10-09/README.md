# Full-popup QR validation, 2026-10-09

Status: final exact-source validation assembled for publication; published-branch CI must be checked on the resulting documentation commit.

## Exact source and scope

Final candidate80e6d41a68fc0e2bf186272fb788a301f4f9e744, tree3a4d1f63a5857a0cf60d37001991f4e59fa61df8, parent4589e0280844df4a070533c7babb9d01cf430905. The preceding QR candidate4589 has tree294c99a7847757d2c41217834b8dc87eab9e4765 and parent15197a71d81c620bf60da18f0d066505e4c0dc99. Seven Rust postimages plus palette-qr.md are bound by source-remote-verification.json and the source manifests. Ordinary package-clean binary SHA256fe662658648c356888745ab72aaf0b8c9df85252f7b559680b7bc803ace7c174; portable/SEAL-v3.json binds it to the source.

This bounded workflow adds truthful unsupported-backend Copy Image behavior to the existing full QR popup, a persistent Save alternative, window-local held Enter/Space refusal, non-editing recovery with no keyboard-release replay, and a constrained preview/status layout. Palette and popup share QR-only capability/key helpers. The source-supported native macOS image path is preserved. Generated/export PNG data is unchanged. No capture/tool/vault gate or dependency was broadened.

Swift specification: BelloBox main e43b1c4595c42383e087fe70f38c28d07c31dda0, QRCodePopupView.swift/QRCodeGenerator.swift and the previously documented palette sources. The complete immutable migration handoff f1c21ae4f22b14dc10c5a57651499b05cc37ec92 was read. Source correspondence is not whole-product parity.

## Portable final checks

The final ordinary-source checks used the pinned Rust toolchain and existing official build prerequisites:

- Full bellobox-app tests:445 passed,2 preexisting ignored (portable/app-full-v3-final.log).
- Focused popup tests:11 passed (portable/qr-intrinsic-fit-r3.log).
- Minimal QR tests:46 passed (portable/qr-minimal-v3.log).
- Minimal launcher tests:52 passed,1 preexisting ignored (portable/launcher-minimal-v3.log).
- Strict default and minimal all-target App Clippy passed (portable/clippy-v3.log and clippy-minimal-v3.log).
- Explicit App package clean and ordinary default build passed. Source hashes were unchanged after checking/building.

GPUI TestPlatform checks cover guarded direct/pointer Copy, text clipboard sentinels, window-local dispatched input, held repeats/two-key guards, missed-release recovery preserving draft/selection, no replay, explicit fresh-pointer Save, immutable PNG bytes and layout. Decoded-image tests inspect the actual image child, repeated normal-size frames, and fresh tiny hosts. These are synthetic tests rather than actual desktop interaction.

## Preserved failures and superseded evidence

The original test-only missing Focusable import compile failure and missing-selector pointer failures remain in portable/qr-focused.log and qr-focused-r2.log. Later passes are separate files. Minimum-height footer and preview diagnostics remain in qr-layout-diagnostic.log, qr-preview-diagnostic.log and qr-preview-minimum-diagnostic.log.

The v2 ordinary GUI exposed actual image overflow despite passing wrapper-bounds assertions. Candidate272e2b1d404be836b836f260dca9b833535f98b3/treec65addce8d8fe06affb7d0774239c193bc279171 was never published on rust. gui/v2 preserves the text failure receipt and independently measured window sizes. portable/independent-review-v2.txt preserves the report and subsequent qualification; only its internal reviewer identifier is omitted. Its earlier source-only Contain conclusion is superseded.

The v3 implementation explicitly sizes the decoded image from final allocated square bounds. The first full v3 test attempt had443 passes,1 failure and2 ignored; its raw log remains. The failed tiny-host debug-selector assertion read stale GPUI debug bounds after resizing. The corrected oracle uses fresh tiny hosts and retains repeated decoded-image normal-size checks. See portable/INTRINSIC-FIT-CORRECTION.md and qr-tiny-diagnostic.log. No raw failure was reconstructed or rewritten into a pass.

## Actual Linux GUI on v3

The exact ordinary v3 binary passed bounded actual X11 interaction: decoded preview fits at400×480,520×620 and520×700; external Mousepad text clipboard survived disabled Copy; explicit Paste/Clear and2,001→2,000-byte validation worked; real Save/Cancel/no-overwrite and long status scrolling worked. Immediate Return/Space+mouse refusal and focused non-editing recovery caused no chooser replay; a fresh mouse action opened one chooser. Four popup and one palette-smoke SaveFile requests match the observed five choosers. Apps and isolated portal sessions closed normally. See gui/v3/acceptance.md and the original portal logs.

Both external clipboard text files were saved from actual GUI source/paste documents and independently hash-verified, not synthesized as expected outputs. PNG receipts preserve actual exported-byte hashes/dimensions without publishing image files. No independently sustained long hold, missed-release-outside-window setup, simultaneous independent two-key ordering, IME/native macOS, Wayland or scanner acceptance is claimed. Actual v2 overflow remains separately visible in gui/v2.

## Pre-repair native results:4589

native/pre-repair-4589 contains the verified receipt and raw logs from issue comments6074717477 and6074718876. Both transport halves decoded with the stated compressed/JSON hashes; all95 embedded file hashes verified. Source patches, the standalone Rust probe and orchestration scripts are omitted from publication, with exclusions disclosed in the receipt. No duplicated Rust is added.

Three ordinary native builds passed. The original matrix remains296 passed,5 failed,3 ignored, with79 distinct selected names (78 runnable and one existing ignored helper). The movie-fixtures launcher invocation had47 passes,5 failures,1 ignored: first an immediate Darwin OS35 WouldBlock in a Clock copilot loopback read helper, then four shared-lock poison cascades. All QR groups passed. A reconstructed byte-identical test binary passed52/0/1 in each of two bounded fresh/learned diagnostic replays. These do not replace the first failure or make this a green preflight.

Both affected fixture helpers and the worker matched published15197 byte-for-byte. A separate numeric-loopback probe reproduced immediate nonblocking accepted-socket WouldBlock3/3 times; explicit blocking mode then received a delayed byte. This supports a pre-existing fixture issue without proving a production regression. At this pre-repair checkpoint the narrow two-file repair remained pending; its completed successor and separate evidence are recorded below.

Native strict Clippy fails the same baseline diagnostics: pre-existing core needless-return and seven App-scoped nonminimal-bool diagnostics, matching anchors/multiplicities across all three configurations. No native strict pass or suppression is claimed. Ordinary build/test/format logs contain no compiler warnings. Native host/toolchain and exact compile/binary provenance remain in the receipt; these native compiled TestPlatform tests do not establish actual AppKit/clipboard/desktop acceptance.

## Repaired native fixture matrix

native/repaired-4589 preserves the separate exact4589-plus-patch matrix. The5,118-byte patch SHA2566420ac93d90893255a51cbedd9485d04ec3c47043488c74159264c0b8bc58ae4 touches only two test helpers/modules: explicitly normalize accepted sockets to blocking mode and add delayed-header/delayed-body regressions while preserving the existing8-second listener deadline and5-second read timeout. Production networking is unchanged.

Three ordinary native builds passed; repaired matrix307 passed,0 failed,3 existing ignored. Each App launcher variant now has54 passes plus one ignored. There are81 distinct selected names,80 runnable. The exact negative control, same new regressions without normalization, failed both tests with the original Darwin WouldBlock:0 passed,2 failed. Its binary/inventory/raw failure logs are retained separately. No assertion or timeout was relaxed.

All72 repaired-bundle file hashes and compressed/decoded transport hashes verified. Native strict remains blocked by the same six baseline comparisons; the earlier exact15197 baseline was carried forward, not rerun for this test-only patch. This is a positive native build/synthetic runtime result, not native strict or actual desktop acceptance.

## Immutable successor and production equivalence

Final candidate80e6d41a68fc0e2bf186272fb788a301f4f9e744 contains exactly the two repaired test postimages on4589, under existing cfg(test) parent-module declarations. Both postimage hashes match the repaired native bundle. Every other Git path and all eight original QR source/doc postimages are unchanged. successor/verified.json records the fetched immutable commit/tree/path verification.

Local successor validation passed formatting, both delayed-reader regressions,54 default launcher tests plus one preexisting ignored,54 minimal launcher tests plus one preexisting ignored, and an explicit package-clean ordinary build. Its binary SHA256fe662658648c356888745ab72aaf0b8c9df85252f7b559680b7bc803ace7c174 is byte-for-byte identical to the binary actually exercised by the v3 GUI. All ten source/doc postimages were unchanged after these checks. This binds the actual ordinary GUI behavior to the final successor while retaining the original4589 GUI receipt's attribution.

The repaired native matrix was run on exact4589 plus the same two postimages; the immutable successor binding establishes exact source equivalence. Native ordinary binary hashes differ between native pre-repair and repaired builds and are not represented as byte-equal. The clean-byte equality proof is specifically the local Linux ordinary build and v3 GUI binary. Strict Clippy was not rerun locally for these two test-only additions; earlier portable strict passes remain attributed to v3, and native strict baseline failures remain open.

## Gates and accounting

Actual macOS NSPasteboard/AppKit/focus/sheets/IME, Retina/AX/TCC/Keychain, scanner acceptance, performance and signed release acceptance remain separate. Linux startup, synthetic dialogs and native builds do not close those gates. No screenshot/capture inventory, executable or duplicate Rust source is included here.

Final source counts:57,905 production/41,607 support/105 benchmark,99,617 active product lines. The successor adds76 support lines and zero production relative to4589. Shared workbench is counted once in Box;115 unchanged standalone evidence Rust lines are excluded. loc/ contains hash-bound reproduction and six tamper-control receipts. No completion date or speedup is inferred.
