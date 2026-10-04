# Bello Box UI preservation review

Review date: 2026-10-04. Contract: preserve the existing app's UI; do not replace it with a generic utility IDE. This document is an acceptance checklist, not a claim that every item is implemented.

## Evidence and current verdict

- Baseline: existing Swift source under `BelloBox/UI` and `BelloBox/Launcher`, read from checkout at `e43b1c4`.
- The actual Rust capture `bellobox-qr-native-20261004-0308.png` was inspected. It fails preservation: permanent command-list sidebar instead of Home, cool dark generic shell, unrelated universal toolbar, code editor gutters/status bars in ordinary text inputs, QR beside its input, and a duplicate text representation below the QR.
- Actual official Swift Home, command-palette, JSON, World Clock light/dark, screenshot-editor and Video-to-GIF screenshots were inspected from `bello-reference-ui`; source provenance is recorded in its `reference-manifest.json`. The Home image is older (v0.0.66), with vertical colorful cards and 21 commands. Current source takes precedence for horizontal orange-family cards and the larger catalog. These images establish the app’s visual identity, not a current pixel-perfect baseline.
- Revised Rust screenshots and macOS light/dark checks remain required. A passing build alone does not satisfy this review.

## Revised capture review: 03:37 UTC

Actual revised Rust Home screenshot `bellobox-source-home-20261004-0337.png` was inspected (binary built03:36 UTC).

- Verified structural restoration: real38pt toolbox artwork,200pt category sidebar and footer, exact Home heading/subtitle/search hierarchy,61-command count, conditional selection-access notice, adaptive three-column nine-tool grid with current-source horizontal orange-family badges, title/subtitle hierarchy and bottom shortcut hint.
- The capture’s card shadows look heavier than the source. Current implementation has since replaced GPUI default shadows with the source black0.035/blur3/y1 recipe; final-binary recapture is required. Source cards are12pt radius,14pt padding,36pt badges,13pt semibold title and11pt subtitle.
- Typography is structurally correct in code but Linux DejaVu Sans appears wider/heavier than Swift system typography. Source heading is28pt semibold with−0.6 tracking; exact macOS type metrics and letter spacing remain unverified.
- Original Glass materials are not implemented; the capture is a solid fallback. Home status/shortcut preference behavior and secondary destinations are not fully ported. This is a structural pass for the shown Home state, not full visual/interaction acceptance.

## QR capture and final verification boundary

- Actual `bellobox-source-qr-20261004-0339.png` was inspected. It is now a separate source-shaped popup: header, real QR above ordinary Encoded text input, byte/capacity line, and trailing Save/Copy actions. The old command sidebar, code gutter and duplicate ASCII QR output are gone.
- The QA operator verified Home’s QR card opens a new native window, and typing `hello` regenerated the QR. This review did not independently decode or export that image.
- At the desktop’s forced1180×812 client size, the QR footer floated above roughly180px of unused bottom space. Source Swift gives the capped QR card a flexible outer slot so the footer stays low; this was sent to the implementation worker for correction.
- Native CUA transport disconnected at03:41:54 UTC; subsequent same-route inventory checks returned no apps / connection refused. Final-binary shadow/footer recaptures, intended520×620 QR size, minimum-size resizing and dark-mode testing are **not verified**. New source changes after the last capture are not visual evidence.
- Final verdict for this review pass: **Home and QR structural restoration verified; full visual and interaction parity not accepted.** Remaining source checklist items and platform-specific typography/materials must stay explicit.

## Shared visual contract

Sources: `BelloBox/UI/Theme.swift`, `WindowMaterials.swift`, `AppWindowChrome.swift`.

- [ ] Use the shipped orange-toolbox artwork, not a text initial, emoji, or unrelated icon.
- [ ] Preserve warm neutrals and orange ink. Light RGB tokens: background `(0.965, 0.960, 0.951)`, surface `(1, 0.998, 0.994)`, well `(0.947, 0.941, 0.931)`, text `(0.10, 0.095, 0.09)`, secondary `(0.26, 0.245, 0.23)`, accent `(0.54, 0.21, 0.025)`.
- [ ] Dark RGB tokens: background `(0.085, 0.083, 0.080)`, surface `(0.145, 0.141, 0.136)`, well `(0.112, 0.108, 0.103)`, text `(0.98, 0.975, 0.97)`, secondary `(0.87, 0.855, 0.84)`, accent `(1, 0.73, 0.52)`.
- [ ] Keep readable orange text distinct from decorative brand orange; primary buttons use the source orange gradient `(0.74, 0.34, 0.05)` to `(0.64, 0.28, 0.05)` with white labels.
- [ ] Match 12pt controls, 28pt minimum button heights, 8pt control radius, 12pt card radius, 16pt popup radius, subtle borders/shadows, and outline icon badges. Do not substitute large emoji or ASCII glyphs for the original icon language.
- [ ] Match both appearance modes. Glass is the original default; preserve macOS native blur/translucency and the Solid choice. Accessibility Reduce Transparency / Increase Contrast force solid surfaces. A Linux solid fallback is not proof of macOS glass parity.

## Home and Settings

Sources: `MainView.swift`, `MainWindowController.swift`, `SettingsView.swift`.

- [ ] Normal launch opens Home, not the palette or a selected tool. Home defaults to 1000×760 content, minimum 900×640.
- [ ] Home sidebar is 200pt wide. Header: 38pt real app icon, “Bello Box” 15pt semibold and “Your workspace” 11pt. Categories in order: Overview, Developer, Capture, Text & AI. Each row is 40pt, 8pt rounded selection wash with a 3×16pt orange leading marker.
- [ ] Keep lower selection/AI status, Settings, Setup guide, available update action, and version. Do not fill the Home sidebar with every command.
- [ ] Content uses 24pt inset and 20pt vertical spacing. Overview heading is “Everything within reach.”, 28pt semibold, tracking −0.6; subtitle 12pt.
- [ ] Search is a card button (“Search all tools and commands”, count, shortcut) opening the separate palette. Permission notice remains conditional.
- [ ] Quick access uses an adaptive grid: minimum240pt per card, 12pt horizontal/vertical gaps. With200pt sidebar,1pt divider and48pt padding, this yields two columns at900pt and three at the default1000pt window (three-column threshold993pt); never hardcode the column count. Original order: Screenshot, Recording, World Clock, JSON, Text Tools, QR, Ask AI, Snippets, Compare. Cards use 36pt badges, 13pt semibold titles, 11pt two-line subtitles, trailing chevron, 14pt padding and 12pt radius.
- [ ] Developer retains filter, group menu, new-tools toggle and task-group sections. Empty filter has Clear filters. Navigation keyboard shortcuts remain usable.
- [ ] Settings remains its own 200pt-sidebar window (minimum 900×680), with General, AI, Capture, Recording, OCR, Permissions and Prompt content and 24pt content padding; do not replace it with CLI-only instructions.

## Palette and tool windows

Sources: `Launcher/LauncherView.swift`, `LauncherModel.swift`, `LauncherWindowController.swift`, `UtilityWorkbenchView.swift`, `UtilityWorkbenchWindowController.swift`, `UI/QRCodePopupView.swift`, `TextToolsPopupView.swift`, `ActionPopupView.swift`.

- [ ] Palette is a separate transient 680pt panel: 64pt search row, optional 48pt selection strip, 25pt section heading, 42pt command headers, 41pt footer; 16pt outer radius.
- [ ] Exactly one focused command expands with its interactive preview. Best match labels the actual suggestion. Up/Down, Enter and Escape retain their source behavior; typing remains in search unless an editor owns focus.
- [ ] Search does not replace the Home sidebar. Palette closes on outside click / lost key focus except its own menus, sheets and children.
- [ ] Preview draft/options transfer to an independent resizable tool window. Opening another window does not share mutable draft state.
- [ ] Full developer workbench defaults 820×660, minimum 740×560. Header: badge, 16pt title, subtitle, New Window and Search controls. Below: tool-specific controls, input, additional controls, result, footer. Compare has equal-height paired inputs; ordinary JSON input/result are vertically organized.
- [ ] QR popup defaults 520×620. Header is “QR Code” with subtitle; real crisp QR on white backing sits ABOVE the 100pt Encoded text editor, byte count/capacity, status, and trailing Save… / Copy Image footer. No second-input box or ASCII QR result is shown.
- [ ] Text Tools defaults 720×660. Header, seven-way category bar (Case / Encode / Decode / Pretty / Hash / Lines / Count), input with Paste/Clear/Reset and 120pt editor, operation options/output, footer. Preserve actual category-specific views.
- [ ] Ask AI defaults 720×600. Header/provider, selected-text disclosure, optional setup banner, custom prompt, Writing actions grid, then result and Copy/Replace actions. Keep streaming/stop/retry states and disabled actions truthful.
- [ ] World Clock remains an independent minimum780×640 window:34pt icon/20pt title header with Live time or Planning and Now; meeting-planner card (reference menu/date/time/quality/timeline), location cards with large time, footer Add Location/Copilot/Copy Times. A plain timestamp output box is not equivalent.
- [ ] Screenshot editor preserves its horizontal annotation/color toolbar, large scrollable/zoomable canvas, optional OCR reader panel, and bottom zoom/copy/save/finish controls. Video to GIF preserves video preview, source/result selection, trim sliders, frame-rate/longest-edge/loop controls and conversion action.
- [ ] Tool popups retain matching minimize/restore/close controls; Escape closes. Ordinary text input must not acquire a code-editor line gutter or Vim status bar.

## Required evidence before acceptance

- [ ] Capture revised Home, Developer, palette with one expanded preview, QR, Text Tools and AI in light and dark at normal size and minimum size.
- [ ] Compare against the same Swift states and content, not unrelated marketing layouts or generic screenshots.
- [ ] Test Home→palette→tool, preview handoff, repeated tool opens, close/minimize/restore, keyboard navigation, copy/save, and resize/scroll without clipping.
- [ ] Record explicitly which surfaces are matched, which still differ, and which were not tested on macOS. Do not call this a full UI-preserving rewrite while key windows or interactions are absent.

## Reproducible original visual references

Official product images below were inspected; they can predate the current Swift source.

- [bello_box_command_palette.jpg](https://belloware.com/assets/bello_box_command_palette.jpg) — Git blob `2d536354f18ede04d8b38c6f73ecdb48895afab4`
- [bello_box_home_workspace.jpg](https://belloware.com/assets/bello_box_home_workspace.jpg) — Git blob `8afe0ee4abefa5392ffd75a5891ba44f7ef8750f`
- [bello_box_json_tools.jpg](https://belloware.com/assets/bello_box_json_tools.jpg) — Git blob `b75c3b4749636d8f7681b54df95fed7cb7eabd31`
- [bello_box_screenshot_editor.jpg](https://belloware.com/assets/bello_box_screenshot_editor.jpg) — Git blob `6a289a62277d1fe542ff2b8566d6755de8b47666`
- [bello_box_video_to_gif.jpg](https://belloware.com/assets/bello_box_video_to_gif.jpg) — Git blob `227e78a35b1cef4e1c3ae3814d340c7d01a3ed86`
- [bello_box_world_clock_dark.jpg](https://belloware.com/assets/bello_box_world_clock_dark.jpg) — Git blob `8cf0236f6a783a3d45b8b21626ed97d30efcfc24`
- [bello_box_world_clock_light.jpg](https://belloware.com/assets/bello_box_world_clock_light.jpg) — Git blob `8a73215946662acbc9d10719cb12e4be7154606a`
