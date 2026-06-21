import XCTest
@testable import BelloBox

@MainActor
final class ScrollCaptureCoordinatorTests: XCTestCase {
    func testCancelledFrameCaptureDoesNotAppendReturnedFrame() async {
        let service = ControllableScreenshotCaptureService()
        let coordinator = makeCoordinator(service: service)

        let task = Task {
            try await coordinator.captureNextFrame()
        }
        await waitUntil(service.requestCount == 1)

        task.cancel()
        service.completeRequest(at: 0, with: .success(testDocument()))

        do {
            try await task.value
            XCTFail("Expected cancellation")
        } catch is CancellationError {
            // Expected.
        } catch {
            XCTFail("Expected CancellationError, got \(error)")
        }

        XCTAssertTrue(coordinator.session.frames.isEmpty)
        XCTAssertEqual(coordinator.session.status, .idle)
    }

    func testCancelledSecondFrameCaptureKeepsExistingFrames() async throws {
        let service = ControllableScreenshotCaptureService()
        let coordinator = makeCoordinator(service: service)

        let firstTask = Task { try await coordinator.captureNextFrame() }
        await waitUntil(service.requestCount == 1)
        service.completeRequest(at: 0, with: .success(testDocument(width: 80, height: 80)))
        try await firstTask.value
        XCTAssertEqual(coordinator.session.frames.count, 1)

        let secondTask = Task { try await coordinator.captureNextFrame() }
        await waitUntil(service.requestCount == 2)
        secondTask.cancel()
        service.completeRequest(at: 1, with: .success(testDocument(width: 80, height: 80)))

        do {
            try await secondTask.value
            XCTFail("Expected cancellation")
        } catch is CancellationError {
            // Expected.
        } catch {
            XCTFail("Expected CancellationError, got \(error)")
        }

        XCTAssertEqual(coordinator.session.frames.count, 1)
        XCTAssertEqual(coordinator.session.status, .waitingForScroll)
    }

    func testCancelledFinishRestoresWaitingState() async {
        let service = ControllableScreenshotCaptureService()
        let coordinator = makeCoordinator(service: service)
        coordinator.session.frames = [
            ScrollCapturedFrame(image: testDocument(width: 80, height: 80).baseImage, targetRect: .zero),
            ScrollCapturedFrame(image: testDocument(width: 80, height: 80).baseImage, targetRect: .zero),
        ]
        coordinator.session.status = .waitingForScroll

        let task = Task {
            try await coordinator.finish()
        }
        task.cancel()

        do {
            _ = try await task.value
            XCTFail("Expected cancellation")
        } catch is CancellationError {
            // Expected.
        } catch {
            XCTFail("Expected CancellationError, got \(error)")
        }

        XCTAssertEqual(coordinator.session.status, .waitingForScroll)
    }

    func testHUDCancelOnlyInvokesOnCancelOnce() {
        let service = ControllableScreenshotCaptureService()
        let viewModel = ScrollingCaptureHUDViewModel(coordinator: makeCoordinator(service: service))
        var cancelCount = 0
        viewModel.onCancel = { cancelCount += 1 }

        viewModel.cancel()
        viewModel.cancel()

        XCTAssertEqual(cancelCount, 1)
    }

    private func makeCoordinator(service: ScreenshotCapturing) -> ScrollCaptureCoordinator {
        let suiteName = "BelloBoxTests.ScrollCaptureCoordinator.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return ScrollCaptureCoordinator(
            target: .area(CaptureArea(cocoaRect: CGRect(x: 10, y: 20, width: 80, height: 80), displayID: nil)),
            service: service,
            settings: AppSettings(defaults: defaults)
        )
    }

    private func testDocument(width: Int = 80, height: Int = 80) -> ScreenshotDocument {
        ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: width, height: height),
            scale: 1,
            source: .importedClipboard
        )
    }

    private func waitUntil(
        _ predicate: @autoclosure @escaping @MainActor () -> Bool,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        for _ in 0..<100 {
            if predicate() { return }
            try? await Task.sleep(nanoseconds: 1_000_000)
        }
        XCTFail("Timed out waiting for condition", file: file, line: line)
    }
}

@MainActor
private final class ControllableScreenshotCaptureService: ScreenshotCapturing {
    private var continuations: [CheckedContinuation<ScreenshotDocument, Error>] = []

    var requestCount: Int { continuations.count }

    func capture(_ target: CaptureTarget, options: CaptureOptions) async throws -> ScreenshotDocument {
        try await withCheckedThrowingContinuation { continuation in
            continuations.append(continuation)
        }
    }

    func completeRequest(at index: Int, with result: Result<ScreenshotDocument, Error>) {
        switch result {
        case let .success(document):
            continuations[index].resume(returning: document)
        case let .failure(error):
            continuations[index].resume(throwing: error)
        }
    }
}
