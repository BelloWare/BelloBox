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
