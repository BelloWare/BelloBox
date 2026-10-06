# Owned native Area overlay foundation

This API-only checkpoint prepares an Apple Silicon macOS 14+ main-display Area selector. Production Area remains disabled. No screen capture, permission grant, native presentation, or runtime performance result is claimed.

The helper resolves CGMainDisplayID to its exact NSScreen, validates unrotated geometry and scale, and configures only a borrowed, hidden GPUI popup identified by its actual view/window handle. It never searches by window title or retains a native window pointer. CGRect getters use checked Objective-C method signatures and typed ARM64 implementations. Other architectures and unsupported layouts fail closed.

Owned-window visibility APIs use source-style orderOut without app deactivation. Callers must preflight all live logical handles and record prior visibility before any mutation; restoration must revisit only those still-live handles. Fullscreen, sheet and parented targets are rejected. Restoration preserves app/key state and orders behind a newer key window or at the back while inactive. Unsafe cross-level restoration fails closed rather than obscuring newer navigation. The caller must independently fence generations, cancellation, close and focus changes.

The only dependency change is a direct macOS edge to already-locked raw-window-handle 0.6.2. No package version is added. The source references and precise ownership contracts are recorded alongside the helper APIs.

Validation on the integrated Area foundation: 16 focused helper policy tests pass; strict aarch64-apple-darwin platform test-target Clippy passes. Independent review covered borrowed-handle identity, all-error-path restoration requirements, checked CGRect ABI, fullscreen rejection, and navigation-preserving ordering. Actual AppKit geometry, Spaces, Retina, stacking, focus and capture remain native runtime gates. A guarded synchronous presentation API and production caller are subsequent work.
