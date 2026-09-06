import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class WorkspaceNavigationTests: XCTestCase {
    func testEveryToolCanBeFoundFromHomeWithoutUsingThePalette() {
        let tools = Set(HomeCategory.allCases.flatMap(\.commands))
        XCTAssertEqual(tools, Set(LauncherCommand.allCases.filter { $0 != .home && $0 != .settings }))
        for category in HomeCategory.allCases {
            XCTAssertEqual(category.commands.count, Set(category.commands).count)
        }
    }

    func testHomeShowsARegularWindowAndOnlyOpensToolsWhenRequested() throws {
        let name = "WorkspaceNavigation.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defer { defaults.removePersistentDomain(forName: name) }
        let settings = AppSettings(defaults: defaults)
        let controller = MainWindowController()
        var launches = 0
        let action = { launches += 1 }
        controller.show(settings: settings, canCheckForUpdates: false, onOpenSettings: action,
            onOpenGuide: action, onOpenLauncher: action, onCapture: action, onScrollCapture: action,
            onRecord: action, onOpenQR: action, onOpenTextTools: action, onOpenWorldClock: action,
            onCheckForUpdates: action, onOpenTool: { _ in launches += 1 })
        let window = try XCTUnwrap(controller.window)
        defer { window.close() }
        XCTAssertTrue(window.styleMask.contains(.titled))
        XCTAssertTrue(window.styleMask.contains(.resizable))
        XCTAssertEqual(launches, 0)
        controller.hide()
        XCTAssertFalse(window.isVisible)
        controller.show(settings: settings, canCheckForUpdates: false, onOpenSettings: action,
            onOpenGuide: action, onOpenLauncher: action, onCapture: action, onScrollCapture: action,
            onRecord: action, onOpenQR: action, onOpenTextTools: action, onOpenWorldClock: action,
            onCheckForUpdates: action)
        XCTAssertTrue(window.isVisible)
        XCTAssertEqual(launches, 0)
    }

    func testPageNavigationStaysValidForShortFilteredLists() {
        let name = "WorkspacePages.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defer { defaults.removePersistentDomain(forName: name) }
        let selection = TextSelection(text: "", anchorRect: nil, appName: nil, bundleID: nil, pid: nil)
        let model = LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults)
        model.query = "json"
        for direction in [-7, 7, -14, 14] {
            model.move(direction)
            XCTAssertNotNil(model.selectedCommand)
        }
        model.query = "regex"
        model.move(-7)
        XCTAssertEqual(model.selectedCommand, .regex)
    }
}
