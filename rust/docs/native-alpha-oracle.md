# Supplied-image native alpha mask and synthetic oracle

## Why the macOS path uses CoreGraphics

The recovery patch was reviewed against `ImageAlphaMask.swift` before integration.
Its active successor is `native_capture/alpha_mask_tests.rs`; the original recovery
patch remains unchanged historical evidence and must not be reapplied.

The first oracle compared the portable integer nearest sampler strictly against
CoreGraphics. At `adb877e58dd65370a8fdcfd1617cdcc68a041aa1`, Linux CI
[37571312929](https://github.com/BelloWare/BelloBox/actions/runs/37571312929) passed.
Native macOS [37571312948](https://github.com/BelloWare/BelloBox/actions/runs/37571312948)
compiled/linked all test targets, then found a real mismatch: frozen2×3, shape1×2,
pixel(0,1), CoreGraphics alpha17 versus portable60. Same-size exhaustive alpha,
palette, RGB-alpha and incompatible-size tests passed; successful RGB diagnostics
were skipped after the failure.

The expanded diagnostic at `780ecd94967651a207c18a58f5ae4e60e7c571b4`, macOS
[37571908312](https://github.com/BelloWare/BelloBox/actions/runs/37571908312), collected
the complete bounded corpus before failing its exact assertion. Center-boundary
choices differed on both axes and depended on scale: output3/source2 or4 chose
lower; output5/source4 chose lower but source6 upper; output7/source8 chose lower
but source6 upper. No guessed tie rule or tolerance was added.

The source application itself uses CoreGraphics. The macOS Rust host now delegates
supplied-image masking to the same drawing operation through existing typed Apple
bindings. Linux's portable sampler remains deterministic and explicitly approximate
for ±1-pixel resampling. The retained characterization test still asserts the
original17-versus60 difference. Changing the native backend is not proof that the
portable algorithm became equivalent.

## Native adapter and common publication seam

`bello_platform::native_capture::mask_image_alpha` receives two borrowed, tightly
packed, normalized SDR straight-RGBA8 image inputs. It does not acquire images,
enumerate windows/displays, check/request permissions, capture, access files, or
contact a provider. Non-macOS is explicitly unsupported for this local native API.
All existing production Area/Window/movie capture gates remain unchanged.

The core `WindowRefreshPlan::prepare_with_alpha_mask` callback sees immutable BASE
pixels only. Annotations, crop, font and rendered overlays cannot be baked into
an eventual clean-after-Undo replacement. Size and cancellation are checked before
the callback; its result must match the frozen dimensions exactly and have no
annotations/crop. Common edit-session/base/context/cancellation publication guards
remain unchanged. Errors never mutate the editor. The actual macOS editor host
calls this seam on its worker; Linux uses the existing portable backend.

Native inputs use explicit RGBA byte order and straight alpha, copied fallible
CFData storage and retained data/provider/color/image owners. The output exactly
matches the source configuration: DeviceRGB, PremultipliedLast without an extra
byte-order flag, CoreGraphics-allocated bitmap storage/automatic stride,
interpolation None, frozen draw then DestinationIn shape, immutable snapshot.
Existing bounded PNG encoding runs synchronously on the same worker. No native
owner or raw image pointer crosses threads.

Validation rejects zero/oversized dimensions, overflow, missing/trailing/padded
bytes, and shape differences exceeding one pixel on either axis. The combined
tight input RGBA byte limit is64MiB, smaller than the general screenshot limit;
a larger pair leaves the frozen screenshot usable. Precisely, every dimension is
1..=32,768, each axis differs by at most1, each slice is exactly width×height×4
bytes, and4×(frozen_width×frozen_height + shape_width×shape_height)≤67,108,864.
For equal-size images this is at most8,388,608 pixels per image:3840×2160 fits,
4096×2048 is exactly the byte boundary,4096×2160 and5120×2880 do not fit. A±1 shape
must also satisfy the combined inequality, so the equal-size boundary can reject
a slightly larger shape. This adapter does not support every general screenshot
size. Native copied input data,
bitmap/snapshot and encoded PNG are additional allocations. The PNG encoder has
its existing64,000,000-byte cap. These are visible buffer bounds, not a claim about
opaque CoreGraphics/ImageIO peak memory or total process memory.

One independent mask lease prevents overlapping native mask jobs. It remains held
through synchronous native operations, encoding, cleanup and final deadline
resolution. Cancellation and the fixed10-second deadline reject late publication;
they cannot forcibly interrupt an opaque framework call or release its lease early.
Capture admission is separate and no capture lifecycle is enabled by this adapter.

## Strict tests and retained limitations

Six macOS oracle tests exercise the public core plan plus actual native adapter
against an independently constructed Swift-shaped oracle on identical normalized
inputs. Full RGBA equality is strict, without tolerance. Coverage includes all
65,536 alpha pairs, asymmetric palette/shape, equal and±1 dimensions on both axes,
1×N/N×1 and larger127/128/254/255 matrices, incompatible-size rejection, and144
low-alpha RGB samples. The normalization boundary is native PNG→core decode→
straight RGBA; this deliberately does not claim original ICC/HDR capture fidelity.

The portable same-size alpha corpus remains checked. Its low-alpha RGB differences
are reported separately with straight-RGB and diagnostic premultiplied-projection
counts; no threshold or broad RGB-equivalence claim is inferred. Original failed
native evidence above remains part of the record.

Run on actual macOS:

    cargo test --locked -p bello-platform native_alpha_ -- --show-output
    cargo test --locked -p bellobox-app screenshot_ui::window_refresh::tests::supplied_image_host_mask_prepares_on_worker -- --show-output

The second command is a plain worker test of the actual host mask wiring. It does
not create a GPUI application/window or perform native capture. The macOS workflow
runs both commands; additional native step/lease regressions run with platform tests.

## Verified exact checkpoint

At `c41ce17a520a9bc39073e2302c5a061d29dd27df` (tree
`fd17a83c4ddf4d5b9e54158250d89ed1a7091f2e`), both exact runs passed:

- [Linux 37575287239](https://github.com/BelloWare/BelloBox/actions/runs/37575287239):
  workspace tests, strict Clippy, build and software-rendered startup smoke.
- [macOS 37575287155](https://github.com/BelloWare/BelloBox/actions/runs/37575287155):
  all native test targets compiled/linked, 153 platform tests passed (3 existing
  ignored), all 9 filtered alpha/oracle/lifecycle tests passed, the actual
  supplied-image host worker test passed, and app build/offline packaging passed.

The native adapter matched the independent normalized-input oracle exactly in
all RGBA channels for the tested corpus. The portable limitations remain measured:
in the 144-pixel low-alpha corpus, 89 pixels differed in straight RGB (maximum
channel delta 129); the diagnostic integer-premultiplied projection differed in
33 pixels (maximum channel delta 1). The original sampling case remains native
alpha 17 versus portable 60. No tolerance was introduced or inferred from these
numbers. [Machine-readable evidence](validation/native-alpha-2026-10-07/ci-evidence.json)
retains the exact run/job identities, counts and limits.

Local focused validation also passed: 25 core refresh/seam tests, 129 platform
tests (3 existing ignored), 43 app screenshot tests, strict component Clippy and
formatting. Independent review fixed outdated helper test call sites and added
actual host-wiring, lifecycle and trailing-storage coverage.

This establishes the tested normalized-SDR mask path only. It does not establish
original capture color/ICC/HDR fidelity, native capture permission/callback/focus/
Spaces/Retina behavior, every framework implementation, native GUI acceptance,
or whole-application parity. Production capture gates remain disabled.


## Reviewed source delta

Relative to the published synthetic host `ebaa29d`, this native-mask checkpoint
adds 372 production and 641 test/support nonblank Rust lines. Totals are
42,267 production, 19,802 test/support and 105 benchmark lines.
The scoped per-file hashes/classification are retained in
[loc-delta.json](validation/native-alpha-2026-10-07/loc-delta.json); concurrent
converter work is excluded. These are source counts, not parity or performance.
