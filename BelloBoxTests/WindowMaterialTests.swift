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
        XCTAssertEqual(view.material, .hudWindow)
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

    func testWorkspaceBackdropCoversTitlebarAndViewportWithoutDuplicatingBlur() async throws {
        let settings = AppSettings(defaults: defaults)
        let content = ToolViewport(minimumSize: CGSize(width: 400, height: 240)) {
            EditorFixture(draft: Draft()).popupCard().windowSurfacePreferences(settings)
        }.workspaceBackground().windowSurfacePreferences(settings)
        let window = NSWindow(contentViewController: NSHostingController(rootView: content))
        AppWindowChrome.apply(to: window, title: "Glass workspace")
        AppWindowChrome.size(window, content: CGSize(width: 440, height: 280), minimum: CGSize(width: 400, height: 240))
        window.makeKeyAndOrderFront(nil)
        defer { window.close() }
        try await settle(window)
        XCTAssertEqual(window.contentLayoutRect.height, 280, accuracy: 1, "The title bar must not steal usable tool height")
        let native = materials(in: window.contentView!)
        if !NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency && !NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast {
            XCTAssertEqual(native.count, 1, "Nested preference scopes must share the window's backdrop")
            let material = try XCTUnwrap(native.first)
            let bounds = material.convert(material.bounds, to: nil)
            XCTAssertEqual(bounds.minY, 0, accuracy: 1)
            XCTAssertEqual(bounds.maxY, window.frame.height, accuracy: 1, "The title bar and scroll edges must be frosted too")
        }
        settings.windowSurfaceStyle = .solid
        try await settle(window)
        XCTAssertTrue(materials(in: window.contentView!).isEmpty)
        XCTAssertEqual(window.contentLayoutRect.height, 280, accuracy: 1)
        XCTAssertNotNil(window.standardWindowButton(.closeButton))
        XCTAssertNotNil(window.standardWindowButton(.miniaturizeButton))
        XCTAssertNotNil(window.standardWindowButton(.zoomButton))
    }

    func testGlassCardsAndFieldsKeepTheirForegroundOpaque() throws {
        for scheme in [ColorScheme.light, .dark] {
            for role in [ToolSurfaceRole.card, .control, .input, .output] {
                for style in WindowSurfaceStyle.allCases {
                    let renderer = ImageRenderer(content: Rectangle().fill(.white).frame(width: 12, height: 12)
                        .frame(width: 80, height: 60).toolSurface(role)
                        .environment(\.windowSurfaceStyle, style).environment(\.colorScheme, scheme))
                    let bitmap = NSBitmapImageRep(cgImage: try XCTUnwrap(renderer.cgImage))
                    let foreground = try XCTUnwrap(bitmap.colorAt(x: 40, y: 30))
                    let backing = try XCTUnwrap(bitmap.colorAt(x: 20, y: 20))
                    XCTAssertEqual(foreground.alphaComponent, 1, accuracy: 0.01, "Glass must never fade the content itself")
                    if style == .solid {
                        XCTAssertEqual(backing.alphaComponent, 1, accuracy: 0.01)
                    } else {
                        XCTAssertGreaterThan(backing.alphaComponent, 0.05)
                        XCTAssertLessThan(backing.alphaComponent, 0.40, "Cards and editors must not flatten the glass")
                    }
                }
            }
        }
    }

    func testGlassSelectionMenuKeepsNativeCheckmarksAndBinding() async throws {
        let draft = MenuDraft()
        let window = host(MenuFixture(draft: draft).padding().popupCard())
        defer { window.close() }
        try await settle(window)
        let button = try XCTUnwrap(find(NSPopUpButton.self, in: window.contentView!))
        // SwiftUI fills its native menu lazily on open. Exercise the real
        // tracking menu instead of expecting items in a closed menu.
        var selectedItemWasChecked = false
        var performedSelection = false
        let timer = Timer(timeInterval: 0.2, repeats: false) { _ in
            MainActor.assumeIsolated {
                guard let menu = button.menu else { return }
                selectedItemWasChecked = menu.items.first(where: { $0.title == "Alpha" })?.state == .on
                if let index = menu.items.firstIndex(where: { $0.title == "Beta" }) {
                    menu.performActionForItem(at: index)
                    performedSelection = true
                }
                menu.cancelTrackingWithoutAnimation()
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        button.performClick(nil)
        timer.invalidate()
        try await settle(window)
        XCTAssertTrue(selectedItemWasChecked)
        XCTAssertTrue(performedSelection)
        XCTAssertEqual(draft.choice, "b")
    }

    private final class MenuDraft: ObservableObject { @Published var choice = "a" }
    private struct MenuFixture: View {
        @ObservedObject var draft: MenuDraft
        var body: some View {
            ToolMenuPicker("Example", value: draft.choice == "a" ? "Alpha" : "Beta", selection: $draft.choice) {
                Text("Alpha").tag("a")
                Text("Beta").tag("b")
            }
        }
    }

    private final class Draft: ObservableObject { @Published var text = "A literal draft" }
    private struct EditorFixture: View {
        @ObservedObject var draft: Draft
        var body: some View {
            LiteralTextEditor(text: $draft.text, label: "Draft", focusesWhenAttached: true)
                .padding(12).toolSurface(.input).padding(16)
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
