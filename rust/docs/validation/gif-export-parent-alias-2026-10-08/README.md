# GIF result identity across aliased destination parents

Baseline: `e83337517d1e93ac1eb89c269a002e6431d14b65`.

The baseline [macOS run 37719702221](https://github.com/BelloWare/BelloBox/actions/runs/37719702221)
passed the independent Swift movie oracle, pure domain/filesystem suite, generated
movie converter host, and all 35 platform recording tests. The complete App
recording-feature suite then reported **315 passed / 2 failed**. Both failures
compared the canonical export path under `/private/var/folders/...` with the
requested spelling under `/var/folders/...`:

- `gif_converter::tests::movie_gif_switch_keeps_source_owner_and_cancelled_retry_result`
- `gif_converter::tests::preempted_seek_restores_latest_request_after_export_with_an_older_frame_and_prior_gif`

`Target::new` deliberately canonicalizes the destination parent, and successful
export returns that canonical publication path. The failed comparisons did not
show a lost source, overwritten GIF, or cancelled export being published.

## Correction

The movie/GIF switching test now creates an explicit symlinked destination parent
on Unix, so the alias case also runs when the system temporary directory itself
has no alias. It verifies that the published result is canonical and differs from
the requested alias, then retains the entire successful result and GIF bytes across
Movie/GIF switching and repeated cancellation of a retry. The retry must leave no
file. A scoped temporary-directory owner cleans up the fixture and its symlink.
The preempted-seek test compares its retained result to the canonical existing
prior output and keeps all seek, preview-selection and publication assertions.

Only tests and this record change. Source ownership, file publication, native
admission gates, the independent Swift oracle, and production rendering are
unchanged.

## Local validation

Host: Apple Silicon, macOS 14.8 (23J21), Xcode 16.1 (16B40), macOS SDK 15.1,
Homebrew Rust/Cargo 1.91.1. Locked dependency versions are unchanged. Validation
uses generated media and the GPUI test platform, with no capture, permission
request, credentials, provider call or GUI launch.

Both affected GPUI tests passed during the full App invocation:

    CARGO_TARGET_DIR=/Users/admin/Library/Caches/BelloRustWork/box-target CARGO_BUILD_JOBS=2 cargo test --offline --locked -p bellobox-app --features recording-fixtures -- --test-threads=1

Compilation took 2m 41s. The complete suite passed: **317 passed / 0 failed /
0 ignored**, including both repaired regressions, in 128.14 seconds. Workspace
`cargo fmt --all -- --check` and `git diff --check` passed. An independent source
review found no issues with the test correction.

The baseline platform recording suite compiled and executed locally: **34 passed /
1 failed**. The sole failure was the source-bound Swift raw-movie oracle, before
Rust pixel comparison. A separate temporary diagnostic of the same Swift driver
reported zero decoded frames, reader status `completed`, no reader error, and the
native diagnostic `IOServiceMatchingfailed for: AppleM2ScalerCSCDriver`. The actual
H.264 writer, same-token decoder, cancellation and held finish-callback tests
passed. The baseline's exact macOS CI oracle passed, so no threshold, skip or
source-renderer change was made for this local environment result.

This record does not establish live recording, Intel parity, interactive native
acceptance, packaging, or a complete green CI run for the correction.
