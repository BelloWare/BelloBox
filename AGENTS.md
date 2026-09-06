# BelloBox Agent Guide

BelloBox is a macOS utility with a searchable command palette (⌃⌥⌘B), selection
suggestions, favorites, and recent commands. It opens with or without selected
text. A floating toolbar still appears when selecting text in another app:

- **AI**: a popup where a configurable AI rewrites, fixes, summarizes,
  translates, or answers about the text — then copy or replace in place. The AI
  backend is user-supplied: an OpenAI-compatible or Anthropic-compatible endpoint
  (with model listing from `/models`), or the local **Codex app-server**
  (`codex app-server`, no key needed).
- **QR code**: a popup with a live, scannable QR code of the selection and an
  editable text field; copy or save the image.
- **Text tools**: offline utilities — case conversion, encode/decode
  (auto-detecting Base64 / URL / HTML / Hex), pretty-print (auto-detecting JSON /
  XML / HTML / brace code), hashes (MD5 / SHA-1 / SHA-256 / SHA-512), line ops,
  and a Count tab with a model-aware token estimate.
- **Screenshot**: capture an area, window, screen, or scrolling page; annotate
  with pen, arrows, rectangles, highlights, text, crop, and blur/redaction;
  copy/save PNG; run local Mac OCR or explicit-confirmation LLM OCR.
- **Recording**: record an area, window, or screen with configurable audio,
  cursor, click, keystroke, privacy, countdown, and quality defaults.

- **All tools**: opens the palette with the captured selection. Developer tools
  include lossless JSON formatting, comparison, JWT inspection, regex, URL/query
  editing, timestamps, cron, JSON/YAML/CSV conversion, snippets, HTTP/cURL, and
  generators. World Clock remains available from the palette and menu.

Add commands in `Launcher/LauncherCatalog.swift`; developer tools share
`UtilityWorkbenchModel`/`UtilityWorkbenchView`, with engines in `DeveloperTools`.
Existing popup routes remain in `SelectionOverlayController.swift`. The global
shortcut reads AX selection without synthesizing copy; clipboard import is an
explicit palette action. Only tool IDs are stored in recents/favorites. Drafts
live for the palette session and pinned comparison text lives until quit.
The compact launcher uses a key-capable non-activating `LauncherPanel` and a
native `LauncherSearchField` with explicit first-responder focus. It dismisses
on outside clicks or loss of key focus, except for its own menus/sheets/children.
`LauncherSelectionContext` caches bounded preview and suggestion metadata once;
inputs over 500 KB are rejected before creating editors and never truncated.
`LauncherPreview` computes a bounded, read-only rich preview off the main actor
for the highest-ranked suggestion; `LauncherModel` caches it for the selection
and cancels/discards stale work on replacement or dismissal. Above 64 KB it
shows a compact notice rather than parsing. Previews never create workbenches,
send requests, copy text, or persist input. Timestamp selections rank World
Clock first; `LauncherModel` then installs a `WorldClockViewModel` in
`.preview` mode (no preference writes, up to four zones: saved order, then
local and UTC) rendered by `LauncherClockPreviewView`. That row is not a
button: the shared `MeetingTimelineView` scrubber (AppKit drag + horizontal
scroll wheel; vertical scrolls pass through), day arrows, reference menu, and
copilot own their input, while Enter opens World Clock with a
`WorldClockHandoff` (previewed instant, chosen reference, and an in-memory
copilot snapshot) via `LauncherCommandContext`; `WorldClockViewModel.adopt`
applies it without saving preferences and drops any earlier conversation in
the window. The palette resizes for the copilot transcript through
`onPreviewResize`, which the session fires after its state settled and which
never refocuses search; panel heights are clamped to the screen's visible
frame. SwiftUI's `ScrollView` is an `NSScrollView` that takes wheel events
before the hit-tested subview, so `TimelineScrubberView` claims horizontal
wheel events through a local monitor that exists only while it is in a
window and only for points inside its unclipped bounds (vertical and
outside events pass through; `BELLOBOX_E2E_WHEEL_DIAGNOSTICS=/path` logs
routing in DEBUG). `LauncherWindowController` leaves Enter/arrows to the
copilot field when it is first responder (`Esc` returns to search) and
`focusSearch` never steals focus from another text input. `←/→` (⌥ hour, ⇧ day) nudge the preview
only while the query is empty. Searching collapses the featured row; clearing
search restores it. Keep explicit query matches above suggestions and
suggestions above favorite/recent bonuses.

Normal launches and Dock/Finder reopens show `MainView`; only the shortcut and
explicit Search actions open the palette. `HomeCategory` organizes the tool
catalog and Home has ⌘1–⌘4 category shortcuts plus ⌘K search. Shared colors,
`ToolBadge`, `ShortcutBadge`, surfaces, and button styles live in `UI/Theme.swift`.
Use these tokens when adding or updating tools; honor Reduce Motion.
The palette follows the orange toolbox icon: `BoxTheme.accent` is adaptive
text/icon ink (burnt orange in light, peach in dark); use
`accentFill`/`accentGradient` behind white labels; `BoxTheme.brand` is the
icon's own orange for decorative washes and badges only, never text. Use the
semantic success/warning/danger colors for status text (warning stays golden so
it is never mistaken for the accent). `ThemeContrastTests` verifies small-text
contrast and the warm hues in both appearances.
World Clock uses the native search field for focused, keyboard-navigable location
search (⌘L). `WorldClockCopilotSession` (in `WorldClock/WorldClockCopilot.swift`)
is the ephemeral copilot shared by the window (⌘J) and the palette preview:
explicit Send only, run-ID guard against stale replies, cancel/retry, and a
missing-provider state. `WorldClockAIResolver` builds the planner-context prompt
and parses the JSON envelope; suggestions become a `WorldClockCopilotPlan` that
is validated against the current planner and applied only on the user's Apply
(the preview applies time only and shows location parts as "press Enter to
apply"; the window may add/replace locations and saves). Completion is tracked
per part (`WorldClockCopilotPlan.Parts`), so a mixed suggestion whose time was
applied in the palette still offers its locations in the window. Questions are bounded
by characters and UTF-8 bytes; a rejected question keeps the draft and clears
retry intent. Suggested instants must fall within Gregorian years 1–9999.
Never log prompts or replies. Review fixtures (DEBUG only): `BELLOBOX_E2E_WORLD_CLOCK_ZONES`
seeds isolated locations for both the window and the palette, and
`BELLOBOX_E2E_WORLD_CLOCK_COPILOT=scripted|error|slow` swaps in an offline
responder; neither enables a real provider.

It follows the same packaging conventions as the sibling Bello macOS apps
(BelloGesture, BelloWall, BelloTracker): xcodegen project, Developer-ID signed
DMG, Sparkle appcast, published to belloware.com.

## Project Structure

```
BelloBox/
├── project.yml                     # xcodegen project definition (source of truth)
├── BelloBox.xcodeproj              # generated by `xcodegen generate` (committed)
├── BelloBox/
│   ├── BelloBoxApp.swift           # @main App, MenuBarExtra, Sparkle updater, AppDelegate
│   ├── Info.plist                  # regular app + Sparkle SUFeedURL/SUPublicEDKey
│   ├── BelloBox.{Debug,Release}.entitlements   # sandbox disabled (needs Accessibility)
│   ├── Assets.xcassets/AppIcon.appiconset      # generated by scripts/generate-app-icons.swift
│   ├── AI/                         # Provider-agnostic AI layer
│   │   ├── AIConfig.swift          # ProviderKind, AIConfig, AIError
│   │   ├── AIClient.swift          # request building + SSE streaming (OpenAI + Anthropic)
│   │   ├── CodexAppServerClient.swift # local Codex app-server transport
│   │   └── QuickAction.swift       # one-click transformations + prompt builder
│   ├── Recording/                  # Screen recording, audio, overlays, privacy
│   ├── WorldClock/                 # Timeline/zone models, planner view model, copilot session + resolver
│   ├── Launcher/                   # Search palette, command catalog, shared workbench, clock preview
│   ├── DeveloperTools/             # Offline engines, snippet store, explicit HTTP client
│   ├── ThirdPartyNotices.txt       # Yams and LibYAML licenses (bundled resource)
│   ├── Screenshot/                 # Screenshot domain; keep separate from TextSelection
│   │   ├── ScreenshotModels.swift  # capture/document/scrolling models
│   │   ├── AnnotationModel.swift   # annotation value models
│   │   ├── ScreenCaptureService.swift       # ScreenCaptureKit capture
│   │   ├── RegionCaptureOverlayController.swift
│   │   ├── WindowCapturePicker.swift
│   │   ├── ScrollCaptureCoordinator.swift
│   │   ├── ImageStitcher.swift
│   │   ├── AnnotationRenderer.swift
│   │   ├── ImageExportService.swift
│   │   └── OCR/                    # Apple Vision + optional LLM OCR
│   │       ├── MacVisionOCRService.swift
│   │       ├── LLMOCRService.swift
│   │       └── AIImageClient.swift # multimodal request builders only
│   ├── Tools/
│   │   ├── QRCodeGenerator.swift   # CoreImage QR rendering (NSImage / PNG)
│   │   ├── TextTransforms.swift    # case / encode / decode / hash / lines / pretty-print
│   │   ├── TokenEstimator.swift    # model-aware token estimate (heuristic)
│   │   └── CodexCLI.swift          # locate the codex binary + preset models
│   ├── Settings/
│   │   ├── AppSettings.swift       # UserDefaults-backed config (ObservableObject)
│   │   └── KeychainStore.swift     # API keys stored in the Keychain
│   ├── Selection/
│   │   ├── AccessibilityService.swift   # AX read selection + bounds, ⌘C/⌘V helpers
│   │   └── SelectionMonitor.swift       # global mouse-up + hotkey monitors
│   └── UI/
│       ├── SelectionOverlayController.swift  # orchestrates monitor → toolbar → AI/QR popup; restarts monitors on grant
│       ├── FloatingPanel.swift               # non-activating NSPanel subclasses + placement
│       ├── FloatingButtonView.swift          # floating toolbar (AI, screenshot, recording, QR, text tools)
│       ├── ActionPopupView.swift             # the AI popup (SwiftUI)
│       ├── ActionPopupViewModel.swift        # runs AI actions, streams result
│       ├── QRCodePopupView.swift             # the QR popup + view model (editable text, copy/save)
│       ├── TextToolsPopupView.swift          # the text-tools popup + view model (inline token-model picker)
│       ├── ScreenshotCaptureChooserView.swift
│       ├── ScreenshotPopupView.swift          # annotation/OCR editor
│       ├── AnnotationCanvasView.swift
│       ├── OCRPanelView.swift
│       ├── OCRTextRegionsOverlayView.swift
│       ├── ScrollingCaptureHUDView.swift
│       ├── Theme.swift                       # design system: orange brand tokens, PopupHeader, popupCard, button styles, appear animation
│       ├── WorldClockView.swift              # dedicated World Clock window (planner, locations, copilot)
│       ├── WorldClockComponents.swift        # shared timeline scrubber, quality badge, icon buttons
│       ├── WorldClockCopilotView.swift       # copilot transcript/input, planner + compact styles
│       ├── MainView.swift                    # the home window (status, how-to, Settings / Updates)
│       ├── MainWindowController.swift         # hosts the home window (centered)
│       ├── ProviderConfigView.swift          # shared provider setup (3 providers, model load, say-hi test)
│       ├── OnboardingView.swift              # first-run flow (welcome → permission → provider → done); Skip allowed
│       ├── OnboardingWindowController.swift  # hosts onboarding in a window
│       └── SettingsView.swift                # provider/endpoint/key/model/prompt UI
├── BelloBoxTests/                  # unit tests (request building + SSE parsing)
└── scripts/
    ├── sparkle-release.sh          # build, sign, notarize, DMG, appcast
    ├── generate-dmg-background.swift
    ├── write-dmg-dsstore.py
    ├── generate-app-icons.swift
    └── run-tests.sh
```

## Conventions

- Bundle id: `com.ainoob.BelloBox`; Team: `43TXHV3TM3`.
- Regular app with a menu-bar extra and Dock presence for app windows. Not
  sandboxed — it reads the selection from other apps over the Accessibility API
  and pastes replacements.
- Keep developer utilities local, except explicit HTTP Send. cURL import never
  executes a shell or reads referenced files. Do not persist HTTP/JWT input or
  response history. HTTP redirects remain visible rather than auto-followed.
  YAML uses pinned Yams; retain third-party notices when updating it.
- Screenshot capture requires Screen Recording permission and uses
  ScreenCaptureKit as the primary path. Local annotation, stitching, and Mac OCR
  stay on-device.
- LLM OCR is the only approved screenshot-to-provider path. It must require
  explicit user confirmation and upload the crop/redaction-aware OCR image, not
  the raw screenshot. Do not log screenshots, Base64 image payloads, provider
  OCR responses, or copied OCR text.
- Do not thread screenshot/image state through `TextSelection`, QR, Text Tools,
  or the text-only AI action models. Keep image logic in `BelloBox/Screenshot`
  and `BelloBox/Screenshot/OCR`.
- Sparkle: shared EdDSA public key `slSJ7z2j8RDa266+E/7To5AOOloc2YtiMUZUVEIhwNA=`
  (private key lives in the login keychain, `acct=ed25519`,
  `svce=https://sparkle-project.org`). Feed: `https://belloware.com/assets/bello_box.appcast.xml`.
- First launch shows `OnboardingView`; it is reopenable from the menu bar
  ("Set Up Bello Box…").
- Gotcha: a global keyboard monitor only receives events once the process is
  Accessibility-trusted. `SelectionOverlayController` watches the trust state and
  calls `restartMonitors()` when access is granted while running, so the ⌃⌥⌘B
  hotkey works without relaunching.

## Building

```bash
xcodegen generate                  # regenerate BelloBox.xcodeproj from project.yml
xcodebuild build -project BelloBox.xcodeproj -scheme BelloBox -configuration Debug -destination 'platform=macOS'
```

Tests:

```bash
./scripts/run-tests.sh
```

## Release Process

1. Bump `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` in `project.yml`, then
   `xcodegen generate`.
2. Commit and push BelloBox to its upstream before publishing artifacts.
3. Smoke package without notarization: `NOTARIZE=0 ./scripts/sparkle-release.sh`.
4. Real release: `./scripts/sparkle-release.sh`
   (reads the Sparkle EdDSA secret from the login keychain; produces
   `dist/updates/BelloBox-<version>.dmg` and `dist/updates/bello_box.appcast.xml`).
5. Verify: `spctl -a -t open --context context:primary-signature -v dist/updates/BelloBox-<version>.dmg`.
6. Publish to the website:
   ```bash
   ditto dist/updates/BelloBox-<version>.dmg ../belloware.com/assets/BelloBox-<version>.dmg
   cp dist/updates/bello_box.appcast.xml ../belloware.com/assets/bello_box.appcast.xml
   ```
7. Update `../belloware.com/bello-box.html`. The page is intentionally **unlisted**:
   do NOT add it to `index.html` or `sitemap.xml` while the app is unpublished.
8. Commit and push `belloware.com`.
