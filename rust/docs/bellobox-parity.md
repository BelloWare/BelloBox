# BelloBox Rust parity ledger

Baseline: Swift commit `e43b1c4`. Rust lives under `rust/`; Swift code, project,
UserDefaults, Keychain entries, bundle identity, and `Snippets.json` are not migrated
or modified. This is an incremental, runnable port, **not feature parity**.

## Evidence and status vocabulary

- **Implemented** means executable code exists, with the cited checks.
- **Partial** means a useful subset exists; omitted behavior is listed.
- **Platform-blocked** means validation requires a platform/session unavailable here.
- **Not ported** means the Rust application does not implement the feature.
- A catalog entry is not evidence that a feature works. Unsupported operations return
  explicit errors. The minimal `--no-default-features` build intentionally excludes
  all developer utility engines and labels them unavailable.

The current source-preserving checkpoint passes 140 core unit tests, 3 malformed-input
integration tests, 28 app layout/session/settings/screenshot tests, 16 platform unit tests and
3 opt-in subprocess tests. Local
Tesseract OCR was exercised on a synthetic image. The first generic GUI prototype
was rejected and replaced: the normal app now opens the current Swift Home
category/card structure; tools and launcher have separate windows. Real Linux
Home and QR screenshots were reviewed; QR input edits update its visible code.
Final source-layout refinements still require recapture. Native platform code
passes Apple ARM Rust type checks and strict Clippy, including compilation of
native tests. It has not linked against an Apple SDK or executed on macOS. No AI
provider was called.

UI preservation is a requirement, not a redesign opportunity. `MainView.swift`,
`UtilityWorkbenchView.swift`, `QRCodePopupView.swift`, `TextToolsPopupView.swift`,
`LauncherView.swift` and `Theme.swift` from the checked-out source are authoritative.
Official older visual references (0.0.66) corroborate the hierarchy but cannot
replace current source (0.0.77) labels, horizontal cards or unified orange badges:
[Home](https://belloware.com/assets/bello_box_home_workspace.jpg),
[JSON](https://belloware.com/assets/bello_box_json_tools.jpg),
[palette](https://belloware.com/assets/bello_box_command_palette.jpg),
[screenshot editor](https://belloware.com/assets/bello_box_screenshot_editor.jpg).
No claim of complete visual or interaction parity is made.

## Application and domain parity

| Swift source / capability | Rust implementation | Status and remaining work |
|---|---|---|
| `Launcher/LauncherCatalog.swift`, 61 commands including 51 developer tools | `bellobox-core::launcher`, source Home and separate GPUI palette | Partial: all IDs/titles, title-weighted search, basic selection suggestions, favorites/recents, coarse learning. Independent per-tool windows and New Window sessions are implemented. Separate 680px search palette has keyboard navigation and a single expanded read-only preview. Global shortcut/nonactivation, full interactive preview-session transfer, and complete suggestion classifier remain absent. |
| `Launcher/LauncherUsageStore.swift` | `launcher::Usage`, `settings::Settings` | Implemented portable 30-day decaying, bounded category/tool counters. Tests cover cap/decay/title precedence. Only explicit tool opens learn. Selection, text, URL, app identity and fingerprints never persisted. |
| `Selection/AccessibilityService.swift`, `SelectionRequest.swift`, `SelectionMonitor.swift` | Explicit GPUI clipboard import; permission preflight | Partial: cfg-gated direct AXSelectedText reader with protected-ancestor/range/source/window checks and 160 ms budget, exposed through explicit macOS --selection CLI; Apple-target type-checked, not runtime-tested. No marker-range fallback, retries, global hotkey/floating toolbar, replacement or UI selection handoff. Clipboard import is not selection capture. |
| `Tools/TextTransforms.swift` | `bellobox-core::text` | Partial: nine case conversions, four encodings/manual decoders and auto detect, four hashes, six line operations, counts and heuristic tokens implemented/tests. GUI restores category bar and choice controls; hash/count specialized result cards still differ; Swift pretty auto-detection and per-model persisted token choices absent. Counts label Unicode scalars, not grapheme clusters. |
| `Tools/QRCodeGenerator.swift`, `UI/QRCodePopupView.swift` | `bellobox-core::qr`, GPUI image, CLI | Implemented bounded medium-correction QR generation, 4-module quiet zone, integer pixel modules, SVG/PNG export, real GUI preview/copy/save. No scanner-decoder roundtrip or macOS clipboard test yet. Live input recalc is wired. |
| `AI/AIClient.swift`, `AIConfig.swift`, `QuickAction.swift` | `bellobox-core::ai`, `bellobox-app::transport` | Partial: OpenAI Chat/Responses and Anthropic request builders, eight instructions, JSON-delimited selection, bounded incremental SSE, thinking exclusion, explicit streaming Send. HTTP timeout/no redirects; credentials runtime environment only. Request/SSE unit tests pass. No live-provider tests, model listing, model-scoped temperature/thinking controls, settings connection test, or replace-in-place. Closing a window invalidates its jobs and signals stream cancellation; blocking reads can remain until the next chunk or timeout. |
| `AI/CodexAppServerClient.swift`, `Tools/CodexCLI.swift` | None | Not ported: Codex app-server transport/model discovery. |
| `WorldClock/WorldClockModels.swift` | `bellobox-core::clock`, CLI and GPUI text summary | Partial: IANA zones, instant parsing, DST-aware calendar-day movement, working-hour quality, zone search; tests cover spring gap and 23-hour day. GUI lacks scrubber, live timer, saved zone controls, reference/location menus and native keyboard interactions. |
| `WorldClock/WorldClockCopilot.swift`, `WorldClockAIResolver.swift` | None | Not ported: copilot plan validation/apply, location mutations and shared ephemeral handoff. |
| `Screenshot/ScreenCaptureService.swift`, capture resolver/overlay | `bello-platform` subprocess adapter and `screenshot_ui::CaptureChooser` | Partial: explicit full-screen PNG via grim/ImageMagick on Linux or macOS screencapture. Editor capture uses private staging removed before returning in-memory bytes; no raw screenshot is published automatically. Separate source-sized capture chooser, PNG clipboard import and editor are wired. Native capture/clipboard UI has not been exercised. Area/window/scrolling/frozen displays/multi-display selection remain unavailable and labeled. |
| `Screenshot/AnnotationModel.swift`, `AnnotationRenderer.swift` | `bellobox-core::screenshot`; `screenshot_ui::ScreenshotEditor` | Implemented bounded pure model and raster pipeline with 34 renderer/tile tests and 25 overlay geometry tests: crop, vectors, highlights, explicit-font Unicode text, final opaque solid/stripe/dot masks, per-annotation eraser holes, move/select, 64-step/16 MB history and PNG export. Partial UI: source 1040×760 / min 640×440 layout, nine-tool strip, inline current-label editing/drag handle, committed text move/context-delete, opaque custom color wells, width/eraser sliders, mask swatches/menu, Fit/Fit Width/100%/steps, scrolling, Text Reader and export footer. Native-resolution preview tiles never exceed 1024 px per side; only intersecting tiles are painted, separate from full export. Pixel reconstruction tests include 40,000-pixel-tall captures. Native preview seams, pointer/keyboard behavior and layout are not runtime-verified. Custom colors use portable RGB/hex controls, not the native macOS color panel; its source color wheel/pipette modes remain absent. The Swift source has no committed-label reopen/edit gesture or popup crop-resize handles, and neither was invented. Continuous committed eraser preview, overlay capture editor and complete source interaction parity remain absent. Portable font metrics/pattern rasterization are not AppKit-identical. |
| `Screenshot/SelectionResizeGeometry.swift`, `ScreenshotPopupViewModel` overlay adjustment | `screenshot::selection` | Implemented pure eight-handle geometry, move/clamp, nonoverlapping dim bands, Cocoa/display-pixel conversion and explicitly enabled adjustment draft. Repeated updates commit as one undo step; canceled/stale drafts leave the document unchanged. Popup adjustment is disabled. Text-label hit frames and movement clamps preserve crop offsets. Overlay/native capture UI remains unported; this is model/test coverage only. |
| `Screenshot/ScrollCaptureEngine.swift`, `ImageStitcher.swift` | None | Not ported: manual/auto scrolling, overlap detection, fixed headers, capture notes and stitch UI. |
| `Screenshot/OCR/MacVisionOCRService.swift` | `bello-platform` Tesseract on Linux and native Apple Vision on macOS | Partial: local bounded image OCR, signature validation, in-memory image snapshot, no network. Tesseract synthetic runtime test passed. Native Vision is Apple-target type-checked, not runtime-tested; image/dimension/text limits apply. Screenshot Text Reader runs on a crop/mask-aware rendered PNG supplied directly as bytes. Mask/crop/undo cancels old jobs, clears reader content/undo and disables stale-copy payloads; revision/cancellation regression tests reject late OCR. Line boxes, language/settings wiring, Markdown formatting and original structured-region output remain absent. |
| `Screenshot/OCR/LLMOCRService.swift` and redaction-aware preprocessor | Sanitized image renderer only | Crop/redaction-aware image generation is implemented/tested, excluding all decorative annotations. Consent dialog, immutable provider/model/image approval snapshot, OCR tile segmentation, provider transport and Markdown output are not ported. AI OCR is disabled; no screenshot-to-provider route exists. |
| `Recording/RecordingEngine.swift`, coordinator/audio/input/privacy | None | Not ported: screen recording, audio mixing, cursor/click/key overlays, secure-field redaction, countdown, pause/review. |
| `Recording/GIF/GIFTranscoder.swift` | None | Not ported: movie/GIF conversion, transactional exports, precise frame timing and loop validation. |
| `Settings/AppSettings.swift`, `KeychainStore.swift` | Separate versioned Rust JSON settings; runtime AI environment | Partial: persisted appearance enum/IDs/provider metadata/zones, atomic private writes, corrupt-file preservation. No Swift migration or Keychain/Secret Service yet. No API key in settings. Source-shaped seven-page Settings now persists appearance, provider/request format/endpoint/model, system Prompt and learned-order reset. All open editors keep content/undo while ink changes. API keys remain runtime-only; inactive-provider drafts currently survive only within the Settings window. Unsupported source toggles remain disabled with explanations. New Settings controls are compile/unit-tested but not runtime-tested while the desktop is down. |
| `DeveloperTools/SnippetsAndGenerators.swift` | `settings::Snippets` | Partial: bounded explicit JSON store and literal field rendering core/tests; not wired to a snippet browser/save UI. No automatic migration or clipboard database. |
| `BelloBoxApp.swift` Sparkle updater | cfg-gated `macos_native::SparkleUpdater`; macOS bundling script | Partial, Apple-target type-checked only. Explicit GPUI update action, bundled-framework validation and main-thread retained controller implemented. No auto-download or update check from preview construction. Rust packaging requires an explicit Rust-specific feed; never defaults to the Swift production appcast. Framework2.8.1 supplied locally. Not signed/notarized/released. |
| `UI/MainView.swift`, onboarding/settings/menu bar | Source-shaped GPUI Home category sidebar/cards and separate windows | Partial: real native app window, process-local drafts, safe explicit clipboard operations and capability display. Settings category layout and working preference subset restored. Onboarding, menu-bar extra, setup guide, launch-at-login and native shortcuts remain absent. |
| `UI/Theme.swift`, `WindowMaterials.swift`, accessibility | Source light/dark GPUI tokens, vector badges and shipped icon | Partial: exact RGB tokens, source dimensions/spacing, plain wrapping editors, original Home navigation and separate QR/JSON/Text/AI structures. Native glass, SF Symbol exact rasterization, full original controls for every utility, native window materials, Reduce Motion/Transparency and full accessibility QA remain gaps. New Settings/dropdown/appearance changes still need desktop visual and interaction QA. |

## Developer tools

The baseline has exactly 51 developer tools (`Launcher/LauncherCatalog.swift`).
All 51 have executable, tested local implementations. Many older engines still
cover a subset of the original operations/dialects; this is not full tool parity.
Every implemented catalog example is exercised with its example options. 53 engine
unit tests cover malformed inputs, bounds, exact numbers, conversions and security
semantics. This is **51 functional engines, not 51 parity-complete tools**. They use
independent source-structured tool windows; specialized controls and visualizations
for many tools still need restoration. Sources below are in
`BelloBox/DeveloperTools/*.swift`.

| ID | Swift source | Rust subset / known gap |
|---|---|---|
| `json` | `DeveloperJSON.swift` | Lossless pretty/minify/validate; duplicate keys rejected |
| `compare` | `InspectionTools.swift` | Bounded exact line diff; JSON-specific semantic compare absent |
| `jwt` | `InspectionTools.swift` | Local header/claims inspection; signature never verified |
| `regex` | `InspectionTools.swift` | Linear-time pattern/groups/replacement with i/m/s; no ICU lookarounds/backreferences |
| `url` | `InspectionTools.swift` | URL inspection and ordered duplicate-preserving query edits |
| `time` | `DeveloperTime.swift` | Seconds/milliseconds/ISO, zones and differences |
| `cron` | `DeveloperTime.swift` | Five fields, next five UTC runs within366 days; timezone planner absent |
| `convert` | `DataConversion.swift` | JSON/YAML/CSV directions; YAML aliases/tags rejected |
| `snippets` | `SnippetsAndGenerators.swift` | Literal template substitution with date/UUID; save/browser UI absent |
| `http` | `HTTPRequestTool.swift` | Raw HTTP request inspection only; cURL and sending absent |
| `generate` | `SnippetsAndGenerators.swift` | UUID/UUID-derived hex/sample records; not a password generator |
| `calculator` | `MathUtilities.swift` | Bounded arithmetic and functions; radians |
| `units` | `MathUtilities.swift` | Documented unit families and temperature conversion |
| `numberBase` | `MathUtilities.swift` | Exact integer conversion up to 256 digits |
| `color` | `DesignUtilities.swift` | HEX/RGB/HSL conversions; visual picker absent |
| `contrast` | `DesignUtilities.swift` | Opaque-color WCAG contrast thresholds; visual sampler absent |
| `gradient` | `DesignUtilities.swift` | Two-stop 90-degree CSS; multi-stop editor absent |
| `markdown` | `StructuredUtilities.swift` | Safe local HTML output; rendered web preview absent |
| `jsonPointer` | `StructuredUtilities.swift` | RFC6901 lookup including URI fragments |
| `jsonFlatten` | `StructuredUtilities.swift` | Typed pointer flatten/unflatten preserves empty containers |
| `jsonCode` | `JSONCodeTool.swift` | TypeScript sample inference only; other languages absent |
| `sqlInsert` | `StructuredUtilities.swift` | Quoted PostgreSQL generation only; never executes |
| `xmlJSON` | `StructuredUtilities.swift` | Ordered XML-to-JSON, attributes/mixed text; reverse conversion absent |
| `unicode` | `TextUtilities.swift` | Codepoints/UTF8/UTF16; names/normalization absent |
| `stringEscape` | `TextUtilities.swift` | JSON/shell/HTML literals; never executes |
| `extract` | `TextUtilities.swift` | Heuristic unique URLs/emails; other extraction modes absent |
| `listSet` | `TextUtilities.swift` | Union/intersection/differences over exact lines |
| `semver` | `SecurityUtilities.swift` | SemVer comparison/sort; full range UI absent |
| `subnet` | `SecurityUtilities.swift` | IPv4/CIDR including /31,/32; IPv6 absent |
| `chmod` | `SecurityUtilities.swift` | Octal/rwx inspection; symbolic special-bit input absent |
| `hmac` | `SecurityUtilities.swift` | HMAC-SHA256, ephemeral key; other hashes and masked GUI key absent |
| `jsonSchema` | `JSONSchemaTool.swift` | Source keyword subset; exact decimal comparisons, code-point counts, local-reference preflight and 50,000-work/64-depth/100-issue limits. Paired document/schema editors restored; current tests cover source fixtures and adversarial references. |
| `jsonMerge` | `DataWorkshop.swift` | RFC7396 merge patch |
| `jsonRedact` | `DataWorkshop.swift` | Recursive configured field names; does not discover secrets |
| `jsonLines` | `DataWorkshop.swift` | NDJSON/array conversion, bounded 10,000 records |
| `csvExplore` | `DataWorkshop.swift` | Projection/filter/exact unique rows, full CSV export; graphical table absent |
| `envFile` | `ProtocolTools.swift` | Literal env/JSON conversion; no expansion or execution |
| `plist` | `PlistTool.swift` | Typed XML/JSON roundtrips preserve dictionaries, integer/real/date/data and reserved keys; strict XML characters/DTD/entity rejection. Source direction menu and Use-as-Input reversal restored. |
| `sqlFormat` | `SQLFormatterTool.swift` | Source bounded lexer/formatter with PostgreSQL/SQLite/MySQL and Uppercase/Preserve options. Token/literal/comment preservation and idempotence tests; never executes or validates queries. Source options menus restored. |
| `httpHeaders` | `ProtocolTools.swift` | Ordered duplicates preserved, offline |
| `cookies` | `ProtocolTools.swift` | Cookie/Set-Cookie inspection, offline |
| `certificate` | `CertificateTool.swift` | Offline public PEM inspection, at most 10 blocks/64 KB DER each, names/dates/serial/key-size/SHA-256/extensions. Source public certificate fixture tests; extra text/private keys/trailing DER rejected. No signature/trust/hostname/revocation/chain evaluation. |
| `sshKey` | `CertificateTool.swift` | Public RSA/Ed25519/ECDSA fingerprints; certificates/trust checks absent |
| `uuidInspect` | `ProtocolTools.swift` | UUID version/variant and supported timestamp fields |
| `bitwise` | `QuantTools.swift` | 64-bit integer operations; interactive word grid absent |
| `statistics` | `QuantTools.swift` | Numeric descriptive statistics; histogram absent |
| `dateMath` | `QuantTools.swift` | UTC date arithmetic/difference; zone-aware calendar controls absent |
| `aspectRatio` | `LayoutTools.swift` | Reduced ratios and scaled dimensions; diagram absent |
| `bezier` | `LayoutTools.swift` | Cubic CSS curve samples; draggable curve absent |
| `boxShadow` | `LayoutTools.swift` | CSS shadow text; raster visual preview absent |
| `textTable` | `LayoutTools.swift` | CSV to Markdown/plain tables; rich editor absent |

## Reproduce

```sh
cd rust
cargo test -p bellobox-core --no-default-features
cargo test -p bello-platform
cargo test -p bello-platform -- --ignored
cargo run -p bellobox-app --no-default-features -- --smoke
cargo run -p bellobox-app --no-default-features
# Full utilities enabled by default (implemented subsets):
cargo test -p bellobox-core
cargo run -p bellobox-app
```

Linux needs a supported X11/Wayland session and the native packages documented by
the shared workspace. GUI tests use isolated `BELLOBOX_CONFIG_DIR`,
`BELLOBOX_TOOL=qr`, and `BELLOBOX_INPUT=https://belloware.com`.
Optional `BELLO_PERF_LOG` writes JSONL CPU callback/startup timing only, never input
content. It is not GPU frame/presentation latency and is not a cross-app benchmark.

On macOS, `scripts/package-bellobox-macos.sh` builds a development `.app` only.
Supply an official Sparkle2.8.1 framework and a Rust-only appcast to test updating.
A macOS user still needs to compile, validate native APIs/permissions/entitlements,
and test signing/notarization; the Linux port cannot attest to those checks.

## Screenshot checkpoint verification limits

The new screenshot editor has compiled and passed source/lifecycle tests, but has
**never been launched for native visual QA** in this checkpoint: the desktop
connection and isolated display-server route are blocked. Earlier Home/QR
screenshots do not verify it. Use an explicitly chosen local synthetic PNG after
access returns, for example `BELLOBOX_TOOL=screenshot BELLOBOX_SCREENSHOT_FILE=/path/to/synthetic.png`.
The fixture loader is bounded and does not read files unless that variable is set.

Preview generation runs off the UI thread and coalesces edits; errors clear the
preview, never show an unmasked fallback. Its PNG tiles reconstruct exactly to
full rendered pixels in tests. Native filtering seams still require runtime QA.
Copy and Save independently render the current full-resolution snapshot; editor
mutations and closing are blocked while an export is pending to prevent newer
redactions racing an older save. Save uses private sibling staging, owner-only
files and atomic no-overwrite publication; choosing an existing filename returns
an error rather than the original app's replace workflow. Export errors and
invalid image bounds leave no partial destination. Full-image export and tile
PNG encoders are bounded and reject even a failed final PNG chunk.

## Label and color source follow-up

The toolbar now uses one 34-point custom color well for strokes/text, and the
source six mask presets plus a custom well and pattern icon menu. Color choices
stay opaque; RGB sliders and six-digit hex edit the current tool style without
applying it to unrelated committed annotations. Opening the well does not round
or change the stored color. The portable Colors panel preserves focus and blocks
underlying tool shortcuts while active. It does not claim native NSColorPanel
visual parity.

Committed text labels have the source interaction frame and right-click Delete
Label action. Select does not move arbitrary vector shapes. A drag starts after
2 view points and clamps the label inside the visible crop; erased holes move
with it. Transient raster snapshots show movement without mutating history; one
mouse release commits one undo step. The active inline label has its source
22-point drag handle. Closing while editing cancels that label and keeps the
editor open, following `requestClose()`; no existing-label edit/reopen gesture
was found in the Swift source or added. Tests cover color opacity and malformed
hex, zoom-scaled drag threshold, nontext rejection, transient snapshot isolation
and a single committed history step. All new native pointer/keyboard behavior
still awaits desktop verification.

## 2026-10-05 remote recovery baseline and converter controls

The fresh checkout started at verified remote `rust` commit
`3a91ece646f86d774891e02bc9de321ef8c2efa9`, with a clean working tree.
The reported recovery object `c05b553ed0f92938c5e80a358c974cc99b854e0d`
was not present in the fetched repository; no old workspace was overwritten.

Reproduced locally with Rust 1.99.0 and the checked-in lockfile:

- `cargo fmt --all -- --check`: passed before changes.
- `cargo test --locked -p bellobox-core -p bello-platform -p bello-workbench`:
  baseline 219 passing tests, four ignored, zero failures.
- `python3 -m unittest discover -s perf -p 'test_*.py' -v`: four passing tests.
- Full local GPUI build/desktop interaction remains blocked by missing native
  development packages. This Linux environment has no macOS SDK; all native
  macOS, Sparkle, original-app upgrade, and visual acceptance gates remain open.

JSON Lines, Environment File and Cookie Inspector now use the source `Convert`
and `Header` option menus instead of an option-string text editor. Initial
JSON-array, JSON-object and Cookie-header selections choose the corresponding
source mode; JSON Lines and Environment File reverse direction on Use as Input.
Source input labels are preserved. The engines and layouts otherwise retain
previously documented limitations.

The new `ui_tool_controls` integration target directly exercises the production
window's option state and existing utility engine without starting GPUI: four
new integration tests plus three existing control-state tests pass. It checks
both converter roundtrips, literal environment values, invalid-menu rejection,
independent-window state, mode-dependent source labels, and Cookie routing. These are **headless callback/state
tests, not desktop interaction or visual-parity evidence**. The new menus remain
implemented but visually unvalidated until a running desktop is inspected.
