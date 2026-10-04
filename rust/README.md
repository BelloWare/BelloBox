# BelloBox Rust migration

Work in progress on the `rust` branch. This does **not** replace the shipped Swift
app or claim feature parity. The original application and its release pipeline
remain in the repository as the behavioral reference.

## Layout

- `crates/bellobox-core`: local utility engines and application state
- `crates/bellobox-app`: GPUI desktop application
- `crates/bello-platform`: platform integration and capability reporting
- `crates/bello-workbench`: shared file tree, Git, document and Vim models
- `crates/bello-workbench-ui`: reusable GPUI workbench/editor UI
- `docs/bellobox-parity.md`: implementation status against native source
- `docs/performance.md`, `perf/`: performance methodology and repeatable tooling

BelloAgent consumes the same workbench crates. During joint development it uses a
local path; published standalone builds must use a pinned Git revision from this
repository, never silently require a sibling checkout.

## Build

Rust stable with GPUI 0.2.2 pinned. Linux requires GPUI's X11/Wayland/font and Vulkan
runtime dependencies. macOS requires Xcode and SDK/framework availability.

Run from this directory:

    cargo test --workspace
    cargo run -p bellobox-app

Read the parity matrix before interpreting functionality. Unsupported platform
capabilities must be reported explicitly rather than returning pretend success.

The libc 0.2.186 dependency is pinned because GPUI 0.2.2's transitive xattr 0.2
references `ENOATTR`, removed in later libc versions. Retain the lockfile.

## Privacy and compatibility

Preserve `com.ainoob.BelloBox`, UserDefaults keys, Keychain service/account names,
Snippets.json data format and Sparkle update trust. Test these on a signed macOS
upgrade. Do not persist selected text, HTTP/JWT drafts, HMAC keys, screenshots or
provider responses as a migration convenience. External requests require explicit
in-app actions. No real provider calls are required for development tests.

## Validation limits

Linux runtime checks cannot establish macOS feature parity, permission continuity,
Keychain access, Sparkle updating, capture or recording correctness. No production
release is implied by these development branch commits.
