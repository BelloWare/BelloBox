# Callback-local Window adapter: production disabled

The Window backend is compiled for Apple Silicon/macOS tests only. Public
`capture_window` returns Unavailable before enumeration, native permission checks
or image acquisition on every platform. No chooser, frozen-overlay UI integration,
new dependency, capture call or permission grant is enabled by this checkpoint.

## Source behavior and deliberate policy revision

Swift ScreenCaptureService.captureWindow constructs a desktop-independent-window
filter and sets only width, height, cursor and audio=false. This implementation
preserves the remaining shadow/opacity/clipping defaults and has no display-crop
fallback. The active overlay first keeps its frozen crop and later uses a live
independent image for occlusion replacement or alpha shape; this backend does not
replace that host behavior or reuse Screen's hide/restore delay.

Within the shareable-content callback, exactly one selected SCWindow is resolved,
locally retained and checked against bounded selected/fresh CG metadata. The actual
process PID is independently excluded at entry, native extraction and filter
creation. Full bundle conversion rejects embedded NULs and oversize/invalid data.
The same SCWindow is supplied to the filter; native request objects remain local
through submission and are released on that callback thread. This uses the same
Cocoa async-argument ownership basis as the existing Display adapter, not a new
claim about SCScreenshotManager's internal retention implementation.

The old policy's same-native-object reread at completion was deliberately removed.
Owned immutable submission evidence crosses queues, not SCWindow/filter/config
pointers. Fresh CG identity/frame/layer/alpha/on-screen evidence and full topology
are checked after encoding. CG bundle absence permits no second bundle-validation
claim. Snapshot comparisons do not establish atomic window incarnation or detect
changes that occur and revert between observations.

## Ownership, lifecycle and validation

Display, Region and Window use one capture admission slot and the same retained
immutable CGImage/PNG transport. The existing Display validation order is preserved.
Window's specialized main-queue contexts contain only owned Rust state; they collect
NSScreen/CG topology before enumeration and after PNG encoding. Main dispatch binds
the exported `_dispatch_main_q` object, not the inline header accessor. CGRect
Method/IMP lookup checks arguments and the exact four-double return encoding.

Current session/generation is read at each async boundary and locked through final
policy acceptance and Job completion. Cancellation/deadline checks bound waiting
and publication, not opaque framework execution. Generation changes or a separate
selection cancellation flag do not interrupt ImageIO already encoding: the PNG sink
checks its operation flag/deadline, while later Window checks reject stale output.
Native callbacks, encoder and final validation retain independent leases; timeout
cannot reopen the slot while their owned work remains. PNG bytes move without an
extra full copy. The future host must still recheck document/session/cancellation
at its actual UI publication boundary.

## Evidence and remaining gates

Independent ownership/race review cleared the final source. Fresh integrated
Linux platform tests pass 121 cases with three existing opt-in subprocess tests
ignored; strict platform test-target Clippy, formatting and diff checks pass.
The revised policy has 33 focused tests; final capture tests cover 29 cases.

Full local Apple Cargo checking is blocked by the existing offline source-cache
limit. A separate metadata-only rustc/Clippy check compiled the actual native and
policy modules/tests against cached Apple-target dependencies, without fake SDK
or framework shims. This is not Cargo/SDK linking or native execution evidence.
Six new native fixtures cover bounded metadata, CGRect ABI, Rust-only state,
PNG ownership and stale finalization; they have only been type-checked locally.
The existing GitHub macOS workflow will compile/link and execute those synthetic
tests, including the unchanged retained-image/dispatch tests. None invokes SCK
capture or requests permission.

Actual SCK lifetime, main-loop delivery, TCC, pixels/cursor/shadow/alpha, display
changes, minimize/close/resize races and protected-content behavior remain runtime
gates. macOS 13 fallback and x86_64 Window are unsupported. Production availability
must not be inferred from portable or synthetic validation.
