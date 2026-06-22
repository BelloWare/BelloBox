# Codex Main Audit - BelloBox

Started: 2026-06-23 (Asia/Singapore)
Last updated: 2026-06-23 06:24 +0800

## Scope and Boundaries

- Source dirs: `BelloBox/AI`, `BelloBox/Recording`, `BelloBox/Screenshot`, `BelloBox/Selection`, `BelloBox/Settings`, `BelloBox/Tools`, `BelloBox/UI`, `BelloBox/BelloBoxApp.swift`, app plist/entitlements.
- Tests: `BelloBoxTests/*.swift`.
- Scripts: `scripts/*.sh`, `scripts/*.swift`, `scripts/*.py`, `scripts/assets/*`.
- Docs/config: `AGENTS.md`, `README.md`, `FEATURES.md`, `.gitignore`, `project.yml`.
- Entry points: `BelloBox/BelloBoxApp.swift`, `SelectionOverlayController.start()`, screenshot/recording coordinators, Sparkle release scripts.
- Build/test commands: `xcodegen generate`; `xcodebuild build -project BelloBox.xcodeproj -scheme BelloBox -configuration Debug -destination 'platform=macOS'`; `xcodebuild build -project BelloBox.xcodeproj -scheme BelloBox -configuration Release -destination 'platform=macOS'`; `./scripts/run-tests.sh`; `./scripts/run-e2e.sh`.
- CI/docs discovered: no `.github` workflow files in this repo; local release flow documented in `AGENTS.md` and scripts.
- Excluded dirs/files: `.git/`, `.claude/`, `dist/` release artifacts, build/DerivedData outputs, generated `BelloBox.xcodeproj/**`, static generated app icon PNGs unless referenced by code/config.
- Chrome/browser-service Bello focus bullets: not applicable after repo search; no BrowserService, RuntimeTabBinding, dedicated Chrome bridge, or workspace layout code exists in BelloBox.

## Status Legend

- `reviewed/no issue`: reviewed; no definite issue found.
- `reviewed/changed`: reviewed and changed for a confirmed issue or safe refactor.
- `reviewed/speculative`: reviewed; suspicious item recorded but not edited because evidence was insufficient.
- `excluded`: generated/vendor/build/static artifact or otherwise outside first-party code audit; accounted for.

## Workstream Results

- Main: manifest, triage, final edits, verification, ledger integration.
- Halley (`019ef15c-bb3c-7ad3-ada7-64c813c49298`): source-domain correctness review completed; reported recording pause timeline and OCR cancellation candidates plus speculative reserved APIs.
- Hume (`019ef15c-bbd4-7700-b862-83019dab0fb4`): UI/app shell review completed; reported chooser auto-start, empty text undo, and dead UI state candidates. Dock/accessory concern resolved as stale docs/current product behavior, not code change.
- Rawls (`019ef15c-bc42-7092-b868-b12191320313`): fallback/legacy/hack review completed; no definite deletion candidates, one discard-error masking bug fixed.
- Faraday (`019ef15c-c502-7de3-b22d-138bee8cc774`): tests/scripts/docs/config review completed; docs and release temp-path issues fixed.

## Triage Log

| Status | File | Symbol | Problem | Why definite | Invariant | Minimal fix | Verification | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| fixed | BelloBox/Recording/RecordingEngine.swift | RecordingPauseTimeline / appendVideo / appendAudio | Paused samples were dropped but resumed media kept original source timestamps, so paused wall-clock time leaked into exported duration. | Code path used source PTS for resumed appends while RecordingCoordinator excludes paused time from elapsed runtime. | Paused time must not appear in the exported recording timeline; audio and video must stay aligned. | Track accumulated pause duration and subtract it from video append PTS and audio sample timing. | ./scripts/run-tests.sh; RecordingPauseTimelineTests | medium |
| fixed | BelloBox/Screenshot/OCR/LLMOCRService.swift | recognize | Hybrid local OCR used try? and swallowed CancellationError before external upload preparation. | try? converts cancellation to nil, then code prepares upload image and calls provider client. | Cancelling OCR must not continue into screenshot upload preparation or provider request. | Rethrow CancellationError, check cancellation before preparing/uploading, allow non-cancellation local OCR failures to remain optional hints. | ./scripts/run-tests.sh; testHybridLocalOCRCancellationDoesNotUploadImage | high |
| fixed | BelloBox/Screenshot/OCR/MacVisionOCRService.swift | performVisionOCR | Detached Vision OCR task was not cancelled when parent task was cancelled. | Task.detached handle was awaited without a cancellation handler. | Closing/cancelling OCR should stop outstanding Vision work as promptly as possible. | Wrap detached task with cancellation handler, cancel task and VNRequest, check cancellation around Vision perform. | ./scripts/run-tests.sh | medium |
| fixed | BelloBox/UI/RecordingReviewView.swift | discard | Discard ignored file deletion errors and closed review anyway. | try? removeItem always followed by onClose. | Failed discard must leave review open and show a copyable error. | Inject remove closure for testability, surface delete error, close only after successful delete. | ./scripts/run-tests.sh; RecordingReviewViewModelTests | low |
| fixed | BelloBox/UI/ScreenshotPopupView.swift | endTextEditing | Committing an empty text annotation removed the annotation but left an undo entry for no visible change. | beginTextAnnotation pushed undo; endTextEditing removed empty annotation without collapsing matching undo state. | Undo entries should correspond to visible document mutations. | Collapse undo entry after empty text removal and clear redo like cancel path. | ./scripts/run-tests.sh; InlineTextEditingTests | low |
| fixed | BelloBox/UI/ScreenshotCaptureChooserView.swift | initialMode auto-start | Delayed initial capture used non-cancellable DispatchQueue.asyncAfter and could fire after chooser dismissal. | Closure had no identity or lifecycle guard. | Dismissed/replaced chooser must not start a capture later. | Store a cancellable task and cancel it on disappear. | ./scripts/run-tests.sh | medium |
| fixed | BelloBox/UI/ScreenshotCaptureChooserView.swift | isCapturing | isCapturing was read in disabled state but never written. | rg found declaration/read only. | View model state should represent real behavior. | Remove dead state and disabled dependency. | ./scripts/run-tests.sh | low |
| fixed | BelloBox/UI/OCRPanelView.swift | selectedRegionID | selectedRegionID was never read or written. | rg found declaration only. | Dead published state should not imply unsupported selection behavior. | Remove dead state. | ./scripts/run-tests.sh | low |
| fixed | scripts/sparkle-release.sh | DMG temp paths | Release script used mktemp -u and created temp paths before cleanup trap. | mktemp -u does not reserve a path; an exit before trap could leak temps. | Release packaging temp names should be reserved and cleanup should cover created temps. | Validate icon before temps, use mktemp -d for RW DMG directory, clean directory in trap. | bash -n scripts/*.sh | low |
| fixed | AGENTS.md / FEATURES.md | docs | First-party docs still referenced codex exec/accessory-only/AI+QR toolbar and omitted recording menu/toolbar details. | Code and README use codex app-server, regular Dock presence, and recording UI. | Repo docs should describe the shipped app accurately for future agents and QA. | Update docs to app-server, current Dock/menu-bar behavior, full toolbar, and recording feature. | rg for stale phrases | low |

## Verification Log

| Command | Result | Notes |
| --- | --- | --- |
| `./scripts/run-tests.sh` | pass | Baseline before edits: 223 tests, 1 skipped, 0 failures (2026-06-23 06:05 +0800). |
| `bash -n scripts/request-e2e-permissions.sh scripts/run-capture-recording-e2e.sh scripts/run-e2e.sh scripts/run-hotkey-e2e.sh scripts/run-tests.sh scripts/sparkle-release.sh` | pass | Baseline shell syntax check for first-party bash scripts. |
| `./scripts/run-tests.sh` | pass | After first patch set: 227 tests, 1 skipped, 0 failures (2026-06-23 06:16 +0800). |
| `xcodegen generate` | pass | Regenerated project after adding RecordingPauseTimelineTests. |
| `./scripts/run-tests.sh` | pass | After recording pause fix: 229 tests, 1 skipped, 0 failures (2026-06-23 06:17 +0800). |
| `bash -n scripts/request-e2e-permissions.sh scripts/run-capture-recording-e2e.sh scripts/run-e2e.sh scripts/run-hotkey-e2e.sh scripts/run-tests.sh scripts/sparkle-release.sh` | pass | Post-edit shell syntax check. |
| `python3 ast parse scripts/write-dmg-dsstore.py` | pass | Post-edit Python syntax check. |
| `rg BrowserService/Chrome-specific Bello focus terms` | pass | No matching files; Chrome/browser-service bullets not applicable to this repo. |
| `xcodebuild build -project BelloBox.xcodeproj -scheme BelloBox -configuration Debug -destination 'platform=macOS'` | pass | Explicit Debug app build succeeded. |
| `xcodebuild build -project BelloBox.xcodeproj -scheme BelloBox -configuration Release -destination 'platform=macOS'` | pass | Explicit Release app build succeeded. |
| `./scripts/run-e2e.sh` | pass | Permissions granted; hotkey E2E passed; real screenshot/capture-overlay screenshot/real recording E2E passed. |
| `./scripts/run-tests.sh` | pass | Final suite after pause timeline edge-case refinement: 230 tests, 1 skipped, 0 failures (2026-06-23 06:23 +0800). |
| `xcodebuild build -project BelloBox.xcodeproj -scheme BelloBox -configuration Debug -destination 'platform=macOS'` | pass | Final Debug build succeeded. |
| `xcodebuild build -project BelloBox.xcodeproj -scheme BelloBox -configuration Release -destination 'platform=macOS'` | pass | Final Release build succeeded. |
| `./scripts/run-e2e.sh` | pass | Final e2e passed: permission check, toolbar hotkey, screenshot hotkey, recording hotkey, real screenshot, capture overlay screenshot, real recording. |

## Complete File Manifest

| Status | Path | Owner | Notes |
| --- | --- | --- | --- |
| `reviewed/no issue` | `.gitignore` | docs-config | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `AGENTS.md` | docs-config | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `excluded` | `BelloBox.xcodeproj/project.pbxproj` | generated | generated by xcodegen from project.yml; regenerated after adding test file |
| `excluded` | `BelloBox.xcodeproj/project.xcworkspace/contents.xcworkspacedata` | generated | generated by xcodegen from project.yml; regenerated after adding test file |
| `excluded` | `BelloBox.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved` | generated | generated by xcodegen from project.yml; regenerated after adding test file |
| `excluded` | `BelloBox.xcodeproj/xcshareddata/xcschemes/BelloBox.xcscheme` | generated | generated by xcodegen from project.yml; regenerated after adding test file |
| `reviewed/no issue` | `BelloBox/AI/AIClient.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/speculative` | `BelloBox/AI/AIConfig.swift` | source-domain | reviewed; suspicious/reserved API noted, no definite change made |
| `reviewed/no issue` | `BelloBox/AI/CodexAppServerClient.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/AI/QuickAction.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Assets.xcassets/AppIcon.appiconset/Contents.json` | assets/config | reviewed by main/subagent pass; no definite issue found |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_128x128.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_128x128@2x.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_16x16.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_16x16@2x.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_256x256.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_256x256@2x.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_32x32.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_32x32@2x.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_512x512.png` | assets/config | static/generated binary asset; references/config audited |
| `excluded` | `BelloBox/Assets.xcassets/AppIcon.appiconset/icon_512x512@2x.png` | assets/config | static/generated binary asset; references/config audited |
| `reviewed/no issue` | `BelloBox/Assets.xcassets/Contents.json` | assets/config | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/BelloBox.Debug.entitlements` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/BelloBox.Release.entitlements` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/BelloBoxApp.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Info.plist` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Recording/Input/InputMonitoringPermission.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Recording/Input/InputOverlayModels.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Recording/Input/RecordingInputMonitor.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Recording/Privacy/PasswordFieldDetector.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/speculative` | `BelloBox/Recording/Privacy/PrivacyGuard.swift` | source-domain | reviewed; suspicious/reserved API noted, no definite change made |
| `reviewed/no issue` | `BelloBox/Recording/RecordingAudioMixer.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Recording/RecordingCoordinator.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `BelloBox/Recording/RecordingEngine.swift` | source-domain | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `BelloBox/Recording/RecordingFrameRenderer.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Recording/RecordingMicrophoneDevices.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Recording/RecordingModels.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Recording/RecordingPermission.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/AnnotationModel.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/AnnotationRenderer.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/CaptureSelectionResolver.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/CaptureWindowCatalog.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/ImageExportService.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/ImageStitcher.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/OCR/AIImageClient.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `BelloBox/Screenshot/OCR/LLMOCRService.swift` | source-domain | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/changed` | `BelloBox/Screenshot/OCR/MacVisionOCRService.swift` | source-domain | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `BelloBox/Screenshot/OCR/OCRBoundingBoxConverter.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/OCR/OCRImagePreprocessor.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/speculative` | `BelloBox/Screenshot/OCR/OCRModels.swift` | source-domain | reviewed; suspicious/reserved API noted, no definite change made |
| `reviewed/no issue` | `BelloBox/Screenshot/OCR/OCRPromptTemplates.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/OCR/OCRResultFormatter.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/OCR/OCRService.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/OCR/OCRTileSegmenter.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/RegionCaptureGeometry.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/RegionCaptureOverlayController.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/ScreenCaptureFrameGrabber.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/ScreenCapturePermission.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/ScreenCaptureService.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/speculative` | `BelloBox/Screenshot/ScreenCoordinateSpace.swift` | source-domain | reviewed; suspicious/reserved API noted, no definite change made |
| `reviewed/speculative` | `BelloBox/Screenshot/ScreenshotModels.swift` | source-domain | reviewed; suspicious/reserved API noted, no definite change made |
| `reviewed/no issue` | `BelloBox/Screenshot/ScrollCaptureCoordinator.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Screenshot/WindowCapturePicker.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Selection/AccessibilityService.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Selection/SelectionMonitor.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/speculative` | `BelloBox/Settings/AppSettings.swift` | source-domain | reviewed; suspicious/reserved API noted, no definite change made |
| `reviewed/no issue` | `BelloBox/Settings/GlobalHotkey.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Settings/KeychainStore.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/Settings/LaunchAtLoginController.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/speculative` | `BelloBox/Tools/CodexCLI.swift` | source-domain | reviewed; suspicious/reserved API noted, no definite change made |
| `reviewed/no issue` | `BelloBox/Tools/QRCodeGenerator.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/speculative` | `BelloBox/Tools/TextTransforms.swift` | source-domain | reviewed; suspicious/reserved API noted, no definite change made |
| `reviewed/no issue` | `BelloBox/Tools/TokenEstimator.swift` | source-domain | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/ActionPopupView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/ActionPopupViewModel.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/AnnotationCanvasView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/AnnotationToolbar.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/AppActivation.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/AudioSourcePickerView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/CaptureOverlayController.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/FloatingButtonView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/FloatingPanel.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/HotkeyRecorderView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/MainView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/MainWindowController.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `BelloBox/UI/OCRPanelView.swift` | ui-app | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `BelloBox/UI/OCRTextRegionsOverlayView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/OnboardingView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/OnboardingWindowController.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/ProviderConfigView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/QRCodePopupView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/RecordingHUDView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/RecordingOptionsBar.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/RecordingPermissionView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `BelloBox/UI/RecordingReviewView.swift` | ui-app | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/changed` | `BelloBox/UI/ScreenshotCaptureChooserView.swift` | ui-app | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `BelloBox/UI/ScreenshotOverlayEditorController.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `BelloBox/UI/ScreenshotPopupView.swift` | ui-app | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `BelloBox/UI/ScrollingCaptureHUDView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/SelectionOverlayController.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/SettingsView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/SettingsWindowController.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/TextToolsPopupView.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBox/UI/Theme.swift` | ui-app | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/AIClientTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/ActionPopupViewModelTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/AnnotationRendererTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/CaptureOverlayAccessoryLayoutTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/CaptureSelectionResolverTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/GlobalHotkeyTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/ImageExportServiceTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/ImageStitcherTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `BelloBoxTests/InlineTextEditingTests.swift` | tests | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/changed` | `BelloBoxTests/LLMOCRRequestBuilderTests.swift` | tests | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `BelloBoxTests/LLMOCRResponseParserTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/OCRBoundingBoxConversionTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/OCRBoundingBoxConverterTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/OCRImagePreprocessorTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/OCRResultFormatterTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/OCRTileSegmenterTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/PrivacyGuardTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/QRCodeGeneratorTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/RecordingAudioMixerTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/RecordingCoordinatorTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/RecordingEngineE2ETests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/RecordingFrameRendererTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/RecordingOptionsTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/RecordingOverlayEventStoreTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `BelloBoxTests/RecordingPauseTimelineTests.swift` | tests | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/changed` | `BelloBoxTests/RecordingReviewViewModelTests.swift` | tests | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `BelloBoxTests/RegionCaptureGeometryTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/ScreenCoordinateSpaceTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/ScreenPlacementTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/ScreenshotPopupViewModelOCRTaskTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/ScreenshotTestHelpers.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/ScrollCaptureCoordinatorTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/SnapshotDocumentTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/TextTransformsTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `BelloBoxTests/WindowCapturePickerViewModelTests.swift` | tests | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `FEATURES.md` | docs-config | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `README.md` | docs-config | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `project.yml` | docs-config | reviewed by main/subagent pass; no definite issue found |
| `excluded` | `scripts/assets/bellobox-icon-source.png` | scripts | static/generated binary asset; references/config audited |
| `reviewed/no issue` | `scripts/e2e-recording-fixture.swift` | scripts | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `scripts/generate-app-icons.swift` | scripts | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `scripts/generate-dmg-background.swift` | scripts | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `scripts/request-e2e-permissions.sh` | scripts | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `scripts/run-capture-recording-e2e.sh` | scripts | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `scripts/run-e2e.sh` | scripts | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `scripts/run-hotkey-e2e.sh` | scripts | reviewed by main/subagent pass; no definite issue found |
| `reviewed/no issue` | `scripts/run-tests.sh` | scripts | reviewed by main/subagent pass; no definite issue found |
| `reviewed/changed` | `scripts/sparkle-release.sh` | scripts | changed for confirmed issue, dead state removal, or obsolete docs/scripts correction |
| `reviewed/no issue` | `scripts/write-dmg-dsstore.py` | scripts | reviewed by main/subagent pass; no definite issue found |

## Speculative / Skipped Items

| File | Item | Reason not changed |
| --- | --- | --- |
| BelloBox/Screenshot/OCR/AIImageClient.swift | BELLOBOX_E2E_LLM_OCR_FIXTURE | Debug-only fixture hook; no tracked reference, but could be manual/CI-only. Not a definite deletion. |
| BelloBox/UI/SelectionOverlayController.swift / BelloBoxApp.swift / CaptureOverlayController.swift | DEBUG E2E hooks | Test-only env hooks are guarded by #if DEBUG and used by tracked e2e scripts. |
| BelloBox/Selection/AccessibilityService.swift | delayed paste after target app activation | The 0.1s delay intentionally lets the source app reactivate before posting Cmd-V for replacement. |
| BelloBox/UI/CaptureOverlayController.swift / BelloBox/UI/SelectionOverlayController.swift | DEBUG e2e delayed quit | The 0.1s delay is behind e2e env hooks and lets marker writes finish before terminating the debug app. |
| BelloBox/AI/CodexAppServerClient.swift | legacy approval request names and completedText reconciliation | Covered behavior prevents app-server hangs or lost text; not synthetic cached success. |
| BelloBox/Selection/AccessibilityService.swift | copy fallback + usleep polling | Bounded fallback is required when AX selected text is unavailable. |
| BelloBox/Screenshot/ScreenCoordinateSpace.swift | display pixel fallback | Covered by coordinate-space tests for CG point-size reports; no definite issue. |
| BelloBox/Screenshot/ScreenCaptureFrameGrabber.swift | 10s timeout | Bounded timeout for pre-macOS-14 frame capture path; no definite issue. |
| BelloBox/Screenshot/OCR/LLMOCRService.swift | non-cancellation local OCR failure suppression | Hybrid LLM OCR can still succeed without a local hint; product behavior not a proven bug. |
| BelloBox/UI/ScreenshotPopupView.swift | preview render fallback to base image | Copy/save paths surface renderer errors; preview behavior not proven wrong. |
| BelloBox/Screenshot/WindowCapturePicker.swift | guarded force unwrap | Force unwrap follows title non-empty guard; optional-binding rewrite would be style only. |
| BelloBox/Tools/TextTransforms.swift | URL + decoding semantics | Could be form/query decoding; no call-site proves literal percent-decoding intent. |
| BelloBox/Screenshot/ScreenshotModels.swift / ImageStitcher.swift | ScrollDirection.up modeled but unused | Could be reserved for future scrolling direction support; no definite removal. |

## Stop Condition

- Complete: complete first-party manifest accounted for; subagent workstreams integrated; confirmed bugs/dead state/stale docs fixed; broad tests, Debug/Release builds, and real e2e passed. Remaining items are speculative/reserved behavior and intentionally skipped.
