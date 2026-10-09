# Independent palette QR source and negative-control review

Reviewed commit: 0d203d98ad2753c1c5b74358289c6a3725e7eae3
Tree: c58f53395cb34fe063661728995c05be9902f75c
Baseline: 5934b9897854a4102ef71d6463d4b4f7a3005158
Date: 2026-10-09 UTC

## Result

No remaining blocking source findings in the reviewed QR slice. This is a source/test-platform review, not independent native or scanner acceptance. Exact reviewed SHA-256 values are in source-hashes.json. Later integration commits need scope/hash reconciliation and applicable checks.

Read complete immutable f1c21ae4f22b14dc10c5a57651499b05cc37ec92 migration handoff and exact Swift main e43b1c4595c42383e087fe70f38c28d07c31dda0 QR session, utility preview, popup, generator, launcher handoff and keyboard ownership source. Swift subtree remains equal to that main commit.

## Findings resolved before freeze

1. Held Enter/Space after Clear moved focus into the editor and could change the just-cleared draft. Matching activation key-down repeats are now consumed until release. A real dispatched regression distinguishes suppressed repeat from a subsequent legitimate editor newline.
2. Drafts above the 500,000-byte handoff limit previously received a misleading 64 KB notice suggesting Open, while Open was disabled. Global validation now precedes preview validation and explains Clear/Paste recovery without truncation.
3. Tests originally checked empty handoff only through a helper and stale generations only with different text. Added actual launch-to-popup edited/empty verification and same-text ABA old-success/old-error identity checks.
4. Confirmed and documented that GPUI 0.2.2 macOS Save uses app-modal beginWithCompletionHandler rather than Swift palette-attached sheet behavior. Linux requests a modal SaveFile portal with the active window identifier. No native sheet equivalence is claimed.

## Source checks

- One retained transient draft/editor, independent original selection, exact edited-empty String handoff, limits before payload clone/work, current-only raster retention.
- Background encoding; owner/generation/activity/exact-text checks fence success and error. Deactivation cancels preview jobs; replacement/close retires entity and subscriptions.
- Clipboard reads/writes only through explicit actions/editor commands. Accepted current image gates Copy/Save. Save retains click-time PNG bytes, independent status token and quit blocker; cancellation writes nothing, private create_new helper rejects existing destinations.
- QR controls have separate focus/activation ownership. IME wins; multiline Enter/arrows stay with editor; Tab traverses enabled controls, Escape returns to search with held-key guard, Ctrl/Cmd+K restores search.
- Pending Save blocks launch, navigation and ordinary close; pointer replacement handlers are guarded. Clock's physical-retirement, handoff, close and quit logic remains present. Existing Clock lifecycle tests passed in the independent baseline.
- Module geometry uses actual encoder, white quiet zone, bounded compact/enlarged rasters and unchanged export bytes. No decoding/scanner success inferred from geometry.
- Diff contains only the eight listed QR/launcher/core/document paths. desktop.rs changes only the approved generate call after dead terminal-preview API removal. Core terminal CLI export unchanged. No movie, capture/provider/vault/native gate, dependency, workflow or Swift edit.

## Independently executed checks

An isolated archive of the frozen source was used; product checkout was not mutated. Required compile-time Swift icon/project.yml assets were restored from the same commit after initial archive setup errors. Builds used the existing official toolchain/sysroot and serialized shared Cargo lane. Synthetic settings were isolated.

Baseline command:
    cargo test --locked -p bellobox-app --bin bellobox launcher_ -- --test-threads=1
Result: 40 passed, 0 failed, 1 existing ignored; baseline.log.

Negative control 1: remove only !self.jobs.accepts(token) from publication condition, leaving activity/retired/exact-text checks intact. Run same_text_aba_rejects_old_success_and_error_then_recovers_at_byte_limit. Result: expected exit 101; obsolete first-A error erased current accepted image and assertion failed. negative-generation.log.

Negative control 2: restore frozen source, then remove only matching consume_release key-down guard. Run dispatched_clear_held_return_and_release_preserve_empty_then_allow_newline. Result: expected exit 101; actual draft was newline instead of empty after held Return. negative-held-return.log.

Restored isolated source and verified its SHA-256 equals the frozen source afterward. No mutated source was committed or published; no screenshot was produced or published by this review. The final shared test executable is a negative-control build, so subsequent testing must rebuild from intended source. The sealed ordinary application binary was not overwritten by these test-only builds.

## Evidence limits

Clipboard assertions inspect test-platform PNG bytes, and Save tests inspect actual temporary file bytes through simulated test-platform dialog selections. They do not prove actual Linux desktop clipboard, portal dialog, or macOS clipboard/sheet behavior. Real desktop acceptance belongs to its separately hash-bound report. Scanner decoding, native IME, Retina output, macOS nonactivation and general performance remain unverified here. Aggregate/feature/minimal checks reported by the implementation owner are separate from this independently executed launcher baseline.
