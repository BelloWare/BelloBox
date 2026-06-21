import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class ScreenshotPopupViewModelOCRTaskTests: XCTestCase {
    func testInitialActiveOCRResultIsShownInPanel() {
        let stitchResult = StitchResult(
            image: ScreenshotTestHelpers.image(width: 80, height: 60),
            placements: [],
            warnings: ["Frame 2 appears nearly unchanged from the previous frame."]
        )
        let document = ScrollCaptureCoordinator.makeDocument(
            from: stitchResult,
            target: ScrollCaptureTargetSummary(title: "Page", ownerName: nil, frame: nil),
            frameCount: 2,
            createdAt: Date(timeIntervalSince1970: 12)
        )

        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: AppSettings(defaults: temporaryDefaults("initial-active-ocr"))
        )

        XCTAssertEqual(viewModel.ocrPanel.result?.warnings, stitchResult.warnings)
    }

    func testRequestLLMOCRWithCodexShowsErrorWithoutConfirmation() {
        let defaults = temporaryDefaults("codex-llm-ocr")
        defaults.set(ProviderKind.codexCLI.rawValue, forKey: "provider")
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: AppSettings(defaults: defaults)
        )

        viewModel.requestLLMOCR()

        XCTAssertNil(viewModel.llmConfirmation)
        XCTAssertEqual(
            viewModel.ocrPanel.errorMessage,
            "Codex app-server does not support image OCR yet. Use Mac OCR or an image-capable HTTP provider."
        )
    }

    func testFailedLLMOCRRequestClearsStaleConfirmationAndError() {
        let openAIDefaults = temporaryDefaults("openai-llm-ocr")
        openAIDefaults.set(ProviderKind.openAI.rawValue, forKey: "provider")
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: AppSettings(defaults: openAIDefaults)
        )
        viewModel.ocrPanel.errorMessage = "Previous error"

        viewModel.requestLLMOCR()

        XCTAssertNotNil(viewModel.llmConfirmation)
        XCTAssertNil(viewModel.ocrPanel.errorMessage)

        let codexDefaults = temporaryDefaults("codex-clears-stale-llm-ocr")
        codexDefaults.set(ProviderKind.codexCLI.rawValue, forKey: "provider")
        let failingViewModel = ScreenshotPopupViewModel(
            document: viewModel.document,
            settings: AppSettings(defaults: codexDefaults)
        )
        failingViewModel.llmConfirmation = viewModel.llmConfirmation
        failingViewModel.ocrPanel.errorMessage = "Previous error"

        failingViewModel.requestLLMOCR()

        XCTAssertNil(failingViewModel.llmConfirmation)
        XCTAssertEqual(
            failingViewModel.ocrPanel.errorMessage,
            "Codex app-server does not support image OCR yet. Use Mac OCR or an image-capable HTTP provider."
        )
    }

    func testClosingEditorIgnoresPendingOCRResult() async {
        let service = ControllableOCRService()
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: AppSettings(defaults: temporaryDefaults()),
            macOCRService: service
        )

        viewModel.runMacOCR()
        await service.waitUntilStarted()
        XCTAssertTrue(viewModel.ocrPanel.isRunning)

        viewModel.close()
        service.succeed()
        await Task.yield()
        await Task.yield()

        XCTAssertFalse(viewModel.ocrPanel.isRunning)
        XCTAssertTrue(viewModel.document.ocrResults.isEmpty)
        XCTAssertNil(viewModel.document.activeOCRResultID)
    }

    func testCloseOnlyInvokesOnCloseOnce() {
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: AppSettings(defaults: temporaryDefaults("close-once"))
        )
        var closeCount = 0
        viewModel.onClose = { closeCount += 1 }

        viewModel.close()
        viewModel.close()

        XCTAssertEqual(closeCount, 1)
    }

    func testFinishOnlyInvokesOnCloseOnce() {
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: AppSettings(defaults: temporaryDefaults("finish-once"))
        )
        var closeCount = 0
        viewModel.onClose = { closeCount += 1 }

        viewModel.finish()
        viewModel.close()
        viewModel.finish()

        XCTAssertEqual(closeCount, 1)
    }

    func testFinishingEditorIgnoresPendingOCRResult() async {
        let service = ControllableOCRService()
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: AppSettings(defaults: temporaryDefaults()),
            macOCRService: service
        )

        viewModel.runMacOCR()
        await service.waitUntilStarted()
        XCTAssertTrue(viewModel.ocrPanel.isRunning)

        viewModel.finish()
        service.succeed()
        await Task.yield()
        await Task.yield()

        XCTAssertFalse(viewModel.ocrPanel.isRunning)
        XCTAssertTrue(viewModel.document.ocrResults.isEmpty)
        XCTAssertNil(viewModel.document.activeOCRResultID)
    }

    func testChangedDocumentRejectsPendingLLMOCRConfirmation() {
        let defaults = temporaryDefaults("stale-llm-confirmation")
        defaults.set(ProviderKind.openAI.rawValue, forKey: "provider")
        let service = ControllableOCRService()
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: AppSettings(defaults: defaults),
            llmOCRService: service
        )

        viewModel.requestLLMOCR()
        XCTAssertNotNil(viewModel.llmConfirmation)

        viewModel.addAnnotation(ScreenshotAnnotation(kind: .rectangle(CGRect(x: 4, y: 4, width: 10, height: 10))))
        viewModel.confirmLLMOCR()

        XCTAssertNil(viewModel.llmConfirmation)
        XCTAssertFalse(viewModel.ocrPanel.isRunning)
        XCTAssertEqual(viewModel.ocrPanel.errorMessage, OCRError.staleResult.localizedDescription)
        XCTAssertEqual(service.startedCount, 0)
    }

    func testConfirmedLLMOCRUsesApprovedConfigWhenSettingsChangeBeforeConfirm() async {
        let settings = AppSettings(defaults: temporaryDefaults("pinned-llm-config"))
        settings.openAIModel = "approved-model"
        settings.openAIBaseURL = "https://openai.example.com/v1"

        var approvedConfigs: [AIConfig] = []
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: settings,
            llmOCRServiceFactory: { config in
                approvedConfigs.append(config)
                return ImmediateOCRService()
            }
        )

        viewModel.requestLLMOCR()
        XCTAssertEqual(viewModel.llmConfirmation?.model, "approved-model")

        settings.openAIModel = "changed-model"
        settings.openAIBaseURL = "https://changed.example.com/v1"

        viewModel.confirmLLMOCR()
        await waitUntilOCRSettles(viewModel)

        let approvedConfig = try? XCTUnwrap(approvedConfigs.first)
        XCTAssertEqual(approvedConfig?.kind, .openAI)
        XCTAssertEqual(approvedConfig?.model, "approved-model")
        XCTAssertEqual(approvedConfig?.baseURL, "https://openai.example.com/v1")
        XCTAssertEqual(viewModel.document.activeOCRResult?.plainText, "approved")
    }

    func testEditingDuringPendingOCRDiscardsStaleResult() async {
        let service = ControllableOCRService()
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
                scale: 1,
                source: .importedClipboard
            ),
            settings: AppSettings(defaults: temporaryDefaults("stale-running-ocr")),
            macOCRService: service
        )

        viewModel.runMacOCR()
        await service.waitUntilStarted()
        XCTAssertTrue(viewModel.ocrPanel.isRunning)

        viewModel.addAnnotation(ScreenshotAnnotation(kind: .rectangle(CGRect(x: 4, y: 4, width: 10, height: 10))))
        service.succeed()
        await waitUntilOCRSettles(viewModel)

        XCTAssertFalse(viewModel.ocrPanel.isRunning)
        XCTAssertTrue(viewModel.document.ocrResults.isEmpty)
        XCTAssertNil(viewModel.document.activeOCRResultID)
        XCTAssertEqual(viewModel.ocrPanel.errorMessage, OCRError.staleResult.localizedDescription)
    }

    func testBaseCaptureRefreshRejectsPendingLLMOCRConfirmation() {
        let defaults = temporaryDefaults("refresh-stale-llm-confirmation")
        defaults.set(ProviderKind.openAI.rawValue, forKey: "provider")
        let service = ControllableOCRService()
        let document = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
            scale: 1,
            source: .window(title: "Old", ownerName: "App", windowID: 3)
        )
        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: AppSettings(defaults: defaults),
            llmOCRService: service
        )

        viewModel.requestLLMOCR()
        XCTAssertNotNil(viewModel.llmConfirmation)

        XCTAssertTrue(viewModel.refreshBaseCapture(
            from: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 120, height: 90),
                scale: 2,
                source: .window(title: "New", ownerName: "App", windowID: 3)
            ),
            expectedDocumentID: document.id
        ))
        viewModel.confirmLLMOCR()

        XCTAssertFalse(viewModel.ocrPanel.isRunning)
        XCTAssertEqual(viewModel.ocrPanel.errorMessage, OCRError.staleResult.localizedDescription)
        XCTAssertEqual(service.startedCount, 0)
    }

    func testBaseCaptureRefreshDuringPendingOCRDiscardsStaleResult() async {
        let service = ControllableOCRService()
        let document = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 80, height: 60),
            scale: 1,
            source: .window(title: "Old", ownerName: "App", windowID: 3)
        )
        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: AppSettings(defaults: temporaryDefaults("refresh-stale-running-ocr")),
            macOCRService: service
        )

        viewModel.runMacOCR()
        await service.waitUntilStarted()
        XCTAssertTrue(viewModel.ocrPanel.isRunning)

        XCTAssertTrue(viewModel.refreshBaseCapture(
            from: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 120, height: 90),
                scale: 2,
                source: .window(title: "New", ownerName: "App", windowID: 3)
            ),
            expectedDocumentID: document.id
        ))
        service.succeed()
        await waitUntilOCRSettles(viewModel)

        XCTAssertFalse(viewModel.ocrPanel.isRunning)
        XCTAssertTrue(viewModel.document.ocrResults.isEmpty)
        XCTAssertNil(viewModel.document.activeOCRResultID)
        XCTAssertEqual(viewModel.ocrPanel.errorMessage, OCRError.staleResult.localizedDescription)
    }

    private func temporaryDefaults(_ name: String = UUID().uuidString) -> UserDefaults {
        let defaults = UserDefaults(suiteName: "BelloBoxTests.ScreenshotPopupViewModelOCRTask.\(name)")!
        defaults.removePersistentDomain(forName: "BelloBoxTests.ScreenshotPopupViewModelOCRTask.\(name)")
        return defaults
    }

    private func waitUntilOCRSettles(_ viewModel: ScreenshotPopupViewModel) async {
        for _ in 0..<50 {
            if !viewModel.ocrPanel.isRunning { return }
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
    }
}

private final class ControllableOCRService: OCRService {
    private let lock = NSLock()
    private var recognitionContinuation: CheckedContinuation<OCRResult, Error>?
    private var startedContinuation: CheckedContinuation<Void, Never>?
    private var didStart = false
    private(set) var startedCount = 0

    func recognize(document: ScreenshotDocument, options: OCROptions) async throws -> OCRResult {
        return try await withCheckedThrowingContinuation { continuation in
            setRecognitionContinuation(continuation)
            markStarted()?.resume()
        }
    }

    func waitUntilStarted() async {
        if hasStarted { return }
        await withCheckedContinuation { continuation in
            setStartedContinuation(continuation)?.resume()
        }
    }

    func succeed() {
        takeRecognitionContinuation()?.resume(returning: OCRResult(
            id: UUID(),
            engine: .appleVision(revision: 3, recognitionLevel: .accurate),
            target: .fullImage,
            plainText: "ignored",
            markdownText: nil,
            regions: [],
            languageHints: [],
            imageDigest: "test",
            warnings: [],
            createdAt: Date()
        ))
    }

    private var hasStarted: Bool {
        lock.lock()
        let value = didStart
        lock.unlock()
        return value
    }

    private func markStarted() -> CheckedContinuation<Void, Never>? {
        lock.lock()
        didStart = true
        startedCount += 1
        let continuation = startedContinuation
        startedContinuation = nil
        lock.unlock()
        return continuation
    }

    private func setStartedContinuation(_ continuation: CheckedContinuation<Void, Never>) -> CheckedContinuation<Void, Never>? {
        lock.lock()
        if didStart {
            lock.unlock()
            return continuation
        }
        startedContinuation = continuation
        lock.unlock()
        return nil
    }

    private func setRecognitionContinuation(_ continuation: CheckedContinuation<OCRResult, Error>) {
        lock.lock()
        recognitionContinuation = continuation
        lock.unlock()
    }

    private func takeRecognitionContinuation() -> CheckedContinuation<OCRResult, Error>? {
        lock.lock()
        let continuation = recognitionContinuation
        recognitionContinuation = nil
        lock.unlock()
        return continuation
    }
}

private final class ImmediateOCRService: OCRService {
    func recognize(document: ScreenshotDocument, options: OCROptions) async throws -> OCRResult {
        OCRResult(
            id: UUID(),
            engine: .llm(provider: .openAI, model: "approved-model"),
            target: options.target,
            plainText: "approved",
            markdownText: nil,
            regions: [],
            languageHints: options.languageHints,
            imageDigest: "test",
            warnings: [],
            createdAt: Date()
        )
    }
}
