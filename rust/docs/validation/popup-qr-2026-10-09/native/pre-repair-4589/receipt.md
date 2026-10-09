migration-peer — complete **pre-repair** native receipt for full-popup QR `4589e0280844df4a070533c7babb9d01cf430905`, tree `294c99a7847757d2c41217834b8dc87eab9e4765`, parent published `15197a71d81c620bf60da18f0d066505e4c0dc99`.

**Three ordinary builds passed. The original matrix remains 296 passed, 5 failed, 3 ignored; format passed. This is not a green native preflight.** Fresh/learned same-binary diagnostic results below are separate and do not replace the first failure. This closes the requested preflight/diagnostic evidence for #issuecomment-6074519168, #issuecomment-6074609621 and blocker #issuecomment-6074597984. The newly requested fixture repair has a separate two-file claim in #issuecomment-6074666951 and has not been included in these results.

| Original native invocation | Passed | Failed | Ignored |
|---|---:|---:|---:|
| Default App qr | 46 | 0 | 0 |
| Default App launcher | 52 | 0 | 1 |
| Minimal App qr | 46 | 0 | 0 |
| Minimal App launcher | 52 | 0 | 1 |
| Movie-fixtures App qr | 46 | 0 | 0 |
| Movie-fixtures App launcher | 47 | 5 | 1 |
| Core QR, once | 7 | 0 | 0 |

The matrix selected79 distinct names (78 runnable plus one existing ignored readiness-cache child), with deliberate overlap between qr/launcher filters and the three feature variants. Selected names/counts were checked against each native compiled test inventory; test binary hashes matched the corresponding inventories. The ignored readiness-cache helper remains invoked only by its own isolated parent test and was not manually enabled by the peer.

All QR groups passed, including shared actual-backend clipboard policy, explicit synthetic image fixtures, refused Copy sentinel preservation, per-window held-key Save refusal, key-release recovery with unchanged draft/selection and no queued Save, decoded Img final square bounds, repeated resizing/refresh and fresh tiny-host absence. The 400x480, 520x620 and 520x700 bounds checks passed. These are native compiled synthetic GPUI tests, not actual desktop clipboard/window/scanner acceptance.

First failure: the movie-fixtures launcher test `discarded_palette_retains_physical_guard_and_rejects_late_actions` panicked at `launcher_clock_ui/copilot/tests.rs:55` with immediate macOS OS35 WouldBlock during loopback socket read; its admission channel then disconnected. Four following copilot lifecycle tests failed with PoisonError on the shared TEST_LOCK. Other selected tests, including the QR lifecycle checks, passed. Original full log SHA-256 `f79f334e313b1b2a4bad444f7d76b9cb9090b8d00b134d40a7fecb745e161205`.

Bounded follow-up: the baseline-lint package clean had removed the first executable, so I reconstructed it from the unchanged candidate and verified an exact SHA-256 match: `79eeaa809ef7ac873ae7288617c8297aff438e5c4513d8b4314b99ce7c010abc`. This byte-identical binary was preserved separately and replayed once with fresh generated configuration and once with a copy of the original learned configuration. **Each replay passed52/0/1.** Both initial/final config hashes and all replay logs are retained. No repeat-until-green loop, assertion change or timeout change occurred. The first failure did not reproduce in these two bounded runs, so no baseline runtime replay was performed; exact baseline source/lint comparisons are separately complete.

Concrete source/runtime diagnosis: both implicated fixture helpers accept from a nonblocking listener, set a read timeout, then read bytes without explicitly resetting the accepted socket mode. Both helpers and the shared worker are byte-identical to published15197. An isolated Rust native numeric-loopback probe reproduced immediate WouldBlock in3/3 samples (1 microsecond each); setting only the probe socket to blocking mode then read the delayed byte successfully after about255ms. This supports a pre-existing macOS fixture timing issue. It does not erase the first failure or establish a candidate production regression. The proposed test-only socket normalization and delayed-byte regression remain a separate repair.

Native strict Clippy remains blocked by exactly matching baseline diagnostics in all three variants. Full strict exits101 at the pre-existing core `settings.rs:253` needless-return diagnostic; App-scoped `--no-deps` strict exits101 with seven existing nonminimal-bool diagnostics. Candidate App locations are desktop.rs263/265, screenshot_ui/quit_tests.rs129/166, shutdown/tests.rs525/582/648; comparison uses source anchors and multiplicities, not shifted line numbers. All six candidate/baseline comparisons match exactly. No lint fix/suppression or native strict pass is claimed. Ordinary build/test/format logs contain no compiler warnings.

Host: macOS14.8 (23J21), arm64 / aarch64-apple-darwin; Rust/Cargo1.91.1, Swift6.0.2, SDK15.1, deployment14.0. Offline locked dependencies, two Cargo jobs, isolated generated configuration, one test thread. Core/App packages were cleaned before candidate ordinary builds and again before baseline lint. Actual native compilation paths and immediate Mach-O arm64 binary hashes are retained. Both exact source snapshots were verified before/after the matrix and after diagnostics:1926 candidate files and1923 baseline files. Eight candidate changed-path postimages are below. All prior3976/83ae/45d5 evidence, original failures and learned-ranking receipts remain hash-unchanged. Original checkout stays clean at59339af1fa422acea4067207c55344666c3fa4ab.

No source edits occurred in this pre-repair validation. No actual desktop, external clipboard, screenshots, user data/credentials, real provider, TCC/AX/capture, tool/vault permission, signing, installation, release, spending or shared rust publication. Launcher copilot tests used generated numeric-loopback provider fixtures only. Dot remains the integrator; actual native GUI acceptance is open.

Eight candidate postimages:
```text
28467350ac13b58fda11a60920495027feec09bff4de79a8103b46573f17c84c  rust/crates/bellobox-app/src/desktop.rs
94433a6e2ea71a42ecc410cdb13300c91e660c810b7d9b41392d712206520508  rust/crates/bellobox-app/src/desktop/qr_clipboard_tests.rs
0e53442ea59b863fbefa8b737201906d05a68425abbb66187bf3196762de8f34  rust/crates/bellobox-app/src/launcher_qr_ui.rs
f0319f4a442fd21430936aa9b8388ed227172bd5f33e07ac5eaf4c4ec55c3722  rust/crates/bellobox-app/src/launcher_qr_ui/tests.rs
ddd0738ec0fef03d2d60c84d1bd49ac8c6b6d2f4bcfa95e23787f92c835ae8b9  rust/crates/bellobox-app/src/main.rs
778d881a4dc7a596ddde97f698a6906fad0d0634d607becc601f0af34dd87278  rust/crates/bellobox-app/src/qr_clipboard.rs
a46d82103610456fc5ef68379984ffa8aa73dd869b7c7e11128f0d7d54d9be3e  rust/crates/bellobox-app/src/qr_input.rs
93ad9391f3d02ac3294f7cee8cf04dffcc1deea5ce42c97482da03ca0f08a1fc  rust/docs/palette-qr.md
```

Original ordinary binary SHA-256:
```text
50292cdc63f6b02ea92c02bf5af84227564c0129f476667299da4bbbc65900e2  build-default
fc280fb7aeec58bb1ee2dd45a2e0dd6f67a72d29d52557407705598c3233730c  build-minimal
96a6b40024c400b87f995a18eb6bb9a754a74736c08e186fce8de1d490792563  build-movie-fixtures
```

Receipt SHA-256:
```text
afd809d783a78161ae4667fcbe97e51aa967093cd83ca41ef39c75442cecbe5d  validation.json
bacbdbde5ff11f0abbb3a83168a6224245ea359404bb29f4f50109bbd55b294a  lint-comparison.json
a163b75e92582174fa3a3cb177d17fc87fbfdcf978ea9016734e5eaa9f3f5239  candidate-changed-postimages.json
91f6d24f76721aabc2a64fa0395606c0a2a39ded9256fe917452d47909ec2353  source-baseline.json
f4b934c5b8beb9b4288f2ddc10eb41015295e21332212c361c581ebc74a40efd  parent-source-baseline.json
b8e5a323bb91b0f318f16e0d5d63d73ec44aefb30e42a521379a5422076046cb  diagnostic-candidate.json
a6df3bffaefcfb53bd50724ab880ea541fa50a951b07f35e5cf1eb85667b1375  native-accept-probe.json
24fd1453966b57ccdc347ca5afb1ba55f1008811ce49d26a06d3ae4b81846805  post-diagnostic-verification.json
afe68ca7fde08f204a1eb452ed9e5aea233f728055ec96529ccd45eaaa00b701  prior-evidence-preservation.json
```

Lossless xz/base64 JSON: files[name]={sha256,text}. Includes original raw build/test/inventory/format/lint/clean logs and commands, all failures, immediate binary hashes, exact inventories, source/lint provenance, same-binary fresh/learned diagnostics, socket probe source/raw outputs, all postimages and preservation hashes. Full source manifests remain local at their recorded hashes. Decode base64 then xz, and verify each UTF-8 file. No screenshots/binaries embedded. Compressed SHA-256 `fcdf25785ffa752cf36d20ffaacccafe4d38caca7ae12e42efab9102242cf22a`; decoded JSON SHA-256 `18ae25ecf1948d3f3437f4e4b7cc8ff25c91637531c9fbd178f134f229e5a012`.

Concatenate the base64 blocks from parts1 and2 before decoding. Full compressed SHA-256 `fcdf25785ffa752cf36d20ffaacccafe4d38caca7ae12e42efab9102242cf22a`.


Full transport: https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6074717477 and https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6074718876

Publication excludes candidate-delta.patch, the standalone Rust probe source and orchestration Python scripts; their original hashes remain in bundle-verification.json and the issue transport. Raw logs and structured outcomes remain included.
