# Recording output-settings policy (B3)

This is pure backend policy, not a completed recording workflow. It ports
`RecordingOutputSettings.make` from
[`RecordingModels.swift:213`](https://github.com/BelloWare/BelloBox/blob/928ac9fd76c1ba0ad73c199a0a7691c7dee46f5e/BelloBox/Recording/RecordingModels.swift#L213).
The acknowledged [B3 claim](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6052285994)
is based on `928ac9fd76c1ba0ad73c199a0a7691c7dee46f5e`
(tree `3ff3b0a79514f1f8cf2b995979ce5978c9b92fc5`).

## Contract

`RecordingOutputSettings::make(width: f64, height: f64, quality)` returns
settings or a typed error. It performs no I/O, allocation of native media,
permission request, credential read or runtime recording admission.

| Preset | Maximum long edge | FPS | Bits/pixel/frame |
| --- | ---: | ---: | ---: |
| Compact | 1600 | 24 | 0.10 |
| Balanced | 2560 | 30 | 0.12 |
| High | 3840 | 30 | 0.16 |

The exact order is source rounding (nearest, ties away from zero), a minimum
source dimension of two, proportional downscaling only when above the preset
edge, output rounding, and upward even padding. Odd sources can gain one padding
pixel; this is not proportional upscaling. Video bitrate is the truncated result
of `Double(width * height * fps) * density`, with a 1,000,000-bit/s floor.
The audio values 48,000 Hz and two channels are metadata; no audio path is enabled.

- Both input dimensions must be finite. NaN and either infinity return
  `NonFiniteDimension`.
- Each rounded dimension must fit the source platform's signed 64-bit Swift
  `Int`: `[-2^63, 2^63)`. The upper bound is exclusive because `i64::MAX as f64`
  rounds upward to `2^63`. Values outside this range return `DimensionOutOfRange`.
  Swift would trap at these conversions; Rust returns an error instead of
  silently saturating or panicking.
- Finite negative, zero and tiny inputs in range retain Swift's two-pixel clamp.
- Scaling bounds each output axis to 2–3840, even; bitrate is bounded to
  1,000,000–70,778,880. Integer products fit `u32`. This policy is specifically
  the existing 64-bit Swift behavior; it is not a configurable writer limit.

The current writer's 32 MiB RGBA admission limit is independent. For example,
High quality for a square 8192-pixel source produces 3840 × 3840, exceeding that
limit. This module does not change or invoke writer validation, recording UI,
platform capture gates, ScreenCaptureKit, cursor/privacy handling or audio.
Serialization and UI/writer wiring are outside this slice.

## Validation on 2026-10-08

Environment: local Apple Silicon macOS 14.8; Homebrew Rust/Cargo 1.91.1;
Apple Swift 6.0.2 from the installed Xcode 16.1 toolchain. Existing dependencies
were used offline/locked. No new dependency or toolchain was installed.
Commands below ran from `rust`, with `CARGO_BUILD_JOBS=2` and
`CARGO_TARGET_DIR=/Users/admin/Library/Caches/BelloRustWork/box-target`.

- `cargo fmt -p bellobox-core -- --check`: passed.
- `cargo test --offline --locked -p bellobox-core recording::output_settings -- --nocapture`:
  **9 passed, 0 failed, 0 ignored**. The other 382 core unit tests and integration
  suites were filtered, not claimed as executed.
- The macOS-only independent oracle extracts the checked-in Swift enum and
  settings struct verbatim, verifies their combined SHA-256, compiles them, and
  compares all six fields for **1,611 cases** (537 sizes × three presets).
  It covers landscape/portrait/square, odd and fractional ties, scaling thresholds,
  very small/negative inputs and the largest valid `f64` below `2^63`.
  Non-finite/overflow errors are covered in Rust tests without intentionally
  crashing Swift. On non-macOS, the eight portable tests remain available.
- Compiler/reference children use isolated temporary files, null stdin, their
  own process groups, a 60-second deadline each and a 2 MiB output check. This
  runs Foundation geometry/math only, without a GUI or device operation.
- A detecting control changed only Rust's High density from 0.16 to 0.15.
  The extracted Swift comparison failed at 479.5 × 479.5 (1,105,920 versus
  1,036,800 bit/s). The exact production bytes were restored and all nine tests
  passed again. The control is not part of the delivered source.
- Strict local `cargo clippy --offline --locked -p bellobox-core --all-targets -- -D warnings`
  remains blocked by the **unchanged** macOS `clippy::needless_return` in
  `crates/bellobox-core/src/settings.rs:199` at the exact base. A diagnostic run
  adding `-A clippy::needless_return` passed. No source allow, CI relaxation,
  unrelated settings change or pinned-toolchain success is claimed.
- No full App/platform/GUI test was run for B3. Exact PR/integration CI results
  belong in the PR/coordination handoff; the base commit's earlier CI does not
  certify this new policy. Existing CI pins Rust 1.99.0. Linux PR CI can run the
  portable tests; macOS CI runs on `rust` push or explicit dispatch after dot's
  integration decision and includes the source oracle through core tests.

Swift source SHA-256:
`7ce94550e7692dd493afd265f7797921072e359014e11211ad04e7e31cc068f2`.
Extracted enum + struct SHA-256:
`402b3a7820e1feb59f741fa3290c1b2a44e9f9427b8700fed98f2b91b534ca5a`.
The oracle pins the excerpts, so an unrelated edit elsewhere in the Swift file
is logged but does not require rebinding the policy.

Final focused output (after restoring the detecting control):

```text
running 9 tests
test recording::output_settings::tests::fractional_ties_round_away_from_zero_before_even_padding ... ok
test recording::output_settings::tests::smaller_sources_are_not_scaled_up_but_odd_dimensions_pad_upward ... ok
test recording::output_settings::tests::finite_zero_negative_and_tiny_values_keep_the_swift_minimum ... ok
test recording::output_settings::tests::source_presets_preserve_dimensions_fps_bitrate_and_audio_metadata ... ok
test recording::output_settings::tests::invalid_dimensions_return_errors_instead_of_saturating_or_panicking ... ok
test recording::output_settings::tests::bitrate_is_truncated_after_floating_density_and_has_a_floor ... ok
test recording::output_settings::tests::square_policy_does_not_silently_apply_the_writer_memory_limit ... ok
test recording::output_settings::tests::policy_outputs_stay_bounded_across_the_source_comparison_matrix ... ok
Swift source SHA256: 7ce94550e7692dd493afd265f7797921072e359014e11211ad04e7e31cc068f2
Swift excerpt SHA256: 402b3a7820e1feb59f741fa3290c1b2a44e9f9427b8700fed98f2b91b534ca5a; matched 1611 cases × 6 fields
test recording::output_settings::swift_oracle::extracted_swift_policy_matches_all_six_fields_for_every_source_case ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 382 filtered out; finished in 2.46s
```

Detecting-control excerpt (expected failure):

```text
assertion `left == right` failed: Swift mismatch for {"width":479.5,"height":479.5,"quality":"high"}
  left: Object {"height": Number(480), "videoBitrate": Number(1105920), "audioSampleRate": Number(48000), "audioChannelCount": Number(2), "framesPerSecond": Number(30), "width": Number(480)}
 right: Object {"width": Number(480), "height": Number(480), "framesPerSecond": Number(30), "videoBitrate": Number(1036800), "audioSampleRate": Number(48000), "audioChannelCount": Number(2)}
```

## Scope and source accounting

Four Rust paths and this new note are the entire change. The only existing Rust
file edit is `pub mod output_settings;` in `recording.rs`. B1/B2 App files,
platform code, shared workbench, workflows and existing evidence remain untouched.
Deliver through a PR targeting `rust`; dot retains shared-branch integration.

Delta against the exact base: **+93 production / +340 test-support
nonblank Rust lines** (comments included, blank lines excluded). The six
`cfg(test)`/module-declaration lines are support; all isolated tests and their
Swift harness/driver are support. The one parent module declaration is production.
No shared workbench code is counted, and the Markdown note is excluded.

| Rust source path | SHA-256 |
| --- | --- |
| `crates/bellobox-core/src/recording.rs` | `6305355eec3a7cb22e637754c95038c862f840156a14170d722d502324d04b18` |
| `crates/bellobox-core/src/recording/output_settings.rs` | `055854012f442c2b5c53152e16b048ca9bb3bedd754ad6542eb2fa4bbeb41a61` |
| `crates/bellobox-core/src/recording/output_settings/tests.rs` | `467cea2ebd3beed2e769bd0704312ec0b3187d32b81b00624be773e99f587c36` |
| `crates/bellobox-core/src/recording/output_settings/swift_oracle.rs` | `fea69cdb7e04542184988f26293f7ae3c51eff58b937ce73ee3630451884467a` |
