import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

@MainActor
final class DialogPresentationTests: XCTestCase {
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
        XCTAssertTrue(WindowDialogPresenter.ownsKeyboard)
        owner.endSheet(panel, returnCode: response)
        try await waitUntil { result != nil }
        XCTAssertEqual(result, response)
        XCTAssertEqual(panel.level, originalLevel)
        XCTAssertEqual(owner.level, .screenSaver)
        XCTAssertEqual(otherShield.level.rawValue, NSWindow.Level.screenSaver.rawValue + 1)
        XCTAssertTrue(owner.isVisible && otherShield.isVisible)
        XCTAssertFalse(WindowDialogPresenter.ownsKeyboard)
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
    }

    func testClosedOwnerCancelsWithoutOpeningAnOrphanDialog() {
        let owner = makeWindow(level: .screenSaver)
        close(owner)
        let panel = NSSavePanel()
        var response: NSApplication.ModalResponse?
        WindowDialogPresenter.present(panel, for: owner) { response = $0 }
        XCTAssertEqual(response, .cancel)
        XCTAssertFalse(panel.isVisible)
        XCTAssertFalse(WindowDialogPresenter.ownsKeyboard)
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
        XCTAssertFalse(WindowDialogPresenter.ownsKeyboard)
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
            try await waitUntil { result != nil && !WindowDialogPresenter.ownsKeyboard }
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
        XCTAssertFalse(cancelled)
        XCTAssertFalse(controller.debugOverlayWindows.isEmpty)
        XCTAssertEqual(owner.level, .screenSaver)
        XCTAssertTrue(owner.isVisible)
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
            try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard && controller.isSearchFieldFocused }
        XCTAssertTrue(controller.isVisible)
        XCTAssertEqual(owner.level, .floating)
        qr.save(in: owner)
        let pending = try await savePanel(in: owner)
        controller.close()
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
            try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
        try await waitUntil { !WindowDialogPresenter.ownsKeyboard }
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
