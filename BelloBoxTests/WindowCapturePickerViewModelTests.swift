import XCTest
@testable import BelloBox

@MainActor
final class WindowCapturePickerViewModelTests: XCTestCase {
    func testReloadIgnoresStaleWindowList() async {
        let provider = WindowProviderStub()
        let viewModel = WindowCapturePickerViewModel(service: provider)

        viewModel.load()
        await waitUntil(provider.requestCount == 1)
        viewModel.load()
        await waitUntil(provider.requestCount == 2)

        let stale = CaptureWindow(windowID: 1, title: "Stale", ownerName: "Old", ownerBundleID: nil, ownerProcessID: nil, frame: nil)
        provider.completeRequest(at: 0, with: .success([stale]))
        try? await Task.sleep(nanoseconds: 5_000_000)

        XCTAssertTrue(viewModel.windows.isEmpty)
        XCTAssertTrue(viewModel.isLoading)

        let fresh = CaptureWindow(windowID: 2, title: "Fresh", ownerName: "Current", ownerBundleID: nil, ownerProcessID: nil, frame: nil)
        provider.completeRequest(at: 1, with: .success([fresh]))
        await waitUntil(!viewModel.isLoading)

        XCTAssertEqual(viewModel.windows, [fresh])
        XCTAssertNil(viewModel.errorMessage)
    }

    func testEmptyResultShowsNoWindowsMessage() async {
        let provider = WindowProviderStub()
        let viewModel = WindowCapturePickerViewModel(service: provider)

        viewModel.load()
        await waitUntil(provider.requestCount == 1)
        provider.completeRequest(at: 0, with: .success([]))
        await waitUntil(!viewModel.isLoading)

        XCTAssertTrue(viewModel.windows.isEmpty)
        XCTAssertEqual(viewModel.errorMessage, "No capturable windows found.")
    }

    func testLoadErrorClearsLoadingState() async {
        let provider = WindowProviderStub()
        let viewModel = WindowCapturePickerViewModel(service: provider)

        viewModel.load()
        await waitUntil(provider.requestCount == 1)
        provider.completeRequest(at: 0, with: .failure(ScreenCaptureService.CaptureError.noWindowFound))
        await waitUntil(!viewModel.isLoading)

        XCTAssertTrue(viewModel.windows.isEmpty)
        XCTAssertEqual(viewModel.errorMessage, ScreenCaptureService.CaptureError.noWindowFound.localizedDescription)
    }

    func testCancelStopsLoadingAndIgnoresLateResult() async {
        let provider = WindowProviderStub()
        let viewModel = WindowCapturePickerViewModel(service: provider)
        var cancelCount = 0
        viewModel.onCancel = { cancelCount += 1 }

        viewModel.load()
        await waitUntil(provider.requestCount == 1)
        XCTAssertTrue(viewModel.isLoading)

        viewModel.cancel()
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertEqual(cancelCount, 1)

        let stale = CaptureWindow(windowID: 4, title: "Late", ownerName: "Old", ownerBundleID: nil, ownerProcessID: nil, frame: nil)
        provider.completeRequest(at: 0, with: .success([stale]))
        try? await Task.sleep(nanoseconds: 5_000_000)

        XCTAssertTrue(viewModel.windows.isEmpty)
        XCTAssertNil(viewModel.errorMessage)
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
private final class WindowProviderStub: CapturableWindowProviding {
    private var continuations: [CheckedContinuation<[CaptureWindow], Error>] = []

    var requestCount: Int { continuations.count }

    func capturableWindows() async throws -> [CaptureWindow] {
        try await withCheckedThrowingContinuation { continuation in
            continuations.append(continuation)
        }
    }

    func completeRequest(at index: Int, with result: Result<[CaptureWindow], Error>) {
        switch result {
        case let .success(windows):
            continuations[index].resume(returning: windows)
        case let .failure(error):
            continuations[index].resume(throwing: error)
        }
    }
}
