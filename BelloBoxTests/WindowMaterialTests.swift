import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

@MainActor
final class WindowMaterialTests: XCTestCase {
    private var suite: String!
    private var defaults: UserDefaults!
    override func setUp() {
        super.setUp()
        suite = "WindowMaterials.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)!
    }
    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
        super.tearDown()
    }

    func testSurfaceChoicePersistsIndependentlyOfLightAndDark() {
        let settings = AppSettings(defaults: defaults)
        XCTAssertEqual(settings.windowSurfaceStyle, .glass)
        settings.appearance = .dark
        settings.windowSurfaceStyle = .solid
        let restored = AppSettings(defaults: defaults)
        XCTAssertEqual(restored.windowSurfaceStyle, .solid)
        XCTAssertEqual(restored.appearance, .dark)
        restored.appearance = .light
        XCTAssertEqual(restored.windowSurfaceStyle, .solid)
        defaults.set("unknown", forKey: "windowSurfaceStyle")
        XCTAssertEqual(AppSettings(defaults: defaults).windowSurfaceStyle, .glass)
    }

    func testAccessibilityPreferencesAlwaysOverrideGlassWithoutChangingTheChoice() {
        for style in WindowSurfaceStyle.allCases {
            for transparency in [true, false] {
                for contrast in [true, false] {
                    let policy = WindowMaterialPolicy(style: style, reduceTransparency: transparency, increasedContrast: contrast)
                    XCTAssertEqual(policy.usesGlass, style == .glass && !transparency && !contrast)
                    XCTAssertEqual(policy.style, style)
                }
            }
        }
    }

    func testNativeBackdropCannotInterceptMouseOrKeyboardInput() {
        let view = WindowMaterialView(role: .popup)
        view.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        XCTAssertNil(view.hitTest(NSPoint(x: 120, y: 80)))
        XCTAssertFalse(view.acceptsFirstResponder)
        XCTAssertFalse(view.isAccessibilityElement())
        XCTAssertEqual(view.blendingMode, .behindWindow)
        XCTAssertEqual(view.state, .active, "Non-activating toolbar stays readable")
        view.configure(role: .workspace)
        XCTAssertEqual(view.state, .followsWindowActiveState)
        XCTAssertEqual(view.material, .underWindowBackground)
    }

    func testNestedCardsUseOneBackdropAndSolidUsesNone() async throws {
        let settings = AppSettings(defaults: defaults)
        let content = VStack {
            Text("Nested tool").frame(maxWidth: .infinity).padding().popupCard()
        }.padding().popupCard().windowSurfacePreferences(settings)
        let expected = NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency || NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast ? 0 : 1
        let window = host(content)
        defer { window.close() }
        try await settle(window)
        XCTAssertEqual(materials(in: window.contentView!).count, expected)
        let original = materials(in: window.contentView!).first
        settings.appearance = .dark
        try await settle(window)
        XCTAssertTrue(materials(in: window.contentView!).first === original, "Unrelated settings don't recreate the effect")
        settings.windowSurfaceStyle = .solid
        try await settle(window)
        XCTAssertEqual(materials(in: window.contentView!).count, 0)
        settings.windowSurfaceStyle = .glass
        try await settle(window)
        XCTAssertEqual(materials(in: window.contentView!).count, expected)
    }

    func testAccessibilityFallbackActuallyRemovesTheNativeEffectInBothThemes() async throws {
        for dark in [false, true] {
            for reducedTransparency in [false, true] {
                let content = Text("Readable content").padding()
                    .background(WorkspaceBackground(policyOverride: WindowMaterialPolicy(style: .glass, reduceTransparency: reducedTransparency, increasedContrast: !reducedTransparency)))
                    .environment(\.colorScheme, dark ? .dark : .light)
                let window = host(content)
                try await settle(window)
                XCTAssertTrue(materials(in: window.contentView!).isEmpty)
                window.close()
            }
        }
    }

    func testChangingBackgroundPreservesNativeEditorFocusSelectionAndUndo() async throws {
        let settings = AppSettings(defaults: defaults)
        let draft = Draft()
        let window = host(EditorFixture(draft: draft).popupCard().windowSurfacePreferences(settings))
        defer { window.close() }
        try await settle(window)
        let editor = try XCTUnwrap(find(LiteralTextView.self, in: window.contentView!))
        window.makeFirstResponder(editor)
        editor.insertText(" edited", replacementRange: NSRange(location: editor.string.utf16.count, length: 0))
        editor.breakUndoCoalescing()
        let selection = NSRange(location: 2, length: 3)
        editor.setSelectedRange(selection)
        for style in [WindowSurfaceStyle.solid, .glass, .solid] {
            settings.windowSurfaceStyle = style
            try await settle(window)
            XCTAssertTrue(find(LiteralTextView.self, in: window.contentView!) === editor)
            XCTAssertTrue(window.firstResponder === editor)
            XCTAssertEqual(editor.selectedRange(), selection)
            XCTAssertEqual(draft.text, "A literal draft edited")
        }
        XCTAssertTrue(editor.undoManager?.canUndo == true)
        editor.undoManager?.undo()
        XCTAssertEqual(draft.text, "A literal draft")
    }

    private final class Draft: ObservableObject { @Published var text = "A literal draft" }
    private struct EditorFixture: View {
        @ObservedObject var draft: Draft
        var body: some View {
            LiteralTextEditor(text: $draft.text, label: "Draft", focusesWhenAttached: true)
                .padding(12).background(BoxTheme.well).padding(16)
        }
    }
    private func host<V: View>(_ content: V) -> NSWindow {
        let window = PopupPanel(contentRect: CGRect(x: 100, y: 100, width: 440, height: 260))
        window.contentView = NSHostingView(rootView: content)
        window.makeKeyAndOrderFront(nil)
        return window
    }
    private func settle(_ window: NSWindow) async throws {
        try await Task.sleep(nanoseconds: 80_000_000)
        window.contentView?.layoutSubtreeIfNeeded()
    }
    private func materials(in view: NSView) -> [WindowMaterialView] {
        (view as? WindowMaterialView).map { [$0] } ?? view.subviews.flatMap { materials(in: $0) }
    }
    private func find<T: NSView>(_ type: T.Type, in view: NSView) -> T? {
        if let match = view as? T { return match }
        return view.subviews.lazy.compactMap { self.find(type, in: $0) }.first
    }
}
