import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

@MainActor
final class TextToolsPopupViewTests: XCTestCase {
    private var defaults: UserDefaults!
    private var suiteName = ""

    override func setUp() {
        super.setUp()
        suiteName = "BelloBoxTests.text-tools-view-\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suiteName)
        super.tearDown()
    }

    private static func descendants(of view: NSView) -> [NSView] {
        view.subviews.flatMap { [$0] + descendants(of: $0) }
    }

    /// ~490 KB of short mixed-case lines, like the Rust comparison's fixture.
    private static func largeLines() -> String {
        let words = ["alpha", "Beta", "gamma", "Delta", "café", "naïve", "ZEBRA", "zebra", "日本語", "emoji 😀", "  padded  "]
        var lines: [String] = []
        var bytes = 0
        var seed: UInt64 = 20_261_010
        func next(_ bound: Int) -> Int {
            seed = seed &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
            return Int((seed >> 33) % UInt64(bound))
        }
        while bytes < 490_000 {
            let line = (0...next(6)).map { _ in words[next(words.count)] }.joined(separator: " ") + " #\(next(3_000))"
            bytes += line.utf8.count + 1
            lines.append(line)
        }
        return lines.joined(separator: "\n")
    }

    private func window(_ viewModel: TextToolsPopupViewModel) -> NSWindow {
        let size = TextToolsPopupView.preferredSize
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: TextToolsPopupView(viewModel: viewModel))
        return window
    }

    func testLargeResultsAppearPromptlyInANativeTextView() throws {
        // 0.0.77 drew the result as one SwiftUI Text with text selection, laid out
        // whole on the main thread, so a large Lines or Case result stalled.
        let input = Self.largeLines()
        XCTAssertGreaterThan(input.utf8.count, 480_000)
        for (category, prepare) in [(TextToolsPopupViewModel.Category.lines, { (m: TextToolsPopupViewModel) in m.lineOp = .sortAscending }),
                                    (.caseConvert, { (m: TextToolsPopupViewModel) in m.caseStyle = .upper })] {
            let viewModel = TextToolsPopupViewModel(selection: TextSelection(text: input, anchorRect: nil, appName: nil, bundleID: nil, pid: nil),
                                                    settings: AppSettings(defaults: defaults), accessibility: AccessibilityService())
            prepare(viewModel)
            viewModel.category = category
            let start = Date()
            let window = window(viewModel)
            defer { window.close() }
            window.contentView?.layoutSubtreeIfNeeded()
            window.displayIfNeeded()
            let elapsed = Date().timeIntervalSince(start)
            let shown = Self.descendants(of: try XCTUnwrap(window.contentView)).compactMap { $0 as? LauncherOutputTextView }
            XCTAssertEqual(shown.count, 1, "\(category): the result is shown by one native text view")
            XCTAssertTrue(shown.first?.string == viewModel.primaryOutput, "\(category): it holds the complete result")
            XCTAssertEqual(shown.first?.isEditable, false)
            XCTAssertLessThan(elapsed, 3, "\(category): laying out and drawing the result took \(elapsed) s")
        }
    }
}
