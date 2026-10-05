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

The development preview uses `com.ainoob.BelloBox.rust.preview`, a separate
preferences identity from the shipped Swift app. `scripts/package-bellobox-macos.sh`
assembles an ad-hoc-signed preview without installing or launching it. By default
it includes no Sparkle feed, key or framework, and disables automatic updates.
An optional preview feed requires an explicit public key; the Swift production
host (including subdomains) and Developer ID signing identities are rejected by
preview packaging. Preview feeds use a deliberately narrow ASCII HTTPS DNS-name
format without ports, credentials, fragments or percent escapes; public keys must
be canonical base64. Native preflight applies the same restrictions.
Packaging builds offline and requires dependencies cached by a prior normal build.
`PACKAGE_PROFILE=debug` reuses the debug build configuration for offline CI checks.
Run `python3 scripts/validate-bellobox-macos.py "dist/BelloBox Rust.app" --require-offline`
to check default preview assembly. This does not establish native UI, permission,
Sparkle runtime, notarization or distribution readiness.

A future, separately reviewed production migration must preserve the original
`com.ainoob.BelloBox` identity, UserDefaults keys, Keychain service/account names,
Snippets.json data format and Sparkle update trust, tested on a signed macOS
upgrade. The current preview updater deliberately rejects that production identity. Do not persist selected text, HTTP/JWT drafts, HMAC keys, screenshots or
provider responses as a migration convenience. External requests require explicit
in-app actions. No real provider calls are required for development tests.

## Validation limits

Linux runtime checks cannot establish macOS feature parity, permission continuity,
Keychain access, Sparkle updating, capture or recording correctness. No production
release is implied by these development branch commits.
