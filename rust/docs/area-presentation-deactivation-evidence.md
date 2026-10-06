# Owned Area presentation and application-deactivation helpers

This platform-only checkpoint adds guarded synchronous presentation for the exact owned Area popup and a source-shaped application-deactivation observer. Production Area remains disabled; no native capture, permission request or desktop interaction was performed.

The unpublished earlier candidate was lost when the cloud workspace was replaced. These files were freshly reconstructed from the published base and original Swift source, then independently reviewed and tested. They are not represented as recovered identical bytes or as carrying forward old test results.

Presentation borrows both currently live GPUI popup/requester owners for one UI-thread call. It validates configuration, topology, cancellation and actual app/key state before ordering, between ordering and key acquisition, and afterward. Failure hides only the verified popup. It avoids GPUI's queued native activate_window path and never activates the application. The future caller must hold both owners through nested updates, validate refreshed GPUI viewport/scale, and maintain its generation/cancellation fences.

The observer follows CaptureOverlayController's NSApplication.didResignActive subscription. A non-Send UI guard owns retained NotificationCenter/token references. Foundation copies a block that captures only bounded Send+Sync Rust state. Notification cancels the transaction before waking a pointer-free one-shot signal. Drop closes that signal before unregistering/releasing; queued late callbacks become inert. Waking occurs outside the mutex behind a panic barrier. The caller must retain the guard until cancellation or accepted editor handoff and interpret ObserverRemoved as waiter termination, never successor cancellation.

Fresh validation: 25 focused macos_capture_overlay policy/observer tests pass, including presentation cancellation and six notification/removal/waker/race cases. Strict bello-platform test-target Clippy passes on Linux and aarch64-apple-darwin; formatting and diff checks pass. No package/dependency change was needed.

Native notification delivery, AppKit reentrancy, actual GPUI view geometry/key focus, Spaces/Stage Manager, native restoration and ScreenCaptureKit pixels remain runtime gates. The app caller is separate work; this API checkpoint does not enable Area or claim full screenshot parity.
