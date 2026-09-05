import AppKit
import CoreMedia
import XCTest
@testable import BelloBox

final class RecordingOverlayEventStoreTests: XCTestCase {
    func testActiveEventsReturnsCurrentEventsAndPrunesExpiredOnes() {
        let store = RecordingOverlayEventStore()
        let expired = TimedOverlayEvent(
            id: UUID(),
            time: CMTime(seconds: 0, preferredTimescale: 600),
            kind: .secureTypingHidden,
            expiresAt: CMTime(seconds: 1, preferredTimescale: 600)
        )
        let active = TimedOverlayEvent(
            id: UUID(),
            time: CMTime(seconds: 1, preferredTimescale: 600),
            kind: .secureTypingHidden,
            expiresAt: CMTime(seconds: 3, preferredTimescale: 600)
        )
        let future = TimedOverlayEvent(
            id: UUID(),
            time: CMTime(seconds: 4, preferredTimescale: 600),
            kind: .secureTypingHidden,
            expiresAt: CMTime(seconds: 5, preferredTimescale: 600)
        )

        store.add(expired)
        store.add(active)
        store.add(future)

        XCTAssertEqual(store.activeEvents(at: CMTime(seconds: 2, preferredTimescale: 600)), [active])
        XCTAssertEqual(store.activeEvents(at: CMTime(seconds: 4.5, preferredTimescale: 600)), [future])
    }

    func testEventStoreRemainsBoundedWithoutVideoFrames() {
        let store = RecordingOverlayEventStore()
        for _ in 0..<1000 {
            store.add(TimedOverlayEvent(id: UUID(), time: .zero, kind: .secureTypingHidden,
                expiresAt: CMTime(seconds: 2, preferredTimescale: 600)))
        }
        XCTAssertEqual(store.activeEvents(at: .zero).count, 128)
    }

    func testPauseAndDisableClearPendingKeysAndClicks() throws {
        var options = RecordingOptions.default
        options.keystrokeMode = .shortcutsOnly
        let monitor = RecordingInputMonitor(options: options)
        let event = try XCTUnwrap(CGEvent(keyboardEventSource: nil, virtualKey: 8, keyDown: true))
        event.flags = .maskCommand
        let characters: [UniChar] = [67]
        event.keyboardSetUnicodeString(stringLength: characters.count, unicodeString: characters)
        monitor.handle(type: .keyDown, event: event)
        XCTAssertEqual(monitor.eventStore.activeEvents(at: CMClockGetTime(CMClockGetHostTimeClock())).count, 1)
        monitor.setPaused(true)
        monitor.handle(type: .keyDown, event: event)
        XCTAssertTrue(monitor.eventStore.activeEvents(at: CMClockGetTime(CMClockGetHostTimeClock())).isEmpty)
        monitor.setPaused(false)
        monitor.handle(type: .keyDown, event: event)
        XCTAssertEqual(monitor.eventStore.activeEvents(at: CMClockGetTime(CMClockGetHostTimeClock())).count, 1)
        XCTAssertTrue(monitor.updateOverlays(clicks: .off, keys: .off))
        monitor.handle(type: .keyDown, event: event)
        monitor.handle(type: .leftMouseDown, event: event)
        XCTAssertTrue(monitor.eventStore.activeEvents(at: CMClockGetTime(CMClockGetHostTimeClock())).isEmpty)
    }

    func testClearRemovesAllEvents() {
        let store = RecordingOverlayEventStore()
        store.add(TimedOverlayEvent(
            id: UUID(),
            time: .zero,
            kind: .secureTypingHidden,
            expiresAt: CMTime(seconds: 10, preferredTimescale: 600)
        ))

        store.clear()

        XCTAssertTrue(store.activeEvents(at: CMTime(seconds: 1, preferredTimescale: 600)).isEmpty)
    }
}
