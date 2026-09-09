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
  with pen, arrows, rectangles, highlights, text, crop, opaque masks (fill
  colour + solid/stripes/dots pattern), and a brush eraser that removes only
  the part of an annotation under it; copy/save PNG; run local Mac OCR or
  explicit-confirmation LLM OCR. The popup editor zooms (Fit, Fit Width,
  100%, steps) and scrolls; tall scrolling captures open at Fit Width.
- **Recording**: record an area, window, or screen with configurable audio,
  cursor, click, keystroke, privacy, countdown, and quality defaults, as a
  movie or a silent GIF (movie kept). **Video to GIF** converts a chosen movie
  locally with AVFoundation + ImageIO (`Recording/GIF/GIFTranscoder.swift`).

- **All tools**: opens the palette with the captured selection. Developer tools
  include lossless JSON formatting, comparison, JWT inspection, regex, URL/query
  editing, timestamps, cron, JSON/YAML/CSV conversion, snippets, HTTP/cURL, and
  generators. World Clock remains available from the palette and menu.

Add commands in `Launcher/LauncherCatalog.swift`; developer tools share
`UtilityWorkbenchModel`/`UtilityWorkbenchView`, with engines in `DeveloperTools`.
Existing popup routes remain in `SelectionOverlayController.swift`. The global
shortcut reads AX selection without synthesizing copy; clipboard import is an
explicit palette action. `SelectionRequest` gives an empty immediate AX read
three bounded retries (35, 65, 100 ms) before opening an empty palette, keeping
the source app in focus. Requests are cancelled by another shortcut, a new mouse
down, capture, or an app/window change; the mouse-up hint cannot race them.
`AccessibilityService` captures the source app/window together, reads its focused
element and at most six ancestors, rejects secure fields, and falls back from
selected text to the selected range's parameterized string or a validated UTF-16
slice of AXValue. Before treating an empty focused-control range as authoritative,
it reads `AXSelectedTextMarkerRange` when available: Chromium/Electron message
actions can retain keyboard focus while another element has the document's
selection. Marker types and noncollapsed endpoints are checked before requesting
`AXStringForTextMarkerRange`; both endpoint owners must belong to the captured
window and have an inspectable, unprotected ancestor path (16 elements and a
shared 160 ms validation budget). It never scans other fields or reads a whole
document as a fallback, and never treats a caret or a complete field as selected text.
Only tool IDs are stored in recents/favorites.
`LauncherUsageStore` separately learns explicit tool opens by coarse content
category (JSON, timestamp, URL, JWT, cron, data, cURL, text, or empty). It saves
only category/tool IDs, bounded weights, and last-use times; never selected
text, app identity, URLs, or fingerprints. Four uses can outweigh the default
suggestion; a 30-day half-life reduces old preferences. The bonus is capped
below explicit title matches. `LauncherModel` snapshots scores per selection
so arrow navigation/previews do not teach or reshuffle the current list; a
replacement selection refreshes them. Floating-toolbar opens teach too, without
double-counting palette routes. Rejected oversized inputs do not teach.
Settings → General can reset learned order. DEBUG fixtures and XCTest sessions
using standard defaults keep learning in memory outside all preference domains. Drafts
live for the palette session and pinned comparison text lives until quit.
The compact launcher uses a key-capable non-activating `LauncherPanel` and a
native `LauncherSearchField` with explicit first-responder focus. It dismisses
on outside clicks or loss of key focus, except for its own menus/sheets/children.
`LauncherSelectionContext` caches bounded preview and suggestion metadata once;
inputs over 500 KB are rejected before creating editors and never truncated.
The palette expands exactly one row: the focused command. Down/Up collapse
the previous row and show the newly focused command's preview, so what is
expanded is always what Enter opens; the "Best match" label stays on the top
suggestion whether or not it is focused, and a search focuses (and expands)
its top result while typing keeps working. Every command's expanded row is
interactive (`LauncherInteractivePreview` in `Launcher/LauncherPreviewSessions.swift`):
`LauncherModel.sessions` keeps one session per focused command for the
palette session (it survives arrowing and searching) and drops them all with
the selection. Developer tools reuse `UtilityWorkbenchModel` itself as the
session, so Enter opens the same instance and every option or draft edit
carries over; QR reuses `QRCodePopupViewModel`, Text Tools reuses
`TextToolsPopupViewModel`, World Clock the `.preview` planner, and the rest
have small session classes (AI prompt draft, capture permission/modes, a
session copy of the recording options, GIF options, app status).
`LauncherCommandContext` (`Launcher/LauncherHandoff.swift`) carries the draft
out of the palette: QR text, Text Tools category/option, an explicit
`AIHandoff`, a capture mode, recording/GIF options, a Settings category, or a
Home destination; `SelectionOverlayController.runLauncherCommand` routes it.
Preview buttons call `LauncherModel.open(_:customize:)` for a specific action.
Creating or focusing a session never records a use, saves preferences,
touches the clipboard, or sends anything; copy, save, Send (HTTP), an AI
action or prompt, Start Recording, Choose Movie and the system-permission
buttons are explicit user actions. Selections over 64 KB
(`LauncherSelectionContext.fitsPreviewLimit`) get no text session: the row
shows the compact notice with an explicit Open button, and Enter opens the
complete text. Views live in `LauncherToolPreviews.swift` (developer tools
on the workbench model) and `LauncherUtilityPreviews.swift`; shared pieces
(`LauncherOutputText`, a read-only native NSTextView that holds the complete
result; `LauncherPreviewField`, a literal single-line native field;
`LauncherPreviewEditor`; `LauncherChoiceBar`; chip button styles) are in
`LauncherPreviewComponents.swift`. `LauncherModel.previewHeight(for:)`
reserves each command's height synchronously (the developer tools, QR and
Text Tools share one height; measured by
`LauncherInteractivePreviewTests.testEveryInteractivePreviewFitsItsReservedHeight`).
`LauncherPreview.make(text:command:context:)` still computes a bounded,
read-only summary off the main actor (header title/subtitle, notices,
accessibility for rows without a session); `LauncherModel.previews` caches
one per focused command, work for a row that lost focus is cancelled and
discarded, and a new selection clears the cache. Row headers and
`LauncherInteractivePreview.accessibilitySummary` always describe the live
session, never the original selection. Drafts that grow past 64 KB inside a
session (a pasted second text, a loaded snippet, "Use as Input", editing in
the full tool then Back) show `LauncherDraftLimitNotice` instead of editors
or a stale result; `UtilityWorkbenchModel.previewsOnly` (true for a row,
false while the tool is open) makes `schedule()` skip such drafts, so the
row never parses them and the full tool calculates them on open; Back
restarts an interrupted calculation when the draft fits. Eligibility follows
the current draft: a tool that was opened keeps its session (notice or
preview) on the same model even when the original selection was too large.
The URL row edits the first 100 parameters (`LauncherURLParameterList.
compactLimit`, lazy rows, "Open for all" chip) while the model, the rebuilt
URL and Copy keep every parameter; the full workbench lists parameters in a
`LazyVStack`. Preview
inputs never claim focus when they mount (`focusesWhenAttached: false`);
while one is first responder, `LauncherWindowController` leaves Enter and the
arrows to it and Escape returns to search. A clicked read-only result is not
an input: arrows and Enter still navigate and open, Escape returns to search,
⌘C copies, and typing a character refocuses search before the key is
delivered. Tracked native menus (`isTrackingMenu`) and attached sheets own
the keyboard entirely and never dismiss the palette or open a tool. The
search field is one native field for the whole palette session: while a
tool is open `LauncherView` parks it (zero height, invisible, disabled so
Tab skips it, hidden from accessibility) instead of unmounting it, so
Escape, Back and ⌘K make it first responder synchronously; there is no
mount gap, no key buffering, and typing, paste, input methods and undo
continue natively while the tool fades out. Sheets from a preview (QR
Save…) attach to the palette panel through `model.hostWindow` so it does
not dismiss. The QR row (`LauncherQRPreview`) shows a real code on a 128 pt
card rendered at two pixels per point; a code whose modules would get fewer
than two pixels on a standard display is "dense" and its Enlarge chip grows
the card (192–384 pt, two pixels per module) and the row through
`onPreviewResize`, once per real height change, also when the text's
density changes while enlarged; copies and saves use
`QRCodeGenerator.exportPixelSize` (≥ 512 px and four pixels per module).
`LauncherInteractivePreviewTests` decodes the compact card, the enlarged
card and the export with `CIDetector`. World Clock is interactive for every
selection: timestamp selections rank it first and seed the planner
(`expandsClock` reserves the height synchronously); other selections get a
live planner (`isFollowingNow`, refreshed by the row's timer). Enter always
hands over what the row shows: `WorldClockHandoff.followsNow` carries live
intent, so `WorldClockViewModel.adopt` returns an already open window to
now (dropping its seed and conversation) instead of keeping an older plan,
while a plain reopen without a handoff keeps whatever the window showed.
`LauncherModel` installs the `WorldClockViewModel` in `.preview` mode (no
preference writes, up to four zones: saved order, then local and UTC)
rendered by `LauncherClockPreviewView`. That row is not a button: the shared
`MeetingTimelineView` scrubber (AppKit drag + horizontal scroll wheel;
vertical scrolls pass through), day arrows, reference menu, and copilot own
their input, while Enter opens World Clock with a `WorldClockHandoff`
(previewed instant or live intent, chosen reference, and an in-memory
copilot snapshot) via `LauncherCommandContext`; `adopt` applies it without
saving preferences. Height changes from focus moves, the copilot transcript
and the QR card go through `onPreviewResize`, which never refocuses search
and fires once per real change; panel heights are clamped to the screen's
visible frame. SwiftUI's `ScrollView` is an `NSScrollView` that takes wheel
events before the hit-tested subview, so `TimelineScrubberView` claims
horizontal wheel events through a local monitor that exists only while it
is in a window and only for points inside its unclipped bounds (vertical
and outside events pass through; `BELLOBOX_E2E_WHEEL_DIAGNOSTICS=/path` logs
routing in DEBUG); nested native scroll views (result text, editors)
receive the wheel normally. `focusSearch` never steals focus from another
text input. `←/→` (⌥ hour, ⇧ day) nudge the preview only while the query is
empty and the clock row is focused. Ask AI's row never sends: Enter (Open)
carries the prompt draft unsent (`AIHandoff.run == false`, kept even without
a provider or selection); only Send, Return inside the prompt field, or a
quick action (which needs selected text) hands over `run == true`, and the
popup starts that one request through `ActionPopupViewModel.apply`. Keep
explicit query matches above suggestions and suggestions above
favorite/recent bonuses. Review fixtures (DEBUG):
`BELLOBOX_E2E_LAUNCHER_TEXT`/`_TEXT_FILE` open the palette with a selection,
`BELLOBOX_E2E_LAUNCHER_FOCUS=<command id>` highlights that row's interactive
preview without opening it, and `BELLOBOX_E2E_LAUNCHER_COMMAND` opens a tool.
`SettingsWindowController.show(settings:category:)` and
`MainWindowController.show(..., category:)` steer an open window through
`WindowNavigation`.

Normal launches and Dock/Finder reopens show `MainView`; only the shortcut and
explicit Search actions open the palette. `HomeCategory` organizes the tool
catalog and Home has ⌘1–⌘4 category shortcuts plus ⌘K search. Shared colors,
`ToolBadge`, `ShortcutBadge`, surfaces, and button styles live in `UI/Theme.swift`.
Use these tokens when adding or updating tools; honor Reduce Motion.
Surfaces are quiet warm gray in light mode and graphite in dark mode. Tool badges
share the orange accent; semantic status colors stay distinct. `ToolChoiceBar`
serves compact previews and full tool tabs, `ToolTextFieldStyle` styles native
fields, and `ToolLinkButtonStyle` prevents macOS link buttons from reverting to
blue. Native controls also inherit `accentColor(accentFill)`. Text actions use
shared primary/secondary styles, square actions use 28 pt `ToolIconButtonStyle`,
and hover feedback respects Reduce Motion. Screenshot hosts own toolbar padding
once (44 pt overlay); popup toolbars and footers rearrange below 760 pt and remain
usable at 640 × 440, keeping the canvas and export actions on screen.
`AppBrandIcon` uses the shipped app artwork in Home, Settings, and Setup. Main,
World Clock, Settings, and Setup share `AppWindowChrome` and standard-sized native window controls;
World Clock remains floating across Spaces without the smaller utility style.
Ask AI centers in the selection's display (the pointer's display without a
selection), fits its visible frame, and scrolls its content on smaller screens.
Other unanchored tool popups center too; recording HUDs keep pointer-relative
placement. `ScreenPlacement.popupFrame` fits both size and position.
`ToolViewport` preserves usable layout dimensions and enables scrolling only
on overflowing axes; adaptive AI, QR, text and screenshot views absorb height
changes. Workspace controllers fit their full native frame (title bar included)
and recover off-screen windows on reopen. Popup minimize/restore retains the
first responder and honors Reduce Motion. `PopupChromeButton` is shared by
full and minimized headers. QR and developer editors focus their first input.
Returning from a workbench explicitly requests search focus; a pending request
survives until the newly mounted native search field is ready.
`LiteralTextEditor` uses a plain native NSTextView for utility, QR and text
inputs: smart quotes, dashes, text replacement and spell correction are off;
local typing keeps native undo, selection and input-method composition.
DEBUG `BELLOBOX_E2E_POPUP_VIEWPORT=width,height` reviews constrained tool panels;
`BELLOBOX_E2E_TEXT_TOOLS_TEXT` opens an isolated text editor fixture.
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
│   ├── Recording/                  # Screen recording, audio, overlays, privacy, GIF/ (transcoder, synthetic fixtures)
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
│       ├── ScrollCaptureHUDView.swift
│       ├── RecordingOptionsBar.swift / RecordingHUDView.swift / RecordingReviewView.swift
│       ├── GIFExportViews.swift              # GIF export panel, Video to GIF popup, animated GIF preview
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
- Annotations: `ScreenshotAnnotation.erasures` are per-annotation brush strokes
  in document pixels; `AnnotationRenderer` draws an erased annotation into a
  transparency layer clipped to `paintedRect(for:)` and punches the strokes out
  with destination-out, so the screenshot pixels, other annotations and later
  annotations are never affected. Erasures move with text labels and shift with
  crops (`ScreenshotAnnotation.offset`). One eraser drag = one undo step
  (`beginErasing`/`erase(toVisiblePoint:)`/`endErasing`, the mouse-up point
  included); nothing is ever deleted on the eraser's behalf, so paint outside
  the brush (thick-stroke edges, arrowheads, unbrushed mask) always survives.
  Masks (`AnnotationKind.blur`) keep `AnnotationStyle.maskFill` opaque whatever
  the picker sends; `MaskPattern` ink sits over the fill. Live canvas, export
  and OCR share `drawAnnotations`. `@Published` observers on the editor view
  model re-enter on every write: normalize with a guarded write-back, never an
  unconditional one (that recursion crashed the editor).
- The popup editor draws the base image through `ScreenshotBaseImageNSView`
  (`draw(_:)`, downsampled display copies), never as one layer texture: tall
  scrolling captures exceed GPU texture limits. `ZoomableCanvasScrollView`
  (an `NSScrollView`) applies `CanvasZoom` literally: Fit/Fit Width use the real
  clip size with legacy scrollers deducted, magnifications pass an explicit
  `renderScale` into `AnnotationCanvasView`/`ImageViewport(scale:)`, centring
  padding never changes magnification, and the laid-out scale is published as
  `displayedScale`. `ScreenshotDocument.captureNotes` carry stitch warnings
  into the editor banner (`CaptureNotesBanner`: two inline, "Show all" for the
  rest; the engine puts incomplete-capture notes first).
- Scrolling capture: the toolbar action is "Scrolling Capture"; the HUD shows
  the mode (manual vs auto-scroll), screens/pixels captured, frames against the
  limit, and messages such as a scroll jump without overlap. Review fixture
  `BELLOBOX_E2E_SCROLL_HUD_DEMO=full|compact` (DEBUG) samples a real scrollable
  window by rendering it, so the HUD, auto-scroll, Finish and Cancel work
  without Screen Recording.
- Recording GIF mode records the movie first; `RecordingCoordinator` converts
  it after `stop()` (`.convertingToGIF(progress:)`, cancel keeps the movie and
  reviews it with a note) and reviews `.reviewing(movie, gif:)`. `GIFTranscoder`
  decodes with `AVAssetReader`, orients frames with the preferred transform,
  writes through ImageIO to a staged sibling, validates the file reads back as
  a GIF with the planned frame count, then renames atomically; cancellation or
  failure removes the staged file and never touches the source. Options are
  bounded (`GIFExportOptions`: ≤120 s, ≤3000 frames, ≤1080 px, 5–20 fps). The
  standalone converter only takes files from the open panel. Timing: frame
  delays are a centisecond budget of the clip length (7, 6, 7… at 15 fps, the
  partial last frame included, none below 2 cs), so intended and encoded length
  differ by at most a centisecond. Repeat semantics follow browsers: a NETSCAPE
  block with 0 repeats forever, N means N extra plays, none means once. One-shot
  GIFs omit the loop property; `GIFLoopBlock.strip` can remove an unexpected
  repeat block structurally, and `validate` checks both modes. `AnimatedGIFNSView`
  applies the same rule, starts paused under Reduce Motion, and reloads on a
  `revision` change. The source can never be the destination (`isSameFile`
  resolves symlinks and file identity). Every job in the review and converter
  view models is generation-guarded; closing cancels it. DEBUG fixtures:
  `BELLOBOX_E2E_RECORDING_OPTIONS=1|gif`, `BELLOBOX_E2E_RECORDING_HUD=
  recording|paused|gif|hidden`, `BELLOBOX_E2E_CONVERTING_GIF=<progress>`,
  `BELLOBOX_E2E_RECORDING_REVIEW_FILE=<mov>` (+`_GIF`), `BELLOBOX_E2E_VIDEO_TO_
  GIF=1|<mov>`, and `BELLOBOX_E2E_WRITE_SYNTHETIC_ASSETS=<dir>` writes
  `tall-page.png`, `screenshot.png`, `sample.mov`, `sample-portrait.mov`
  (`SyntheticMediaFixture`) for those fixtures and for tests. Launch fixtures
  with `-hasCompletedSetup YES -floatingButtonEnabled NO -appearance light`
  process arguments and `BELLOBOX_DISABLE_KEYCHAIN=1`; never override `HOME` or
  `defaults write` the real bundle.
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
