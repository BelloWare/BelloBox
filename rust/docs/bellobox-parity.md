# BelloBox Rust parity ledger

Baseline: Swift commit `e43b1c4`. Rust lives under `rust/`; Swift code, project,
UserDefaults, Keychain entries, bundle identity, and `Snippets.json` are not migrated
or modified. This is an incremental, runnable port, **not feature parity**.

## Evidence and status vocabulary

- **Implemented** means executable code exists, with the cited checks.
- **Partial** means a useful subset exists; omitted behavior is listed.
- **Missing** means the Rust application does not implement the feature.
- **Validated** always names the specific tests or desktop interactions exercised;
  it never means whole-feature parity from backend tests alone.
- **Platform-blocked** is a verification qualifier, not an implementation status;
  it means validation requires a platform/session unavailable here.
- A catalog entry is not evidence that a feature works. Unsupported operations return
  explicit errors. The minimal `--no-default-features` build intentionally excludes
  all developer utility engines and labels them unavailable.

The earlier source-preserving checkpoint passed 140 core unit tests, 3 malformed-input
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
| `Launcher/LauncherCatalog.swift`, 61 commands including 51 developer tools | `bellobox-core::launcher`, source Home and separate GPUI palette | Partial: all IDs/titles, title-weighted search, basic selection suggestions, favorites/recents, coarse learning. Independent per-tool windows and New Window sessions are implemented. Separate 680px search palette has keyboard navigation and one expanded row. World Clock now has an offline interactive preview and explicit value-snapshot handoff; other previews remain read-only. Global shortcut/nonactivation, other tools’ interactive session transfer, and complete suggestion classifier remain absent. |
| `Launcher/LauncherUsageStore.swift` | `launcher::Usage`, `settings::Settings` | Implemented portable 30-day decaying, bounded category/tool counters. Tests cover cap/decay/title precedence. Only explicit tool opens learn. Selection, text, URL, app identity and fingerprints never persisted. |
| `Selection/AccessibilityService.swift`, `SelectionRequest.swift`, `SelectionMonitor.swift` | Explicit GPUI clipboard import; permission preflight | Partial: cfg-gated direct AXSelectedText reader with protected-ancestor/range/source/window checks and 160 ms budget, exposed through explicit macOS --selection CLI; Apple-target type-checked, not runtime-tested. No marker-range fallback, retries, global hotkey/floating toolbar, replacement or UI selection handoff. Clipboard import is not selection capture. |
| `Tools/TextTransforms.swift` | `bellobox-core::text` | Partial: nine case conversions, four encodings/manual decoders and auto detect, four hashes, six line operations, counts and heuristic tokens implemented/tests. GUI restores category bar and choice controls; hash/count specialized result cards still differ; Swift pretty auto-detection and per-model persisted token choices absent. Counts label Unicode scalars, not grapheme clusters. |
| `Tools/QRCodeGenerator.swift`, `UI/QRCodePopupView.swift` | `bellobox-core::qr`, GPUI image, CLI | Implemented bounded medium-correction QR generation, 4-module quiet zone, integer pixel modules, SVG/PNG export, real GUI preview/copy/save. No scanner-decoder roundtrip or macOS clipboard test yet. Live input recalc is wired. |
| `AI/AIClient.swift`, `AIConfig.swift`, `QuickAction.swift` | `bellobox-core::ai`, `bellobox-app::transport` | Partial: OpenAI Chat/Responses and Anthropic request builders, eight instructions, JSON-delimited selection, bounded incremental SSE, thinking exclusion, explicit streaming Send. HTTP timeout/no redirects; credentials runtime environment only. Request/SSE unit tests pass. No live-provider tests, model listing, model-scoped temperature/thinking controls, settings connection test, or replace-in-place. Closing a window invalidates its jobs and signals stream cancellation; blocking reads can remain until the next chunk or timeout. |
| `AI/CodexAppServerClient.swift`, `Tools/CodexCLI.swift` | None | Missing: Codex app-server transport/model discovery. |
| `WorldClock/WorldClockModels.swift`, `UI/WorldClockView.swift`, `WorldClockComponents.swift`, `WorldClockWindowController.swift` | `bellobox-core::clock`, `world_clock_ui`, dedicated GPUI window | Partial: source-shaped offline planner with live/Now, validated local date/time fields, retained reference-day timeline, continuous scrub, clamped 15-minute slider keys, horizontal-wheel day overflow, location/reference controls, alias search, explicit Copy Times and saved zone/reference preferences. Plain reopen reuses the window; closing starts a new live session next time. Offline launcher live/planned/reference snapshot adoption is implemented separately from ordinary reopen. Scoped Linux behavior/visual evidence is recorded below. Copilot and its conversation handoff, native localized pickers, floating/Spaces, window-frame persistence and native accessibility remain absent. |
| `WorldClock/WorldClockCopilot.swift`, `WorldClockAIResolver.swift` | None | Missing: copilot plan validation/apply, AI-requested location mutations and ephemeral conversation handoff. Offline clock-time/reference handoff is separate. |
| `Screenshot/ScreenCaptureService.swift`, capture resolver/overlay | `bello-platform` subprocess adapter and `screenshot_ui::CaptureChooser` | Partial: explicit full-screen PNG via grim/ImageMagick on Linux or macOS screencapture. Editor capture uses private staging removed before returning in-memory bytes; no raw screenshot is published automatically. Separate source-sized capture chooser, PNG clipboard import and editor are wired. Native capture/clipboard UI has not been exercised. Area/window/scrolling/frozen displays/multi-display selection remain unavailable and labeled. |
| `Screenshot/AnnotationModel.swift`, `AnnotationRenderer.swift` | `bellobox-core::screenshot`; `screenshot_ui::ScreenshotEditor` | Implemented bounded pure model and raster pipeline with 34 renderer/tile tests and 25 overlay geometry tests: crop, vectors, highlights, explicit-font Unicode text, final opaque solid/stripe/dot masks, per-annotation eraser holes, move/select, 64-step/16 MB history and PNG export. Partial UI: source 1040×760 / min 640×440 layout, nine-tool strip, inline current-label editing/drag handle, committed text move/context-delete, opaque custom color wells, width/eraser sliders, mask swatches/menu, Fit/Fit Width/100%/steps, scrolling, Text Reader and export footer. Native-resolution preview tiles never exceed 1024 px per side; only intersecting tiles are painted, separate from full export. Pixel reconstruction tests include 40,000-pixel-tall captures. Native preview seams, pointer/keyboard behavior and layout are not runtime-verified. Custom colors use portable RGB/hex controls, not the native macOS color panel; its source color wheel/pipette modes remain absent. The Swift source has no committed-label reopen/edit gesture or popup crop-resize handles, and neither was invented. Continuous committed eraser preview, overlay capture editor and complete source interaction parity remain absent. Portable font metrics/pattern rasterization are not AppKit-identical. |
| `Screenshot/SelectionResizeGeometry.swift`, `ScreenshotPopupViewModel` overlay adjustment | `screenshot::selection` | Implemented pure eight-handle geometry, move/clamp, nonoverlapping dim bands, Cocoa/display-pixel conversion and explicitly enabled adjustment draft. Repeated updates commit as one undo step; canceled/stale drafts leave the document unchanged. Popup adjustment is disabled. Text-label hit frames and movement clamps preserve crop offsets. Overlay/native capture UI remains unported; this is model/test coverage only. |
| `Screenshot/ScrollCaptureEngine.swift`, `ImageStitcher.swift` | None | Missing: manual/auto scrolling, overlap detection, fixed headers, capture notes and stitch UI. |
| `Screenshot/OCR/MacVisionOCRService.swift` | `bello-platform` Tesseract on Linux and native Apple Vision on macOS | Partial: local bounded image OCR, signature validation, in-memory image snapshot, no network. Tesseract synthetic runtime test passed. Native Vision is Apple-target type-checked, not runtime-tested; image/dimension/text limits apply. Screenshot Text Reader runs on a crop/mask-aware rendered PNG supplied directly as bytes. Mask/crop/undo cancels old jobs, clears reader content/undo and disables stale-copy payloads; revision/cancellation regression tests reject late OCR. Line boxes, language/settings wiring, Markdown formatting and original structured-region output remain absent. |
| `Screenshot/OCR/LLMOCRService.swift` and redaction-aware preprocessor | Sanitized image renderer only | Crop/redaction-aware image generation is implemented/tested, excluding all decorative annotations. Consent dialog, immutable provider/model/image approval snapshot, OCR tile segmentation, provider transport and Markdown output are not ported. AI OCR is disabled; no screenshot-to-provider route exists. |
| `Recording/RecordingEngine.swift`, coordinator/audio/input/privacy | None | Missing: screen recording, audio mixing, cursor/click/key overlays, secure-field redaction, countdown, pause/review. |
| `Recording/GIF/GIFTranscoder.swift` | None | Missing: movie/GIF conversion, transactional exports, precise frame timing and loop validation. |
| `Settings/AppSettings.swift`, `KeychainStore.swift` | Separate versioned Rust JSON settings; runtime AI environment | Partial: persisted appearance enum/IDs/provider metadata/zones, atomic private writes, corrupt-file preservation. No Swift migration or Keychain/Secret Service yet. No API key in settings. Source-shaped seven-page Settings now persists appearance, provider/request format/endpoint/model, system Prompt and learned-order reset. All open editors keep content/undo while ink changes. API keys remain runtime-only; inactive-provider drafts currently survive only within the Settings window. Unsupported source toggles remain disabled with explanations. New Settings controls are compile/unit-tested but not runtime-tested while the desktop is down. |
| `DeveloperTools/SnippetsAndGenerators.swift` | `settings::Snippets`, `snippet_library`, `desktop::snippets` | Partial: source-ordered find/name/library menu and explicit New/Save/Delete/load controls now use an isolated Rust store. Delete asks for confirmation; corrupt data blocks mutations; load/new clear field drafts; saves preserve IDs and reload disk to retain other windows’ saves. Seven state tests pass. Linux native create/update/find/load/New, delete cancel/confirm, persistence and one-item menu focus/dismissal passed the scoped QA recorded below; macOS and full visual parity remain unvalidated. Source custom-field Value rows and one-pass renderer are implemented with a stable per-tool UUID and fresh UTC time; their native QA is pending. Large field sets use an all-reachable virtual viewport instead of mounting all fields. Rust retains 500-item/512-byte-name limits and case-folded lexical ordering/filtering rather than source localized natural sorting. No Swift-library migration, shared live-window store notifications, or clipboard database. |
| `BelloBoxApp.swift` Sparkle updater | cfg-gated `macos_native::SparkleUpdater`; macOS bundling script | Partial, Apple-target type-checked only. Explicit GPUI update action, bundled-framework validation and main-thread retained controller implemented. No auto-download or update check from preview construction. Rust packaging requires an explicit Rust-specific feed; never defaults to the Swift production appcast. Framework2.8.1 supplied locally. Not signed/notarized/released. |
| `UI/MainView.swift`, onboarding/settings/menu bar | Source-shaped GPUI Home category sidebar/cards and separate windows | Partial: real native app window, process-local drafts, safe explicit clipboard operations and capability display. Settings category layout and working preference subset restored. Onboarding, menu-bar extra, setup guide, launch-at-login and native shortcuts remain absent. |
| `UI/Theme.swift`, `WindowMaterials.swift`, accessibility | Source light/dark GPUI tokens, vector badges and shipped icon | Partial: exact RGB tokens, source dimensions/spacing, plain wrapping editors, original Home navigation and separate QR/JSON/Text/AI structures. Native glass, SF Symbol exact rasterization, full original controls for every utility, native window materials, Reduce Motion/Transparency and full accessibility QA remain gaps. New Settings/dropdown/appearance changes still need desktop visual and interaction QA. |

## World Clock offline planner checkpoint — 2026-10-05

The World Clock route now opens its own 920×740 window (minimum 780×640), rather
than the generic text workbench. The source hierarchy is preserved: header,
scrollable planner and location cards, then a fixed footer. No new third-party
dependencies or shared-editor changes were introduced for this UI slice.

- Live refresh does not replace focused/composing fields or unchanged text.
  Temporary date/time text fields validate on Enter or blur; invalid/partial
  drafts do not change the instant. Tab/Shift-Tab traverse controls without
  inserting text, and picker focus stays contained until dismissal.
- The displayed core `Timeline` survives its next-midnight endpoint. Pointer
  scrubbing is continuous; slider keys clamp within that day; horizontal wheel
  steps may roll across days. Reference changes preserve the instant. Dates,
  transitions, location presentation and copy formatting use the core APIs.
- The 440px picker focuses its search, excludes existing locations, returns up to
  ten city/alias matches, wraps Up/Down and supports Enter/Escape/Cancel. At least
  one location remains. Fresh defaults use the resolved local IANA zone plus UTC;
  an explicitly saved UTC-only list stays UTC-only.
- Clock preference mutations save only location IDs/reference through the
  reload-and-patch helper; corrupt settings stay untouched. Planned instants,
  date/time drafts and searches are not saved. The existing generic explicit-open
  metadata save can still normalize unknown nested usage data; arbitrary JSON
  preservation is not claimed for the entire tool-opening path.
- Clipboard access is an explicit Copy Times write only. The planner never reads
  clipboard/selection implicitly and makes no provider request. Existing-window
  reopen retains the plan; closing releases the session and reopening starts live
  from saved locations/reference. Preview/handoff adoption was unported at this initial checkpoint; the bounded follow-up below adds the offline path.

Validation is deliberately split by immutable candidate:

- Functional Linux/X11 evidence used binary SHA-256
  `43945549e8ef03b6bd5d5e6cc2b5e6f405f1f023b4bb7302f015ac618740a3f4`
  (v8). Native checks covered modal Return/focus restoration without a second
  button activation, separate button Return acting once, exact 190-byte synthetic
  Copy Times output, minimum-size scrolling/readability, live minute refresh
  preserving a focused partial time draft, same-window repeat-open, and
  close/reopen with saved locations/reference and a fresh live session.
- The final v9 changed only the warning/purple quality ink to the exact adaptive
  Swift `Theme.swift` tokens and added an exact-token test. Its binary SHA-256 is
  `c0f2793fd14e9db5881a58e467b6d53c285df0347bb10e5f2dfe2498454aa79d`.
  Fresh light/dark captures at 920×740 and 780×640 were inspected; launch,
  keyboard-add, persistence/relaunch and clean close smoke checks passed.
  The v8 dark image is superseded as visual evidence, while its separate
  functional evidence is retained. See `ui-parity-review.md` for capture names.
- Final-source `scripts/check.sh` passed: 363 Rust tests, four existing ignored
  tests, formatting, strict all-target Clippy, workspace build and four performance
  harness tests. These are correctness/harness checks, not FPS or performance parity.

This is **not complete World Clock or macOS parity**. Formatting is deterministic
English/24-hour; fields are not native localized DatePickers. Copilot is visibly
unavailable, not redirected to another tool. The later follow-up below adds only offline launcher preview/handoff; native
accessibility labels/IME runtime validation, glass, floating/all-Spaces behavior,
saved native placement and complete native control equivalence remain gaps.
No macOS GUI execution,
OS IME test, provider call, signing, notarization or release is claimed here.

### Picker and reference-menu follow-up — 2026-10-05

A bounded follow-up restores the source clear-search control, keeps Add Location
visible but disabled when no valid result is selected, and completes ordinary
reference-menu dismissal and selected-item visibility. Clear returns focus to the
empty query and resets the first result/scroll without adding a location. The
empty Add state is guarded for pointer and keyboard activation and omitted from
Tab traversal. The reference menu consumes an outside dismissing click, exempts
its own toggle, restores logical trigger focus, and dismisses on activation loss.
Initial selection reveal runs after layout with a generation/weak-entity guard;
keyboard navigation then reveals the selected row, including wrapped endpoints.
The existing IME and paired Enter key-up protections remain in place.

The immutable Linux/X11 candidate was
`6f245fdc71b899a380a6891e99fee90aefb60034a3477b25b2ed77891e08facc`.
At 1180×812, synthetic fixtures verified: saved last-reference Vancouver visible
on the first opening of an 18-location menu; Down/Up wrap revealing UTC/Vancouver;
outside Now click dismissing without changing a planned 11:18 instant; toggle,
Escape, selection-focus restoration and window-deactivation dismissal; inert empty
Add; pointer/keyboard Clear restoring Los Angeles as first suggestion and query
focus without changing the location count; modal Tab/Enter adding London exactly
once; and clean close. Captures are `clock-picker-6f-first-reveal.png`,
`clock-picker-6f-disabled-add.png` and `clock-picker-6f-clear.png`.
These are new ordinary-path checks, not re-labeled evidence from the prior build.

Source review cleared the corrected first-layout behavior. Final aggregate gates
passed: 367 Rust tests with four existing ignored, formatting, strict all-target
Clippy, workspace build, four performance-harness tests and eleven Linux UI-harness
self-tests. No core, shared-editor, dependency or workflow changes were needed.
No OS IME runtime or macOS interaction/visual verification is claimed; the broader
native/Copilot gaps above remain open; offline handoff is addressed below.


### Offline launcher Clock preview and handoff — 2026-10-05

The clock row uses the source 261-point reservation and layout: status/reference
header, compact day controls and quality timeline, up to four location cards,
then the visibly unavailable compact Copilot region. It is an interactive planner,
not generic result text or a row-wide Open button. The existing 680-point palette
reserves the clock height synchronously and keeps the footer outside its scrolling
list. Preview controls use the same warm theme and quality tokens as the full clock.

A palette-owned session retains its seed, instant, reference and displayed day
across query/row navigation. Replacement input or Clear discards it; dismissal
releases it. The live timer runs only while the row is active. Pointer offsets are
continuous on the captured reference day; horizontal wheel uses accumulated
15-minute steps and permits day overflow, while vertical wheel scrolls the list.
With an empty search, Left/Right step 15 minutes, Alt steps one hour, and Shift
moves a reference-local calendar day. A nonempty query keeps its cursor arrows.
Menus own their keys, composition is checked before launcher routing, Tab visits
compact controls, and handled Enter releases cannot activate a restored control.

The source Clock handoff is a value snapshot, unlike the developer tools’
same-instance transfer. An explicit snapshot adopts the exact planned instant or
fresh live intent and chosen reference into a new or existing dedicated window.
It retains the full window's locations, appending only a missing reference in
memory. Ordinary reopen still preserves its existing plan. Preview creation,
scrubbing, reference changes and adoption never save clock preferences; explicit
open goes through the existing usage-accounting path once. No text, instant,
query, preview state, or clipboard content is persisted by this new path.

The parser remains the existing bounded Rust parser (zoned RFC3339 and integer
Unix seconds/milliseconds, at most 256 bytes for seed detection). Swift quoted,
local ISO, RFC2822/JavaScript, micro/nanosecond and fractional Unix inputs remain
outside this checkpoint. Existing 64 KB preview and 500 KB input limits remain.
Formatting is deterministic English/24-hour; Swift localized relative-time
subtitles are not reproduced. Copilot/provider/conversation transfer, native localized controls, native palette/nonactivation and macOS/OS IME
runtime equivalence remain unimplemented or unverified.

Independent source review cleared the bounded session/routing and final compact
icon delta. Final aggregate gates passed 389 Rust tests (four existing ignored),
formatting, strict workspace/all-target Clippy, workspace build, four performance
harness tests and eleven Linux UI-harness self-tests. No core, dependencies,
shared editor, platform or workflow changed.

Native Linux/X11 validation is split by immutable candidate:

- Functional candidate `f50527bc4eee1c05fe876221bab5c86f1c2b5c6d742e56dfe97e86527bab82c0`
  passed seed reset, 15-minute/hour/calendar-day keys, continuous endpoint drag,
  horizontal/vertical wheel routing, query cursor behavior, cached state across
  navigation/search, Clear replacement, live/Now, and exact new/existing-window
  handoff. Ordinary reopen reused an edited window and preserved NY 18:45;
  close/reopen created a fresh live NY-only session. Saved clock preferences
  remained NY-only throughout preview and adoption.
- Final candidate `82d2e2ca29c3aa0e60d4106b2cdd5861e9725218e1dcff6c1853d44e8f3106f2`
  changes only ordinary-control Escape routing and adds its regression test.
  Targeted native checks verified one Escape from timeline focus closes the
  palette, menu Escape only dismisses the menu, and the next Escape closes.
  Fresh light/dark captures at 680×663 show four readable cards from a five-zone
  saved fixture. Enter adopted the exact Oct 8 NY 10:15 instant into a full window
  retaining all five zones; the fixture's saved clock preferences stayed unchanged.
  Captures are `clock-launcher-82d-light-four.png` and
  `clock-launcher-82d-dark-four.png`. Test windows closed cleanly.

These ordinary Linux checks do not establish OS IME, macOS GUI, native panel or
complete launcher parity. Earlier full-window captures remain attributed to their
original binaries rather than relabeled as this checkpoint's evidence.

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
| `snippets` | `SnippetsAndGenerators.swift` | Partial: source placeholder grammar, one-pass missing-field-preserving substitution, selection/date/timestamp/UUID, per-tool UUID stability, dynamic field rows and isolated library controls. Renderer and cache-policy tests pass; large-field virtual viewport differs from Swift outer scrolling; latest field UI/macOS still need QA |
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
- Initial full local GPUI linking was blocked by missing unversioned linker
  aliases for preinstalled XCB/XKB runtime libraries. Workspace-only aliases and
  `LIBRARY_PATH` resolved it without changing system files. Full workspace tests,
  strict Clippy and build subsequently passed (270 tests, four ignored), including
  the concurrent shared CRLF editor regression slice. Three option-state tests
  are also compiled by the app target, so these are test executions, not a count
  of distinct behaviors. These are historical checkpoint counts, not totals for
  every later source slice.
- This Linux environment has no macOS SDK; all native macOS, Sparkle,
  original-app upgrade, and visual acceptance gates remain open.

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

## Snippet library controls checkpoint (2026-10-05)

Source: `UtilityWorkbenchView.swift` snippetControls/SnippetLibraryMenu,
`UtilityWorkbenchModel.swift` load/new/save/delete, and
`DeveloperTools/SnippetsAndGenerators.swift` SnippetStore. The implemented library
controls retain the two source rows and help text. Data stays under the existing
Rust config directory; original Swift Snippets.json remains untouched. Only
explicit Save/Delete actions write the Rust store, atomically and with private
permissions. Loading and New reset the template-fields draft. Failed writes keep
the persisted library and selected identity intact. Confirmation captures the
deleted ID, so a changed selection is not accidentally deleted. Store contents are
refreshed on menu open/load/mutation; no cross-process locking or live notification
is claimed. Existing Rust storage limits and locale/sorting differences remain
explicit in the row above. The source macOS alert is requested through GPUI's
platform prompt; platform-specific alert appearance is unverified.

Validated scope: seven isolated filesystem/state tests cover write-free mount and
New, identity-preserving updates, filtering, independent-window reload, corrupt
file/write-failure preservation, confirmed-ID deletion and Swift-file isolation/
Unix private permissions. This does not validate native menu/alert/pointer flows.
Template-field UI and rendering parity are separate incomplete work.

Manual Linux QA of the earlier db67901 source: JSON Lines conversion and reverse
conversion, malformed-input error, input editing, and close/reopen were exercised
in a live Linux SwiftShader session. This evidence does not validate the new
Snippets UI or macOS.

Checkpoint build validation: locked offline workspace tests passed (282 test
executions, four ignored, including concurrent shared-editor regressions); strict
workspace/all-target Clippy and full workspace build passed. Formatting and
diff whitespace checks passed. The seven snippet-library tests also passed with
no default features. Existing proc-macro-error2 dependency future-compatibility
warning remains; no macOS runtime test or snippet visual acceptance is implied.

## Native Snippets and final-window lifecycle QA (2026-10-05)

Validated on Linux software rendering, Snippets candidate SHA-256
`51448f9e85a01b88b1a6abcd257396e38ebc03170a278f95bd75b44d3bb0bac6`:
create/save, New clearing the draft and disabling empty Save, Find/load,
identity-preserving update with count staying at one, Delete Cancel preserving the
item, close/reopen loading the saved body, and confirmed Delete resetting count and
draft. A keyboard defect remains: opening the library menu from an unfocused tool
window does not route Return; mouse selection works. This is not full UI acceptance.

That candidate reproduced a GPUI X11 RefCell panic when the final window closed.
The separate lifecycle fix queues quit on the foreground executor, outside the
native close callback, and rechecks that no replacement window opened. Validated
candidate SHA-256 `44033f3e2df059938333d86b4b3d72a9b75133b9f5eafaf2ada3b57b3fda6e99`:
closing Snippets leaves Home responsive; closing the final Home exits cleanly;
relaunch and close of the sole Home window also exits cleanly. The old candidate
was reproduced immediately before this comparison. App tests (35), strict app
all-target Clippy, formatting and app build passed. macOS lifecycle remains untested.
The original macOS app stays running after its last window closes; Dock reopen and
menu integration are still missing. This fix addresses the Linux crash only and
does not establish macOS lifecycle parity.

The follow-up menu-focus fix gives the saved-snippet popup its own tracked focus,
restores previous focus only while the menu still owns it, and consumes saved
focus on every dismissal route, including outside clicks. Validated live on Linux
candidate `265d557110b4f208e61db9c598f7f89d08fc4531e4f031b963050e541bfc3391`:
unfocused tool reopen → menu → Return loads the item; Escape restores prior name
input so subsequent typing edits that name; clicking Find outside the menu sends
typing to Find without stealing focus; blank-area dismissal followed by Return /
Escape does not choose a hidden item. Repeated one-item menus passed. Two-item
arrow navigation and macOS menu behavior remain untested. App tests (35), strict
app all-target Clippy, formatting and build passed after the final focus guard.

## Source snippet fields and renderer checkpoint (2026-10-05)

Sources: `SnippetTemplate` in SnippetsAndGenerators.swift and the complete
UtilityWorkbenchModel.makeOperation call path. The GUI captures its original
selection and snippetUUID once per independent tool; each recomputation supplies
fresh UTC date/time. Generic CLI renderer calls generate a fresh UUID. Placeholder
grammar is exactly `[A-Za-z][A-Za-z0-9_ -]{0,59}` with first-seen unique fields,
case-sensitive names, built-ins taking precedence, unknown fields kept literally,
and no second evaluation of inserted text. Untouched missing fields differ from
explicitly cleared empty values. The old implicit Ada default is removed.

The GUI replaces the raw JSON options editor with “Fill template fields”, 130pt
labels and Value inputs after Template. Ordinary template edits retain field
values (including removing/re-adding names); New and loading a saved snippet reset
them. Values, selected text and UUID are never serialized to the library. The
source does not show Example on Snippets, so that invented action is removed.

Partial large-template layout: small field sets expand; more than eight rows use
a bounded virtualized field viewport, preserving access to every field rather
than silently truncating. Focused or composing rows are pinned visibly during the
list measurement phase,
before its viewport is computed, so their real keyboard/IME handlers remain
mounted. After composition finishes through the platform, click outside a Value
field to end focus and scroll freely. An outside click while the focused field
is composing is consumed without clearing its marked range or changing focus;
this is an explicit large-template interaction constraint, not source parity.
Outside clicks preserve the focus of any newly clicked input. Measurement callbacks
never evict editor entities; actual viewport reconciliation synchronizes values
before eviction. At checkpoint 4807399, evicted inactive fields retained their values but not
EditorView undo stacks. The bounded cold-state checkpoint below addresses
retention within an explicit memory/count limit. Small field sets and still-visible
editors retain their existing entities/undo.
A generation guard rejects queued events from
old fields after New/load. Small field sets use flexible, wrapping source rows;
large-list row heights reserve the measured label wrapping. This differs from the
source outer-scroll layout for large field sets. Long-label wrapping and offscreen IME behavior still need native
QA. The existing Rust 4MB rendered-output bound is retained and errors explicitly.

Validated: six deterministic renderer/session tests cover grammar/order/duplicates,
literal and empty substitution, UTC and pre-epoch timestamp truncation, stable
UUID/selection, resets, size bounds and 10,000 complete field names. Seven app
viewport/cache/state tests cover bounded height, 10,000-row traversal with focused/
composing retention, eviction and remove/re-add/reset value semantics. The
existing developer integration test now also rejects implicit Ada substitution.
After the pin/measurement and composition-guard fixes, locked offline workspace
tests passed 314
executions (four ignored, including concurrent Home/shared-editor/platform-fixture
tests); strict workspace/all-target Clippy, formatting and app build passed.
The core-only checkpoint also passed an independent workspace compile against
the published UI with no field-UI changes.

Validated on the earlier Linux field candidate dea2fafc: no invented Ada, live name
substitution, remove/re-add retaining values, stable UUID across edits, built-ins
not appearing as custom fields, spaced/hyphenated names, load/New resetting values.
A nine-field test reached and edited the last field and retained the first value
on return. That candidate is not evidence of the corrected pin/measurement flows;
the final viewport checks are recorded below; macOS remains untested. Actual
platform IME commit,
cancel and candidate-window behavior remains unvalidated; helper tests are not
evidence of native IME correctness. Shared compact inputs still
handle Tab as spaces rather than native next-field traversal; no complete native
TextField keyboard parity is claimed.

Final Linux candidate `8c6929fb86b9ce0d5ccda4eec1c03d355a5d15e889167fe2580518a25830275c`
passed ordinary native QA: active field stayed visible and accepted continued
typing under scroll attempts; clicking another field transferred focus without
misdirected text; label blur allowed reaching the last of twelve fields; returning
retained earlier values; a new edit after remount could be undone to its retained
text. This does not establish preservation of undo history across eviction. Exact
740×560 client-size testing reached the last field and Result after blur. A
49-character label wrapped over three lines with a usable Value input and live
result, without overlap; changing to a zero-field template removed all field rows.

Independent source re-review found the measurement/active-row fixes and final
composition guard correct within their stated constraints. No actual OS IME
commit, cancellation or candidate-window test was performed. These are scoped
Linux checks, not full native TextField, macOS or overall feature parity.

## Bounded snippet field edit-state retention (2026-10-05)

The shared, independently backed-up `EditorEditState` API is now used by the
Snippets field host. An inactive field moves its complete edit model into a
per-window cold pool when its render entity is evicted. The state preserves undo,
redo, pending edit groups, cursor and plain selection, without GPUI focus handles,
platform input handlers, composition or rendered geometry. Fresh views are fully
configured before restoration; restoration does not call set_text or mode setters
and does not emit a synthetic Changed event. The API rejects active or mismatched
state transfers without mutation.

The cold pool holds at most 64 states and 16 MiB of conservatively accounted state
bytes, using the shared accounting for model buffers, indexes and history
capacities. This is not measured allocator/RSS usage; bounded cache metadata and
still-mounted views are separate. Oldest inactive states are dropped when the
limit is reached; oversized states are not retained. Authoritative field values
remain in the session even when undo retention is pruned. Nothing is serialized
or written to settings, snippet files, logs or temporary files. Native cross-field
undo grouping/lifetime is not claimed equivalent to AppKit.

Session and per-mount generations reject stale editor callbacks, including an old
entity from the same field after remount. New/load clears both mounted and cold
state; a removed/re-added field can recover its retained state only in the same
session and while still cached. The exact current text/configuration is checked
before restoration. Focused, composing or dragging fields remain mounted using
the shared suspension blocker. Existing composition and scrolling constraints
remain; this checkpoint changes no Return, Tab, paste or newline behavior.

The virtual list sets both height and minimum height to at least twice its
measured row height, with no internal padding/border. Thus its actual visible
range always includes multiple rows; outer clipping does not change that layout
range in pinned GPUI 0.2.2. The ordinary 304px layout is unchanged; exceptionally
tall labels can enlarge this still-bounded viewport. A tall-row regression covers
the invariant.

Measurement returns only a correctly sized row after active-input pinning. It
never creates a measured-only editor, consumes cold state or updates recency;
only actual viewport reconciliation detaches editors. This avoids repeatedly
restoring/suspending row zero solely for layout measurement.

Seven new host regression tests cover the two-row viewport invariant, exact
byte/count boundaries, replacement
accounting, oversized rejection, opaque move identity, stale generations/mounts,
10,000 evictions preserving all values, removal/re-add/reset semantics and drag
protection. The shared API's separate tests exercise actual undo/redo/open-group
transfer with Unicode/CRLF and atomic rejection. Host cache unit tests do not
claim an end-to-end native editor or OS IME test. Locked offline workspace tests passed 325 executions with four ignored; strict
workspace/all-target Clippy, formatting and the full build passed after the
dependency cleanup checkpoint c1bb4c2. The final fields source was independently
reviewed at SHA-256 f48a902f2b515bdf0901ebdb3bfac3ec135738a2552b0c1df9cd02ed1cc17ff0.

Running Linux candidate
`04a80bd5aace1ce4b839b5e28af9a8010c4a0c211b79828181fd6ba0dee4d100` passed
fresh native checks: a nonzero-index field's edit made before eviction could be
undone immediately after remount; its redo stack survived a separate eviction and
remount. No new edit after remount was needed. Active-row pinning, continued typing
and drag-selection replacement passed. Removing/re-adding fields retained values;
New and loading a saved snippet cleared them. At exact740×560 client size, the
last of twelve fields remained reachable/editable; a long label wrapped over
three lines with a usable Value field and live result. Test windows closed
cleanly. These are ordinary Linux-path checks, not actual OS IME, native macOS
field-editor grouping or complete keyboard/paste parity.

## Shared workbench focused-text query (2026-10-06)

The shared `WorkbenchView::has_focused_editable_text` read-only query lets host
applications preserve source shortcut ownership without inspecting private editor
state. It requires the internal Editor panel, the editor's actual focus handle,
and its real non-read-only engine state. Changes/History panels can retain an old
editor focus handle; that does not count as visible editable text. Hosts must
also check their outer pane/tab visibility. The query does not focus an editor,
commit marked text, change content or enable a new Box command.

Focused shared validation passes: 101 tests across `bello-workbench` and
`bello-workbench-ui`, one intentionally ignored subprocess fixture, strict
all-target Clippy and formatting. The new policy test covers every internal-panel,
read-only and focus combination. Actual host routing and native keyboard behavior
require separate integration checks; these unit tests do not establish macOS
interaction or OS IME validation. No dependencies or release files changed.

## List Set Operations checkpoint (2026-10-06)

Sources: `DeveloperTools/AdditionalUtility.swift`, `TextUtilities.swift` and
`Launcher/AdditionalUtilityViews.swift`. The Rust tool now exposes the source
Operation menu (Union, Intersection, A − B, B − A, Symmetric difference) and Match
menu (Exact, Trim, Trim & ignore case). Results are plain lines in first-seen
order, rather than the previous sorted JSON bundle. Blank lines and duplicate
matching keys are removed; output retains the first matching item's spelling.
Canonical-equivalence keys preserve Swift String equality, including Exact mode.
The already-locked `unicode-normalization` package is now a direct core dependency;
no new package/version was introduced. Foundation's newline/whitespace sets include
U+200B for trimming and blank-line removal. Reviewed lowercase cases include
accented text, contextual Greek final sigma and dotted I; this does not establish
identical behavior across every OS/Unicode database version.

The two document drafts remain separate from options. Example resets both inputs
and options; Clear clears both documents. Each side has its own explicit Paste.
Equal editor heights use the source's bounded scalar-prefix/UTF-16 wrap formula.
A nonempty second list works when the first is empty. List Set does not offer the
non-source Use as Input action, and Copy refuses busy/error/empty results. Jobs
retain the existing window/generation fences. No clipboard read, persistence or
network action occurs during calculation. Original Swift files are unchanged.

The existing Rust limit remains 500,000 combined input bytes, whereas Swift allows
512,000 bytes per field. This is a bounded-port difference, not exact size parity.
Existing GPUI font, native text-field/IME and platform limitations remain; this
checkpoint does not establish complete source UI or macOS parity.

Focused validation: four engine tests cover all operations, matching, order,
duplicates, empty inputs, all Foundation line separators, canonical equivalence,
U+200B and input bounds. Nine headless control/height tests pass, including two new
List Set cases and seven existing option-state regressions. Strict all-target
Clippy for core/app, the app build, minimal-feature app check and formatting pass.

Independent Linux/SwiftShader native QA passed on immutable binary SHA-256
`a1e8cf7bf03d030489fb39242f4eeb06034469afc17212868c35f5c9f2724a8e`:
all five operation results, all three matching modes, Copy, Clear-both, each Paste
independently, calculation with blank A/nonempty B, readable controls/results at
740×560, and close/reopen resetting empty drafts and Union/Exact defaults. The
second close completed normally. This is scoped Linux interaction evidence, not
macOS, native IME, exhaustive clipboard/error-state or overall UI parity.
