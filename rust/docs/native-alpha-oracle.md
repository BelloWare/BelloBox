# Synthetic native Window alpha oracle

The recovery patch was reviewed against `ImageAlphaMask.swift` before integration.
Its active successor is `bello-platform/src/native_capture/alpha_mask_tests.rs`.
The recovery file remains unchanged historical evidence, not a patch to reapply.

Five macOS-only tests exercise the public WindowRefreshPlan path against a typed
CoreGraphics implementation of the exact Swift drawing sequence: DeviceRGB,
premultiplied-last, allocated bitmap storage with automatic stride, interpolation
None, frozen-image draw followed by DestinationIn shape, immutable image snapshot.
Inputs use explicit premultiplied RGBA byte order and copied CFData storage. All
native objects are created, retained and dropped on one worker thread. Existing
native PNG encoding and core decoding normalize both native input and output;
input alpha and the opaque palette are independently checked.

Coverage includes all 65,536 input alpha pairs, an asymmetric palette/shape,
nearest sampling with each axis equal or one pixel larger/smaller, rejected size
differences leaving the original document untouched, and 144 low-alpha RGB cases.
Alpha and sampling comparisons are exact. RGB quantization is reported without a
tolerance or a claim of universal straight-RGB equivalence. Tests do not enumerate
windows, query displays, capture, request permissions, access files or contact a
provider. The selection context uses a selectable 16-point surface with each
corpus's pixel dimensions, so the public identity path and actual raster agree.

Run on actual macOS:

    cargo test --locked -p bello-platform native_alpha_ -- --show-output

The macOS workflow includes this command to retain successful RGB diagnostics.
Source review and formatting are complete; native compile/execution are pending
for this checkpoint. Exact CI results must be recorded before claiming the corpus
passes. Linux compilation excludes the module and cannot validate these bindings.
Even a passing corpus does not establish ICC/HDR, capture fidelity, native UI,
TCC/Spaces/focus behavior or production Window enablement. All capture gates remain.

## First native result

At `adb877e58dd65370a8fdcfd1617cdcc68a041aa1`, Linux CI
[37571312929](https://github.com/BelloWare/BelloBox/actions/runs/37571312929) passed.
Native macOS [37571312948](https://github.com/BelloWare/BelloBox/actions/runs/37571312948)
compiled and linked all test targets, then exposed a genuine strict resampling
mismatch: frozen 2×3, shape 1×2, pixel (0,1), CoreGraphics alpha 17 versus portable
60. Same-size exhaustive alpha, palette, RGB-alpha and incompatible-size tests
passed; the successful-RGB diagnostic step was skipped after that failure.

The resize oracle now collects all bounded corpus mismatches before its final
strict assertion, to determine the native axis/tie pattern before changing the
portable sampler. This neither introduces a tolerance nor ignores a failing case.
Production enablement remains blocked. No general alpha/sampling equivalence claim
is made while this mismatch remains unresolved.
