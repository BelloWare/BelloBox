import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

@MainActor
final class DialogPresentationTests: XCTestCase {
    func testSettingsDiagnosticsSheetRejectsScreenshotHotkeyWithVisibleHint() async throws {
        let settings = makeSettings()
        let owner = makeWindow(level: .normal)
        defer { close(owner); OverlayTooltipPresenter.shared.hide() }
        let view = SettingsView(settings: settings, navigation: WindowNavigation(.capture))
        owner.contentView = NSHostingView(rootView: view)
        view.exportCaptureDiagnostics(in: owner)
        let panel = try await savePanel(in: owner)
        XCTAssertEqual(panel.nameFieldStringValue, "bello-box-capture-diagnostics.log")
        let controller = SelectionOverlayController(settings: settings)
        var accepted: [String] = []
        controller.debugCaptureRequest = { accepted.append($0) }

        controller.triggerScreenshotShortcut()
        XCTAssertTrue(accepted.isEmpty, "A screenshot hotkey must stop before requesting capture")
        XCTAssertEqual(OverlayTooltipPresenter.shared.visibleText, "Close the dialog before starting a capture.")
        XCTAssertTrue(OverlayTooltipPresenter.shared.debugPanel?.isVisible == true)
        XCTAssertTrue(panel.isKeyWindow, "The hint must leave the file dialog usable")
        XCTAssertTrue(owner.attachedSheet === panel)
        let visibleWindows = Set(NSApp.windows.filter(\.isVisible).map(ObjectIdentifier.init))
        controller.triggerScreenshotCapture()
        controller.triggerScrollingScreenshotCapture()
        XCTAssertTrue(NSApp.windows.filter(\.isVisible).allSatisfy { visibleWindows.contains(ObjectIdentifier($0)) },
                      "Menu capture requests must also leave the existing sheet alone")
        let hint = try XCTUnwrap(OverlayTooltipPresenter.shared.debugPanel)
        XCTAssertGreaterThan(hint.level.rawValue, panel.level.rawValue)
        XCTAssertTrue(hint.ignoresMouseEvents)
        XCTAssertFalse(hint.canBecomeKey)
        try await waitUntil { OverlayTooltipPresenter.shared.visibleText == nil }
        XCTAssertTrue(panel.isKeyWindow, "The hint expires without moving focus")

        // File panels handle WindowServer events remotely; their Cancel action is
        // the native equivalent of Escape (physical Escape is covered by the fixture).
        panel.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        controller.triggerScreenshotShortcut()
        XCTAssertEqual(accepted, ["screenshot"], "Dismissal must unblock the next request")
    }

    func testSnippetDeleteConfirmationRejectsRecordingHotkeyAndEscapeCancels() async throws {
        let owner = makeWindow(level: .normal)
        defer { close(owner); OverlayTooltipPresenter.shared.hide() }
        var presented = true
        var deletions = 0
        // Mount the same native confirmation used by UtilityWorkbenchView's
        // Delete button; no real snippet library is read or written.
        owner.contentView = NSHostingView(rootView: Text("Snippet fixture").windowConfirmation(
            "Delete this snippet?", isPresented: Binding(get: { presented }, set: { presented = $0 }),
            confirmTitle: "Delete", onConfirm: { deletions += 1 }
        ))
        try await waitUntil { owner.attachedSheet?.isKeyWindow == true }
        let sheet = try XCTUnwrap(owner.attachedSheet)
        let controller = SelectionOverlayController(settings: makeSettings())
        var accepted: [String] = []
        controller.debugCaptureRequest = { accepted.append($0) }

        controller.triggerRecording()
        XCTAssertTrue(accepted.isEmpty, "A recording hotkey must stop before requesting capture")
        XCTAssertEqual(OverlayTooltipPresenter.shared.visibleText, "Close the dialog before starting a capture.")
        XCTAssertTrue(sheet.isKeyWindow)
        try sendEscape(to: sheet)
        try await waitUntil { !presented && owner.attachedSheet == nil }
        XCTAssertEqual(deletions, 0)
        controller.triggerRecording()
        XCTAssertEqual(accepted, ["recording"])
    }

    func testUnrelatedDialogDoesNotDisableCaptureEscape() async throws {
        let controller = CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(), settings: makeSettings(), macOCRService: MacVisionOCRService()
        )
        var cancelled = false
        controller.beginScreenshotForTesting(snapshots: [], onError: { XCTFail($0) }, onCancel: { cancelled = true })
        let overlay = try XCTUnwrap(controller.debugOverlayWindows.first)
        let owner = makeWindow(level: .normal)
        defer { controller.cancel(); close(owner) }
        let alert = NSAlert()
        alert.messageText = "Unrelated confirmation"
        alert.addButton(withTitle: "Cancel").keyEquivalent = "\u{1b}"
        var response: NSApplication.ModalResponse?
        WindowDialogPresenter.present(alert, for: owner) { response = $0 }
        try await waitUntil { alert.window.isKeyWindow }
        XCTAssertTrue(WindowDialogPresenter.hasDialog(for: owner))
        XCTAssertFalse(WindowDialogPresenter.hasDialog(for: overlay))

        XCTAssertFalse(controller.handleKeyEvent(try escapeEvent(in: alert.window)),
                       "The frontmost dialog must receive its own Escape")
        // Reproduce a hidden/unreachable unrelated sheet without starting a
        // second capture. Its ownership must never suppress the shield's Escape.
        alert.window.level = .normal
        overlay.makeKeyAndOrderFront(nil)
        XCTAssertTrue(controller.handleKeyEvent(try escapeEvent(in: overlay)))
        XCTAssertTrue(cancelled)
        XCTAssertTrue(controller.debugOverlayWindows.isEmpty)
        XCTAssertNil(response, "Cancelling this overlay must not cancel another owner's sheet")
        owner.endSheet(alert.window, returnCode: .cancel)
        try await waitUntil { response != nil }
    }

    func testFrontmostUnrelatedConfirmationReceivesEscapeBeforeCapture() async throws {
        let controller = CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(), settings: makeSettings(), macOCRService: MacVisionOCRService()
        )
        var cancelled = false
        controller.beginScreenshotForTesting(snapshots: [], onError: { XCTFail($0) }, onCancel: { cancelled = true })
        let overlay = try XCTUnwrap(controller.debugOverlayWindows.first)
        let owner = makeWindow(level: .normal)
        defer { controller.cancel(); close(owner) }
        let alert = NSAlert()
        alert.messageText = "Frontmost confirmation"
        alert.addButton(withTitle: "Cancel").keyEquivalent = "\u{1b}"
        var response: NSApplication.ModalResponse?
        WindowDialogPresenter.present(alert, for: owner) { response = $0 }
        try await waitUntil { alert.window.isKeyWindow }
        XCTAssertGreaterThan(alert.window.level.rawValue, overlay.level.rawValue)
        try sendEscape(to: alert.window)
        try await waitUntil { response != nil && !WindowDialogPresenter.hasDialog(for: owner) }
        XCTAssertEqual(response, .alertFirstButtonReturn)
        XCTAssertFalse(cancelled, "The first Escape dismisses only the frontmost dialog")
        overlay.makeKeyAndOrderFront(nil)
        try sendEscape(to: overlay)
        try await waitUntil { cancelled }
        XCTAssertTrue(controller.debugOverlayWindows.isEmpty)
    }

    func testNativeSheetAlsoBlocksCaptureAndOwnersRemainIndependent() async throws {
        let owner = makeWindow(level: .normal)
        let other = makeWindow(level: .floating)
        let nativeSheet = NSPanel(contentRect: CGRect(x: 0, y: 0, width: 350, height: 150),
                                  styleMask: [.titled], backing: .buffered, defer: false)
        defer { close(owner); close(other); OverlayTooltipPresenter.shared.hide() }
        // Covers sheets made by AppKit/SwiftUI outside WindowDialogPresenter,
        // such as the World Clock location picker.
        owner.beginSheet(nativeSheet, completionHandler: nil)
        try await waitUntil { owner.attachedSheet === nativeSheet }
        let controller = SelectionOverlayController(settings: makeSettings())
        var requests = 0
        controller.debugCaptureRequest = { _ in requests += 1 }
        controller.triggerScreenshotShortcut()
        controller.triggerRecording()
        XCTAssertEqual(requests, 0)
        XCTAssertTrue(WindowDialogPresenter.hasDialog(for: owner))
        XCTAssertFalse(WindowDialogPresenter.hasDialog(for: other))

        let alert = NSAlert()
        alert.messageText = "Another owner's confirmation"
        alert.addButton(withTitle: "Cancel")
        WindowDialogPresenter.present(alert, for: other) { _ in }
        try await waitUntil { other.attachedSheet === alert.window }
        owner.endSheet(nativeSheet, returnCode: .cancel)
        try await waitUntil { !WindowDialogPresenter.hasDialog(for: owner) }
        XCTAssertTrue(WindowDialogPresenter.hasDialog(for: other))
        controller.triggerRecording()
        XCTAssertEqual(requests, 0, "Closing one owner's sheet must not clear another owner's state")
        other.endSheet(alert.window, returnCode: .cancel)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        controller.triggerScreenshotShortcut()
        XCTAssertEqual(requests, 1)
    }

    func testCaptureBoundariesRejectDialogBeforeCreatingAnyShields() async throws {
        let owner = makeWindow(level: .normal)
        defer { close(owner); OverlayTooltipPresenter.shared.hide() }
        let panel = NSSavePanel()
        WindowDialogPresenter.present(panel, for: owner) { _ in }
        _ = try await savePanel(in: owner)
        let settings = makeSettings()
        let controller = CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(), settings: settings, macOCRService: MacVisionOCRService()
        )
        defer { controller.cancel() }
        var cancellations = 0
        controller.beginRecording(
            initialOptions: settings.recordingOptions, onRecord: { _, _ in XCTFail("Must not record") },
            onError: { XCTFail($0) }, onCancel: { cancellations += 1 }
        )
        controller.beginScreenshot(onError: { XCTFail($0) }, onCancel: { cancellations += 1 })
        // Enter at the post-snapshot boundary, representing a dialog opened while
        // the asynchronous snapshot phase was in flight. No Screen Recording needed.
        controller.beginScreenshotForTesting(
            snapshots: [], onError: { XCTFail($0) }, onCancel: { cancellations += 1 }
        )
        XCTAssertEqual(cancellations, 3)
        XCTAssertTrue(controller.debugOverlayWindows.isEmpty)
        let editor = ScreenshotOverlayEditorController()
        defer { editor.close() }
        let model = makeScreenshotModel(settings: settings)
        XCTAssertFalse(editor.show(viewModel: model, captureFrame: owner.frame, settings: settings))
        XCTAssertNil(model.hostWindow)
        XCTAssertTrue(panel.isKeyWindow)
        XCTAssertTrue(owner.attachedSheet === panel)
        panel.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
    }

    func testUnrelatedDialogDoesNotDisableScreenshotEditorEscape() async throws {
        let settings = makeSettings()
        let model = makeScreenshotModel(settings: settings)
        let controller = ScreenshotOverlayEditorController()
        let screen = try XCTUnwrap(NSScreen.main)
        controller.show(viewModel: model, captureFrame: screen.frame.insetBy(dx: 200, dy: 200), settings: settings)
        try await waitUntil { model.hostWindow != nil }
        let overlay = try XCTUnwrap(model.hostWindow)
        let owner = makeWindow(level: .normal)
        defer { controller.close(); close(owner) }
        var closes = 0
        model.onClose = { closes += 1 }
        let panel = NSSavePanel()
        WindowDialogPresenter.present(panel, for: owner) { _ in }
        _ = try await savePanel(in: owner)
        XCTAssertFalse(controller.handleKeyEvent(try escapeEvent(in: panel)))
        panel.level = .normal
        overlay.makeKeyAndOrderFront(nil)
        XCTAssertTrue(controller.handleKeyEvent(try escapeEvent(in: overlay)))
        XCTAssertEqual(closes, 1, "Another owner's dialog cannot disable this editor's Escape")
        panel.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
    }

    func testScreenshotSaveAttachesToOverlayAndCancelPreservesShield() async throws {
        let suite = "BelloBoxTests.DialogPresentation.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let settings = AppSettings(defaults: defaults)
        let model = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1, source: .importedClipboard
            ),
            settings: settings
        )
        let controller = ScreenshotOverlayEditorController()
        defer { controller.close() }
        let screen = try XCTUnwrap(NSScreen.main)
        controller.show(viewModel: model, captureFrame: screen.frame.insetBy(dx: 200, dy: 200), settings: settings)
        let overlay = try XCTUnwrap(NSApp.keyWindow)
        XCTAssertEqual(overlay.level, .screenSaver)

        let inspected = expectation(description: "save panel inspected and cancelled")
        // This also runs inside the old runModal loop, so the regression fails
        // with an assertion instead of leaving an invisible dialog hung in CI.
        let timer = Timer(timeInterval: 1, repeats: false) { _ in
            MainActor.assumeIsolated {
                XCTAssertNotNil(overlay.attachedSheet, "The save panel must attach above the screenshot shield")
                XCTAssertEqual(overlay.level, .screenSaver, "The shield must stay above other apps")
                let panel = (overlay.attachedSheet as? NSSavePanel)
                    ?? (NSApp.modalWindow as? NSSavePanel)
                    ?? NSApp.windows.compactMap { $0 as? NSSavePanel }.first
                XCTAssertNotNil(panel)
                panel?.cancel(nil)
                inspected.fulfill()
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        RunLoop.main.add(timer, forMode: .modalPanel)
        defer { timer.invalidate() }
        model.saveRenderedImage()
        await fulfillment(of: [inspected], timeout: 5)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertTrue(overlay.isVisible)
        XCTAssertEqual(overlay.level, .screenSaver)
        XCTAssertNil(model.errorMessage)
        XCTAssertNil(model.statusMessage, "Cancelling must not report a saved image")
    }

    func testSaveSheetAcceptanceRestoresDialogLevelAndPreservesAllShields() async throws {
        try await checkPresentation(panel: NSSavePanel(), response: .OK)
    }

    func testSaveSheetCancellationRestoresDialogLevelAndPreservesAllShields() async throws {
        try await checkPresentation(panel: NSSavePanel(), response: .cancel)
    }

    func testOpenSheetCancellationRestoresDialogLevelAndPreservesAllShields() async throws {
        try await checkPresentation(panel: NSOpenPanel(), response: .cancel)
    }

    func testAlertAcceptanceAndCancellationRestoreDialogLevel() async throws {
        let owner = makeWindow(level: .screenSaver)
        defer { close(owner) }
        for response in [NSApplication.ModalResponse.alertFirstButtonReturn, .alertSecondButtonReturn] {
            let alert = NSAlert()
            alert.messageText = "Fixture confirmation"
            alert.addButton(withTitle: "Cancel")
            alert.addButton(withTitle: "Confirm")
            let originalLevel = alert.window.level
            var received: NSApplication.ModalResponse?
            WindowDialogPresenter.present(alert, for: owner) {
                XCTAssertEqual(alert.window.level, originalLevel)
                received = $0
            }
            try await waitUntil { owner.attachedSheet === alert.window }
            XCTAssertGreaterThan(alert.window.level.rawValue, owner.level.rawValue)
            owner.endSheet(alert.window, returnCode: response)
            try await waitUntil { received != nil }
            XCTAssertEqual(received, response)
            XCTAssertEqual(owner.level, .screenSaver)
            XCTAssertTrue(owner.isVisible)
        }
    }

    private func checkPresentation(panel: NSSavePanel, response: NSApplication.ModalResponse) async throws {
        let owner = makeWindow(level: .screenSaver)
        let otherShield = makeWindow(level: NSWindow.Level(rawValue: NSWindow.Level.screenSaver.rawValue + 1))
        defer { close(owner); close(otherShield) }
        let originalLevel = panel.level
        var result: NSApplication.ModalResponse?
        WindowDialogPresenter.present(panel, for: owner) {
            XCTAssertEqual(panel.level, originalLevel, "Restore before calling client code")
            result = $0
        }
        try await waitUntil { owner.attachedSheet === panel }
        XCTAssertTrue(panel.sheetParent === owner)
        XCTAssertGreaterThan(panel.level.rawValue, otherShield.level.rawValue)
        XCTAssertEqual(owner.level, .screenSaver)
        XCTAssertTrue(WindowDialogPresenter.hasActiveDialogs)
        owner.endSheet(panel, returnCode: response)
        try await waitUntil { result != nil }
        XCTAssertEqual(result, response)
        XCTAssertEqual(panel.level, originalLevel)
        XCTAssertEqual(owner.level, .screenSaver)
        XCTAssertEqual(otherShield.level.rawValue, NSWindow.Level.screenSaver.rawValue + 1)
        XCTAssertTrue(owner.isVisible && otherShield.isVisible)
        XCTAssertFalse(WindowDialogPresenter.hasActiveDialogs)
    }

    func testExplicitOwnerWinsOverAnotherKeyWindow() async throws {
        let owner = makeWindow(level: .screenSaver)
        let other = makeWindow(level: .statusBar)
        defer { close(owner); close(other) }
        other.makeKeyAndOrderFront(nil)
        let panel = NSSavePanel()
        WindowDialogPresenter.present(panel, for: owner) { _ in }
        try await waitUntil { owner.attachedSheet === panel }
        XCTAssertNil(other.attachedSheet)
        panel.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
    }

    func testClosedOwnerCancelsWithoutOpeningAnOrphanDialog() {
        let owner = makeWindow(level: .screenSaver)
        close(owner)
        let panel = NSSavePanel()
        var response: NSApplication.ModalResponse?
        WindowDialogPresenter.present(panel, for: owner) { response = $0 }
        XCTAssertEqual(response, .cancel)
        XCTAssertFalse(panel.isVisible)
        XCTAssertFalse(WindowDialogPresenter.hasActiveDialogs)
    }

    func testPanelStartupFailureRestoresLevelBeforeAndAfterCompletion() {
        let owner = makeWindow(level: .screenSaver)
        defer { close(owner) }
        let panel = AbortingSavePanel()
        let originalLevel = panel.level
        var responses: [NSApplication.ModalResponse] = []
        WindowDialogPresenter.present(panel, for: owner) {
            XCTAssertEqual(panel.level, originalLevel)
            responses.append($0)
        }
        XCTAssertEqual(responses, [.abort])
        XCTAssertEqual(panel.level, originalLevel)
        XCTAssertEqual(owner.level, .screenSaver)
        XCTAssertNil(owner.attachedSheet)
        XCTAssertFalse(WindowDialogPresenter.hasActiveDialogs)
    }

    func testRepeatedSaveDoesNotStackSheets() async throws {
        let owner = makeWindow(level: .screenSaver)
        defer { close(owner) }
        let first = NSSavePanel()
        WindowDialogPresenter.present(first, for: owner) { _ in }
        try await waitUntil { owner.attachedSheet === first }
        let second = NSSavePanel()
        var secondResponse: NSApplication.ModalResponse?
        WindowDialogPresenter.present(second, for: owner) { secondResponse = $0 }
        XCTAssertEqual(secondResponse, .cancel)
        XCTAssertTrue(owner.attachedSheet === first)
        XCTAssertFalse(second.isVisible)
        first.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
    }

    func testScreenshotSuccessfulWriteAndWriteErrorKeepOverlayLevelAndFocus() async throws {
        let directory = try temporaryDirectory()
        let settings = makeSettings()
        let model = makeScreenshotModel(settings: settings)
        let controller = ScreenshotOverlayEditorController()
        defer { controller.close() }
        let screen = try XCTUnwrap(NSScreen.main)
        controller.show(viewModel: model, captureFrame: screen.frame.insetBy(dx: 200, dy: 200), settings: settings)
        try await waitUntil { model.hostWindow != nil }
        let owner = try XCTUnwrap(model.hostWindow)
        for shouldFail in [false, true] {
            let destination = directory.appendingPathComponent(shouldFail ? "directory.png" : "saved.png")
            if shouldFail {
                try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
            }
            // Control only the selected URL. The native sheet and PNG writer
            // are real, including the write error after the sheet finishes.
            let panel = DestinationSavePanel()
            panel.destination = destination
            let originalLevel = panel.level
            var result: Result<Void, Error>?
            ImageExportService.savePNG(model.document.baseImage, suggestedName: "fixture", in: owner, savePanel: panel) {
                XCTAssertEqual(panel.level, originalLevel)
                result = $0
            }
            _ = try await savePanel(in: owner)
            owner.endSheet(panel, returnCode: .OK)
            try await waitUntil { result != nil && !WindowDialogPresenter.hasActiveDialogs }
            XCTAssertEqual(owner.level, .screenSaver)
            XCTAssertTrue(owner.isVisible)
            if shouldFail {
                guard case .failure = result else { return XCTFail("Expected the PNG write to fail") }
            } else {
                try result?.get()
                XCTAssertEqual(Array(try Data(contentsOf: destination).prefix(8)), [137, 80, 78, 71, 13, 10, 26, 10])
            }
            try await waitUntil { owner.isKeyWindow }
        }
    }

    func testEscapeCancelsSaveInsteadOfClosingScreenshotEditor() async throws {
        let settings = makeSettings()
        let model = makeScreenshotModel(settings: settings)
        let controller = ScreenshotOverlayEditorController()
        defer { controller.close() }
        let screen = try XCTUnwrap(NSScreen.main)
        controller.show(viewModel: model, captureFrame: screen.frame.insetBy(dx: 200, dy: 200), settings: settings)
        try await waitUntil { model.hostWindow != nil }
        let owner = try XCTUnwrap(model.hostWindow)
        var closes = 0
        model.onClose = { closes += 1 }
        model.saveRenderedImage()
        let panel = try await savePanel(in: owner)
        // Remote save panels consume WindowServer key events in another process.
        // Verify our local monitor passes Escape through, then emulate its Cancel.
        XCTAssertFalse(controller.handleKeyEvent(try escapeEvent(in: panel)))
        model.handleEscape()
        XCTAssertEqual(closes, 0)
        panel.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertEqual(closes, 0)
        XCTAssertTrue(owner.isVisible)
        XCTAssertEqual(owner.level, .screenSaver)
        model.handleEscape()
        XCTAssertEqual(closes, 1, "Escape belongs to the editor again after the sheet")
    }

    func testInlineCaptureSaveOwnsEscapeAndKeepsNonactivatingShield() async throws {
        let screen = try XCTUnwrap(NSScreen.main)
        let displayID = try XCTUnwrap(ScreenCoordinateSpace.displayID(for: screen))
        let image = ScreenshotTestHelpers.image(width: Int(screen.frame.width), height: Int(screen.frame.height))
        let controller = CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(), settings: makeSettings(), macOCRService: MacVisionOCRService()
        )
        defer { controller.cancel() }
        var cancelled = false
        controller.beginScreenshotForTesting(
            snapshots: [DisplaySnapshot(displayID: displayID, screenFrame: screen.frame, scale: 1, image: image)],
            initialSelection: .area(CaptureArea(cocoaRect: screen.frame.insetBy(dx: 100, dy: 100), displayID: displayID)),
            onError: { XCTFail($0) }, onCancel: { cancelled = true }
        )
        let model = try XCTUnwrap(controller.debugActiveScreenshotViewModel)
        try await waitUntil { model.hostWindow != nil }
        let owner = try XCTUnwrap(model.hostWindow)
        model.saveRenderedImage()
        let panel = try await savePanel(in: owner)
        XCTAssertTrue(owner.styleMask.contains(.nonactivatingPanel))
        XCTAssertFalse(controller.handleKeyEvent(try escapeEvent(in: panel)))
        panel.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertFalse(cancelled)
        XCTAssertFalse(controller.debugOverlayWindows.isEmpty)
        XCTAssertEqual(owner.level, .screenSaver)
        XCTAssertTrue(owner.isVisible)
        XCTAssertTrue(controller.handleKeyEvent(try escapeEvent(in: owner)))
        XCTAssertTrue(cancelled, "After the sheet, Escape must cancel the capture again")
        XCTAssertTrue(controller.debugOverlayWindows.isEmpty)
    }

    func testClosingScreenshotOverlayCancelsItsPendingSave() async throws {
        let settings = makeSettings()
        let model = makeScreenshotModel(settings: settings)
        let controller = ScreenshotOverlayEditorController()
        let screen = try XCTUnwrap(NSScreen.main)
        controller.show(viewModel: model, captureFrame: screen.frame.insetBy(dx: 200, dy: 200), settings: settings)
        try await waitUntil { model.hostWindow != nil }
        let owner = try XCTUnwrap(model.hostWindow)
        defer { controller.close() }
        model.saveRenderedImage()
        let panel = try await savePanel(in: owner)
        controller.close()
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertFalse(panel.isVisible)
        XCTAssertFalse(owner.isVisible)
        XCTAssertEqual(owner.level, .screenSaver)
        XCTAssertNil(model.statusMessage)
    }

    func testScreenshotDiscardConfirmationIsAboveOverlayAndEscapeKeepsEdits() async throws {
        let settings = makeSettings()
        let model = makeScreenshotModel(settings: settings)
        let controller = ScreenshotOverlayEditorController()
        let screen = try XCTUnwrap(NSScreen.main)
        defer { controller.close() }
        controller.show(viewModel: model, captureFrame: screen.frame.insetBy(dx: 200, dy: 200), settings: settings)
        try await waitUntil { model.hostWindow != nil }
        let owner = try XCTUnwrap(model.hostWindow)
        model.document.cropRect = CGRect(x: 0, y: 0, width: 40, height: 30)
        model.requestClose()
        try await waitUntil { owner.attachedSheet != nil }
        let alert = try XCTUnwrap(owner.attachedSheet)
        XCTAssertGreaterThan(alert.level.rawValue, owner.level.rawValue)
        try sendEscape(to: alert)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertFalse(model.showDiscardCloseConfirmation)
        XCTAssertTrue(model.hasImageEdits)
        XCTAssertTrue(owner.isVisible)
        XCTAssertEqual(owner.level, .screenSaver)
    }

    func testOCRViewModelAndFormatterExportsAttachToTheirOwner() async throws {
        let owner = makeWindow(level: .screenSaver)
        defer { close(owner) }
        let result = OCRResult(
            id: UUID(), engine: .appleVision(revision: 3, recognitionLevel: .accurate), target: .fullImage,
            plainText: "Local fixture", markdownText: "**Local fixture**", regions: [], languageHints: [],
            imageDigest: "fixture", warnings: [], createdAt: Date()
        )
        let model = OCRPanelViewModel()
        model.hostWindow = owner
        model.result = result
        for mode in [OCRDisplayMode.text, .markdown] {
            model.activeDisplayMode = mode
            model.saveText()
            let panel = try await savePanel(in: owner)
            XCTAssertTrue(panel.nameFieldStringValue.hasSuffix(mode == .markdown ? ".md" : ".txt"))
            panel.cancel(nil)
            try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
            XCTAssertNil(model.statusMessage)
        }
        for markdown in [false, true] {
            var cancelled = false
            let finish: (Result<Void, Error>) -> Void = {
                if case .failure(OCRError.cancelled) = $0 { cancelled = true } else { XCTFail("Expected cancellation") }
            }
            if markdown {
                OCRResultFormatter.saveMarkdown(result, suggestedName: "ocr", in: owner, completion: finish)
            } else {
                OCRResultFormatter.saveText(result, suggestedName: "ocr", in: owner, completion: finish)
            }
            let panel = try await savePanel(in: owner)
            panel.cancel(nil)
            try await waitUntil { cancelled }
            XCTAssertEqual(owner.level, .screenSaver)
        }
    }

    func testQRPopupReadsItsHostAndKeepsItAfterCancel() async throws {
        let owner = makeWindow(level: .statusBar)
        defer { close(owner) }
        let model = QRCodePopupViewModel(text: "Local fixture")
        owner.contentView = NSHostingView(rootView: QRCodePopupView(viewModel: model))
        try await waitUntil { model.hostWindow === owner }
        model.save()
        let panel = try await savePanel(in: owner)
        panel.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertNil(model.errorMessage)
        XCTAssertNil(model.statusMessage)
        XCTAssertEqual(owner.level, .statusBar)
    }

    func testQRPaletteSaveRetainsSearchAndClosingPaletteCancelsSave() async throws {
        let controller = LauncherWindowController()
        controller.settings = makeSettings()
        defer { controller.close() }
        controller.show(selection: BelloBox.TextSelection(
            text: "Local fixture", anchorRect: nil, appName: nil, bundleID: nil, pid: nil
        ), focus: .qr)
        let owner = try XCTUnwrap(controller.panel)
        let qr = try XCTUnwrap(controller.model?.expandedSession?.qr?.code)
        qr.save(in: owner)
        let panel = try await savePanel(in: owner)
        XCTAssertTrue(controller.isVisible)
        XCTAssertFalse(controller.handleKeyEvent(try escapeEvent(in: panel)))
        panel.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs && controller.isSearchFieldFocused }
        XCTAssertTrue(controller.isVisible)
        XCTAssertEqual(owner.level, .floating)
        qr.save(in: owner)
        let pending = try await savePanel(in: owner)
        controller.close()
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertFalse(pending.isVisible)
        XCTAssertFalse(controller.isVisible)
    }

    func testRecordingMovieGIFAndConversionSavePanelsAttach() async throws {
        let owner = makeWindow(level: .statusBar)
        defer { close(owner) }
        let directory = try temporaryDirectory()
        let movie = directory.appendingPathComponent("fixture.mov")
        let gif = directory.appendingPathComponent("fixture.gif")
        let model = RecordingReviewViewModel(fileURL: movie, gifURL: gif)
        defer { model.close() }
        model.hostWindow = owner
        let actions: [() -> Void] = [model.saveAs, model.saveGIFAs, model.exportGIF]
        for action in actions {
            action()
            let panel = try await savePanel(in: owner)
            panel.cancel(nil)
            try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
            XCTAssertFalse(model.isSaving || model.isConverting)
            XCTAssertNil(model.statusMessage)
            XCTAssertEqual(owner.level, .statusBar)
        }
    }

    func testVideoToGIFOpenAndSavePanelsAttachWithoutStartingOnCancel() async throws {
        let owner = makeWindow(level: .statusBar)
        defer { close(owner) }
        let model = VideoToGIFViewModel()
        defer { model.close() }
        model.hostWindow = owner
        model.chooseVideo()
        let open = try await savePanel(in: owner)
        XCTAssertTrue(open is NSOpenPanel)
        open.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertNil(model.sourceURL)
        let movie = try await SyntheticMediaFixture.writeMovie(
            to: temporaryDirectory().appendingPathComponent("fixture.mov"),
            size: CGSize(width: 80, height: 60), duration: 0.2, framesPerSecond: 10
        )
        model.load(movie)
        try await waitUntil { model.plan != nil }
        model.convert()
        let save = try await savePanel(in: owner)
        save.cancel(nil)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertFalse(model.isConverting)
        XCTAssertNil(model.result)
        XCTAssertEqual(owner.level, .statusBar)
    }

    func testRecordingDiscardConfirmationCancelsWithoutRemovingFiles() async throws {
        let owner = makeWindow(level: .statusBar)
        defer { close(owner) }
        var removals = 0
        let model = RecordingReviewViewModel(
            fileURL: try temporaryDirectory().appendingPathComponent("fixture.mov"),
            removeRecording: { _ in removals += 1 }
        )
        defer { model.close() }
        owner.contentView = NSHostingView(rootView: RecordingReviewView(viewModel: model))
        model.requestDiscard()
        try await waitUntil { owner.attachedSheet != nil }
        let alert = try XCTUnwrap(owner.attachedSheet)
        XCTAssertGreaterThan(alert.level.rawValue, owner.level.rawValue)
        try sendEscape(to: alert)
        try await waitUntil { !WindowDialogPresenter.hasActiveDialogs }
        XCTAssertEqual(removals, 0)
        XCTAssertFalse(model.showDiscardConfirmation)
        XCTAssertEqual(owner.level, .statusBar)
        XCTAssertTrue(owner.isVisible)
    }

    // MARK: Native integration helpers (no Screen Recording or real preferences)

    private func makeWindow(level: NSWindow.Level) -> PopupPanel {
        let window = PopupPanel(contentRect: CGRect(x: 120, y: 150, width: 800, height: 620))
        window.level = level
        window.contentView = NSView()
        window.makeKeyAndOrderFront(nil)
        return window
    }

    private func close(_ window: NSWindow) {
        WindowDialogPresenter.cancelDialogs(for: window)
        window.orderOut(nil)
        window.close()
    }

    private func savePanel(in owner: NSWindow) async throws -> NSSavePanel {
        try await waitUntil { owner.attachedSheet != nil }
        let panel = try XCTUnwrap(owner.attachedSheet as? NSSavePanel)
        try await waitUntil { panel.isVisible && panel.isKeyWindow }
        XCTAssertTrue(panel.sheetParent === owner)
        XCTAssertGreaterThan(panel.level.rawValue, owner.level.rawValue)
        return panel
    }

    private func sendEscape(to window: NSWindow) throws {
        NSApp.postEvent(try escapeEvent(in: window), atStart: false)
    }

    private func escapeEvent(in window: NSWindow) throws -> NSEvent {
        try XCTUnwrap(NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, characters: "\u{1b}",
            charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53
        ))
    }

    private func waitUntil(_ condition: () -> Bool, file: StaticString = #filePath, line: UInt = #line) async throws {
        for _ in 0..<200 {
            if condition() { return }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        XCTFail("Timed out waiting for native dialog state", file: file, line: line)
        throw CocoaError(.userCancelled)
    }

    private func temporaryDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBoxDialogs-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        return directory
    }

    private func makeSettings() -> AppSettings {
        let suite = "BelloBoxTests.DialogPresentation.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        addTeardownBlock { defaults.removePersistentDomain(forName: suite) }
        return AppSettings(defaults: defaults)
    }

    private func makeScreenshotModel(settings: AppSettings) -> ScreenshotPopupViewModel {
        ScreenshotPopupViewModel(
            document: ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                                         scale: 1, source: .importedClipboard),
            settings: settings
        )
    }
}

private final class DestinationSavePanel: NSSavePanel {
    var destination: URL?
    override var url: URL? { destination }
}

private final class AbortingSavePanel: NSSavePanel {
    override func beginSheetModal(for window: NSWindow, completionHandler handler: @escaping (NSApplication.ModalResponse) -> Void) {
        handler(.abort)
    }
}
