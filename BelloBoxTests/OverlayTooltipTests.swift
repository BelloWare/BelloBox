import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class OverlayTooltipTests: XCTestCase {
    override func setUp() {
        super.setUp()
        OverlayTooltipPresenter.shared.exclusionRect = nil
        OverlayTooltipPresenter.shared.hide()
    }

    func testTooltipKeepsClearOfTheExcludedRegion() throws {
        let screen = CGRect(x: 0, y: 0, width: 1440, height: 900)
        let size = CGSize(width: 300, height: 42)
        let pointer = CGPoint(x: 700, y: 300)

        // Nothing excluded: centred above the pointer.
        XCTAssertEqual(
            OverlayTooltipPresenter.origin(for: size, near: pointer, avoiding: nil, visibleFrame: screen),
            CGPoint(x: 550, y: 318)
        )

        // The sampled region sits right above the HUD: the tooltip flips below the pointer.
        let region = CGRect(x: 200, y: 340, width: 1000, height: 400)
        let below = try XCTUnwrap(OverlayTooltipPresenter.origin(for: size, near: pointer, avoiding: region, visibleFrame: screen))
        XCTAssertEqual(below, CGPoint(x: 550, y: 240))
        XCTAssertFalse(CGRect(origin: below, size: size).intersects(region))

        // The HUD sits beside the region: the tooltip is pushed sideways past it.
        let sideRegion = CGRect(x: 100, y: 100, width: 600, height: 700)
        let side = try XCTUnwrap(
            OverlayTooltipPresenter.origin(for: size, near: CGPoint(x: 760, y: 450), avoiding: sideRegion, visibleFrame: screen)
        )
        XCTAssertFalse(CGRect(origin: side, size: size).intersects(sideRegion.insetBy(dx: -6, dy: -6)))
        XCTAssertGreaterThanOrEqual(side.x, sideRegion.maxX + 6)

        // The region fills the screen around the pointer: nowhere is safe, so no tooltip.
        XCTAssertNil(
            OverlayTooltipPresenter.origin(for: size, near: pointer, avoiding: screen.insetBy(dx: 10, dy: 10), visibleFrame: screen)
        )
    }

    func testTooltipInsideTheExcludedRegionIsNotShown() {
        let presenter = OverlayTooltipPresenter.shared
        let screen = NSScreen.screens.first?.frame ?? CGRect(x: 0, y: 0, width: 1440, height: 900)
        presenter.exclusionRect = screen.insetBy(dx: 10, dy: 10)
        presenter.showImmediately("Stop scrolling automatically", at: CGPoint(x: screen.midX, y: screen.midY))
        XCTAssertNil(presenter.visibleText)
        XCTAssertTrue(presenter.debugPanel?.isVisible != true)
        presenter.exclusionRect = nil
    }

    func testHideAndUpdateFromAnotherOwnerLeaveTheTooltipAlone() {
        let presenter = OverlayTooltipPresenter.shared
        let owner = UUID()
        presenter.show("Stop scrolling automatically", owner: owner, delay: 0)
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        XCTAssertEqual(presenter.visibleText, "Stop scrolling automatically")

        // A sibling view disappearing, or refreshing its own text, must not touch it.
        presenter.hide(owner: UUID())
        presenter.update("Ignored", owner: UUID())
        XCTAssertEqual(presenter.visibleText, "Stop scrolling automatically")

        presenter.update("Scroll the content automatically", owner: owner)
        XCTAssertEqual(presenter.visibleText, "Scroll the content automatically")
        presenter.hide(owner: owner)
        XCTAssertNil(presenter.visibleText)
    }

    func testTooltipShowsAboveOverlayLevelsAndHides() {
        let presenter = OverlayTooltipPresenter.shared
        presenter.showImmediately("Scroll to capture more", at: CGPoint(x: 300, y: 300))

        let panel = try? XCTUnwrap(presenter.debugPanel)
        XCTAssertNotNil(panel)
        XCTAssertEqual(presenter.visibleText, "Scroll to capture more")
        XCTAssertTrue(panel?.isVisible == true)
        XCTAssertGreaterThan(panel?.level.rawValue ?? 0, NSWindow.Level.screenSaver.rawValue + 1)
        XCTAssertFalse(panel?.styleMask.contains(.nonactivatingPanel) == false)

        presenter.hide()
        XCTAssertNil(presenter.visibleText)
        XCTAssertTrue(panel?.isVisible == false)
    }

    func testDelayedTooltipIsCancelledByHide() {
        let presenter = OverlayTooltipPresenter.shared
        presenter.show("Pen: draw freehand", delay: 0.05)
        presenter.hide()
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        XCTAssertNil(presenter.visibleText)
    }

    func testUpdateReplacesTheVisibleTextInPlace() {
        let presenter = OverlayTooltipPresenter.shared
        presenter.showImmediately("Line width: 3 px", at: CGPoint(x: 400, y: 400))

        presenter.update("Line width: 8 px")
        XCTAssertEqual(presenter.visibleText, "Line width: 8 px")
        XCTAssertTrue(presenter.debugPanel?.isVisible == true)

        presenter.hide()
        presenter.update("Line width: 9 px")
        XCTAssertNil(presenter.visibleText)
        XCTAssertTrue(presenter.debugPanel?.isVisible == false)
    }

    func testUpdateBeforeTheDelayElapsesShowsTheNewText() {
        let presenter = OverlayTooltipPresenter.shared
        presenter.show("Auto-scroll", delay: 0.05)
        presenter.update("Stop")
        RunLoop.main.run(until: Date().addingTimeInterval(0.6))
        XCTAssertEqual(presenter.visibleText, "Stop")
        presenter.hide()
    }

    func testMouseDownDismissesTheTooltip() throws {
        let presenter = OverlayTooltipPresenter.shared
        presenter.showImmediately("Pen: draw freehand", at: CGPoint(x: 300, y: 300))
        XCTAssertEqual(presenter.visibleText, "Pen: draw freehand")

        let click = try XCTUnwrap(
            NSEvent.mouseEvent(
                with: .leftMouseDown,
                location: CGPoint(x: 300, y: 300),
                modifierFlags: [],
                timestamp: 0,
                windowNumber: 0,
                context: nil,
                eventNumber: 0,
                clickCount: 1,
                pressure: 1
            )
        )
        NSApp.sendEvent(click)

        XCTAssertNil(presenter.visibleText)
        XCTAssertTrue(presenter.debugPanel?.isVisible == false)
    }
}
