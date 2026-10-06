# Native capture off-main PNG encoding checkpoint

Date: 2026-10-06 UTC. Base: `b2e27d6cd730afbfff69a834568da9c4e4334459`.
Worktree: `BelloBox-capture-encode-handoff`. Rust platform-only change, isolated
from the frozen Area core/overlay work. No dependency or lockfile change.

## Scope and invariant

The ScreenCaptureKit image completion queue is undocumented. Starting capture on
a Rust worker does not establish the completion queue. The callback now does only
phase/error/null checks, a native image retain, small owned metadata/Arc copies,
and asynchronous queue submission. Image dimensions, display-change checks and
all ImageIO destination/consumer work execute in the explicit dispatch worker.

The synchronous macOS capture entry points reject the actual Cocoa main thread
with `CaptureError::WorkerThreadRequired`, before permission preflight/native
capture/wait. The encoder has the same defense before ImageIO. These guards use
`NSThread.isMainThread`, not Rust thread names or queue identity. The ordinary
Rust test harness cannot prove rejection on the application's actual main thread.

The queued worker creates and destroys its own autorelease pool. The two existing
SCK closure capture sets still must pass the `Send + Sync` compile-time assertion.
Native request objects continue to live only through native request submission;
the copied SCK blocks capture no raw pointer or native owner.

## Ownership and cancellation proof

1. `Image -> EncodingQueued` is claimed before any retain. A duplicate, abandoned,
   canceled or timed-out callback cannot claim another image/context. Null/error
   results stop before retaining. A cancellation racing after the claim may still
   enqueue one context; the worker will reject it without starting ImageIO.
2. `OwnedCaptureImage` performs one `CGImageRetain` and one `CGImageRelease`.
   It is private, non-Clone and has no unsafe `Send`/`Sync` implementation. This
   narrow transfer applies only to the immutable image delivered by SCK.
3. The context contains the retained image, owned request/resolution values,
   `Arc<Job>` and a clone of the operation lease. The borrowed global queue is
   obtained with `dispatch_get_global_queue(0, 0)` and checked for null before
   ownership leaves Rust. Its identifier/flags use `isize`/`usize`, matching
   `intptr_t`/`uintptr_t`; the work pointer is `unsafe extern "C" fn(*mut c_void)`.
4. One `Box<EncodeContext>` becomes a pointer immediately before
   `dispatch_async_f`. No intervening fallible/panicking Rust operation exists.
   One specialized C entry point reconstructs that exact Box once. No generic
   non-Send dispatcher, pointer-as-integer transfer, or pointer channel is used.
5. Libdispatch schedules the function with the unchanged context; it does not
   free the allocation. The waiter owns only `Arc<Job>`, never the context pointer.
   Cancellation/timeout cannot reclaim queued native state or race its destruction.
6. The worker claims `EncodingQueued -> Encoding`, checks interruption before
   ImageIO and between adding/finalizing the image; the existing sink checks every
   write and the existing Job fence rejects late publication. The sink remains
   capped at 64 million PNG bytes; partial/oversized output cannot publish.
7. The inner unwind fence converts encoding panics to `NativeFailure`; an outer
   fence also covers context cleanup before returning to C. Panic payloads are
   forgotten rather than allowing a custom payload destructor to unwind into C.
8. ImageIO temporaries and the worker pool are disposed before the context.
   `EncodeContext.image` precedes its lease field, so the retained native image is
   released before that lease on success, failure, cancellation or Rust unwind.
   The original SCK callback lease stays captured until the framework drops it.
   Either the callback or worker may finish first without releasing the slot too
   soon. Publication precedes final context teardown, so a waiter can briefly see
   Busy after receiving its result; this intentionally preserves the resource bound.

## Primary API basis

- Apple's [CGImage documentation](https://developer.apple.com/documentation/coregraphics/cgimage?changes=latest__1)
  lists Sendable conformance. This specific image contract, rather than a generic
  CoreGraphics thread-safety assumption, is the basis for the narrow transfer.
- Apple's [Core Foundation thread-safety guidance](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html)
  permits querying, retaining, releasing and passing immutable objects between
  threads. Mutable-object/content rules are not generalized to arbitrary images.
  The same guide requires secondary threads to manage their autorelease pools.
- [CGImageRetain](https://developer.apple.com/documentation/coregraphics/cgimageretain)
  and [CGImageRelease](https://developer.apple.com/documentation/CoreGraphics/CGImageRelease)
  supply the independent native ownership reference.
- Apple's [libdispatch queue header](https://raw.githubusercontent.com/apple-oss-distributions/libdispatch/main/dispatch/queue.h)
  declares the async function/context and global-queue scalar types; priority zero
  is the default and flags zero is supported. The [dispatch_function_t API](https://developer.apple.com/documentation/dispatch/dispatch_function_t)
  describes the context pointer supplied to the function.

These contracts support the unsafe FFI boundary; Rust's cross-target checker does
not validate Apple's ABI/linkage or execute native ownership behavior.

## Executed focused checks

Environment: Linux; existing Rust 1.99.0 toolchain/cache; no package retrieval.
All Cargo commands used these values and remained within `bello-platform`:

```sh
export CARGO_HOME=/workspace/scratch/5ddc1675ed73/.cargo
export RUSTUP_HOME=/workspace/scratch/5ddc1675ed73/.rustup
export CARGO_TARGET_DIR=/workspace/scratch/5ddc1675ed73/target-clock-ui-final
export CARGO_BUILD_JOBS=2

$CARGO_HOME/bin/cargo test --locked --offline --manifest-path rust/Cargo.toml \
  -p bello-platform --lib native_capture -- --test-threads=2
$CARGO_HOME/bin/cargo check --locked --offline --manifest-path rust/Cargo.toml \
  -p bello-platform --tests --target aarch64-apple-darwin
$CARGO_HOME/bin/cargo clippy --locked --offline --manifest-path rust/Cargo.toml \
  -p bello-platform --tests --target aarch64-apple-darwin -- -D warnings
$CARGO_HOME/bin/cargo clippy --locked --offline --manifest-path rust/Cargo.toml \
  -p bello-platform --tests -- -D warnings
```

Results: 23 portable capture tests passed, 0 failed (25 filtered out). Apple
test-target check and both strict Clippy checks passed. Modified Rust files were
formatted with the existing rustfmt. `git diff --check` passed.

The seven new portable tests exercise the production Job and operation lease,
with a clearly labeled queue/image ownership model: queued cancellation, queued
timeout with exclusive raw-Box reconstruction, original-callback/worker completion
in either order, duplicate queue claim, panic destruction order, pre-submission
failure and pre-queue cancellation. Existing tests retain late publication,
cancellation/deadline precedence, bounds and stale-display policy coverage.

Five macOS-only tests were added and type-checked, but were NOT executed here:
- Synthetic 2x2 bitmap -> PNG on an explicit Rust worker, after explicitly
  retaining the image and dropping its original owner
- Real dispatch handoff of a retained synthetic image, duplicate callback rejection,
  teardown/lease release, and execution off the actual main thread; display ID zero
  fails before any display query/image read, so this fixture needs no capture/GUI
  session. It does not establish retained-image usability across the asynchronous
  callback lifetime; native instrumentation remains required for that claim
- Canceled callback skips native retain and queue submission
- Null/error callback does not allocate a worker context
- PNG consumer checks byte bound/cancellation before reading source bytes

## Remaining gates and limits

The bounded claim is one native operation / queued retained image / encoder,
64 million pixels, 32,768 pixels per axis and 64 million PNG output bytes.
The ten-second Job deadline bounds waiting/publication, not native execution.
CoreGraphics/ImageIO temporary allocations and CPU/runtime remain unbounded native
validation gates. A stalled SCK callback or dispatched native encode can keep the
adapter Busy indefinitely; no timed-out waiter frees it or initiates another one.

No real SCK capture, permission request/grant, screenshot bytes, GUI, clipboard,
filesystem export, network upload, broad workspace build or app link was performed.
Apple SDK link checks, native retain/release behavior, synthetic test execution,
actual application-main-thread rejection, framework cancellation timing and real
capture/image correctness still require a macOS validation lane. This change does
not close macOS 13 fallback, window capture, trust verification, protected-content,
Area-overlay or scrolling-source parity gates from the original checkpoint.
