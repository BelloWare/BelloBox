import XCTest
@testable import BelloBox

@MainActor
final class ActionPopupViewModelTests: XCTestCase {
    func testRunWithMissingConfigurationClearsStaleStateAndShowsCopyableError() {
        let settings = AppSettings(defaults: temporaryDefaults())
        settings.openAIModel = ""
        let viewModel = ActionPopupViewModel(
            selection: TextSelection(text: "selected text", anchorRect: nil, appName: nil, bundleID: nil, pid: nil),
            settings: settings,
            client: AIClient(),
            accessibility: AccessibilityService()
        )
        viewModel.resultText = "stale output"
        viewModel.isStreaming = true

        viewModel.run(QuickAction.library[0])

        XCTAssertEqual(viewModel.resultText, "")
        XCTAssertFalse(viewModel.isStreaming)
        XCTAssertTrue(viewModel.didRun)
        XCTAssertNotNil(viewModel.errorMessage)
        XCTAssertEqual(viewModel.copyableText, viewModel.errorMessage)
        XCTAssertFalse(viewModel.canReplace)
        XCTAssertTrue(viewModel.canCopy)
    }

    func testProviderErrorIsShownAfterStreamingTaskFinishes() async {
        let settings = AppSettings(defaults: temporaryDefaults("provider-error"))
        settings.openAIBaseURL = "https://api.example.com/v1"
        settings.openAIModel = "m-1"
        settings.apiKey = "sk-test"
        let client = clientReturning(status: 500, body: #"{"error":{"message":"bad key"}}"#)
        let viewModel = ActionPopupViewModel(
            selection: TextSelection(text: "selected text", anchorRect: nil, appName: nil, bundleID: nil, pid: nil),
            settings: settings,
            client: client,
            accessibility: AccessibilityService()
        )

        viewModel.run(QuickAction.library[0])
        await waitUntilFinished(viewModel)

        XCTAssertFalse(viewModel.isStreaming)
        XCTAssertEqual(viewModel.errorMessage, "The provider returned HTTP 500. bad key")
        XCTAssertEqual(viewModel.copyableText, viewModel.errorMessage)
        XCTAssertTrue(viewModel.canCopy)
        XCTAssertFalse(viewModel.canReplace)
    }

    private func temporaryDefaults(_ name: String = UUID().uuidString) -> UserDefaults {
        let suiteName = "BelloBoxTests.ActionPopupViewModel.\(name)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }

    private func waitUntilFinished(_ viewModel: ActionPopupViewModel) async {
        for _ in 0..<50 {
            if !viewModel.isStreaming { return }
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
    }

    private func clientReturning(status: Int, body: String) -> AIClient {
        ActionPopupMockURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url ?? URL(string: "https://api.example.com/v1/chat/completions")!,
                statusCode: status,
                httpVersion: "HTTP/1.1",
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data(body.utf8))
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ActionPopupMockURLProtocol.self]
        return AIClient(session: URLSession(configuration: configuration))
    }
}

private final class ActionPopupMockURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: AIError.transport("No mock response was configured."))
            return
        }

        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}
