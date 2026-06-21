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
