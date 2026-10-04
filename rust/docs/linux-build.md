# Linux build and native runtime

## Verification scope

The initial development environment is Debian 13 (trixie), x86_64, GCC 14,
Rust/Cargo 1.99.0, GPUI 0.2.2, and a live Xfce X11 desktop. The GPUI upstream
Hello World example compiled and visibly rendered on 2026-10-04. This establishes
the build/display toolchain, not application feature parity or macOS validation.

The available renderer is **Mesa llvmpipe (LLVM 19.1.7), software Vulkan 1.4**.
Report measurements from this machine as Linux software-rendered results. Do not
compare these numbers directly with macOS Metal or claim hardware GPU speed.

## System packages

On a fresh Debian 13 machine:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends \
  build-essential pkg-config curl ca-certificates \
  libc6-dev libfontconfig-dev libxkbcommon-x11-dev libwayland-dev \
  libx11-xcb-dev libasound2-dev libzstd-dev libssl-dev libvulkan-dev \
  mesa-vulkan-drivers vulkan-tools cmake clang-19 lld-19
```

The package manager resolves the required FreeType, XCB, PNG, Expat and C++
headers. This is the validated dependency set, not a claim that every listed
package is strictly minimal. Other distributions need their corresponding
packages; the Debian command has not been validated on Ubuntu or Fedora.

An X11 or Wayland session is required for interactive windows. For the verified
screenshot route, also install ImageMagick (`sudo apt-get install imagemagick`).
A headless Xvfb route is not yet validated here.

## Rust and dependency locking

Install Rust with the [official rustup instructions](https://rust-lang.org/tools/install/).
Rust 1.99.0 was used for the initial Linux verification. The workspace's declared
minimum Rust version is not established by that one successful build; CI should
exercise the minimum separately before advertising it as tested.

Use the checked-in lockfile for builds and CI:

```sh
cd rust
cargo build --locked --workspace
cargo test --locked --workspace
cargo run --locked -p bellobox-app
```

GPUI is pinned to `=0.2.2`, with `default-features = false` and features
`["x11", "wayland", "font-kit"]`. Keep `libc = "=0.2.186"` constrained and retain
the lockfile: GPUI's `gpui_http_client` → `zed-async-tar` → `xattr 0.2.3` dependency
references `libc::ENOATTR`, which is absent in libc 0.2.190. Resolving the newest
unconstrained libc made the initial build fail; 0.2.186 built successfully.

If intentionally regenerating dependencies, inspect every change and rerun the
whole build/test/runtime checks. Do not modify the registry source to hide errors.

The development profile disables debug symbols and incremental compilation to
reduce build storage and memory. The initial machine has approximately 9.7 GiB
RAM and no swap. Use `CARGO_BUILD_JOBS=4` or fewer and avoid simultaneous independent
workspace builds. A shared Cargo target directory is safe when builds use matching
profiles and flags, and Cargo's lock serializes writers.

## Normal Linux runtime

Use the desktop session's `DISPLAY`/`WAYLAND_DISPLAY`. To explicitly select X11:

```sh
WAYLAND_DISPLAY='' cargo run --locked -p bellobox-app
```

Inspect the actual Vulkan driver before interpreting performance:

```sh
vulkaninfo --summary
```

For intentional software-rendered testing with Debian's Mesa package:

```sh
VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json \
  WAYLAND_DISPLAY='' cargo run --locked -p bellobox-app
```

The ICD file name varies by distribution and Mesa packaging; verify its installed
path rather than assuming one. A missing or incompatible Vulkan device is a
runtime blocker and must be reported honestly.

## Real screenshot verification

Inspect the running application, then capture its actual X11 window with the
installed ImageMagick `import -window WINDOW_ID output.png` command from the same
desktop session. Verify the saved PNG before sharing it. Label upstream smoke
images as toolchain checks; they must never be presented as application UI.
Preserve logs and exact build identity beside performance records. A screenshot
does not establish frame rate or interaction correctness.

## CI boundaries

Linux CI should run locked workspace builds/tests and repeatable non-UI benchmarks.
An interactive screenshot needs a separately verified display/runtime setup.
Linux checks do not validate macOS Accessibility, Keychain, ScreenCaptureKit,
permissions, signing, Sparkle updates, or compatibility with existing user data.
Keep those checks as explicit macOS gates.

## Primary references

- [Rust installation](https://rust-lang.org/tools/install/)
- [GPUI 0.2.2](https://crates.io/crates/gpui/0.2.2)
- [Zed Linux development dependencies](https://zed.dev/docs/development/linux)
- [Zed Linux graphics requirements](https://zed.dev/docs/linux)
- [Debian trixie repository](https://deb.debian.org/debian/dists/trixie/)
