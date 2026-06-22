import CoreMedia
import XCTest
@testable import BelloBox

final class RecordingPauseTimelineTests: XCTestCase {
    func testOutputTimeSubtractsAccumulatedPauseDuration() {
        var timeline = RecordingPauseTimeline()

        XCTAssertEqualTime(timeline.outputTimeForWriting(sourceTime: seconds(8)), seconds(8))

        timeline.setPaused(true, at: seconds(10))
        timeline.setPaused(false, at: seconds(14))

        XCTAssertEqualTime(timeline.accumulatedDuration, seconds(0))
        XCTAssertEqualTime(timeline.outputTimeForWriting(sourceTime: seconds(14)), seconds(10))
        XCTAssertEqualTime(timeline.accumulatedDuration, seconds(4))
        XCTAssertEqualTime(timeline.outputTimeForWriting(sourceTime: seconds(20)), seconds(16))
    }

    func testRepeatedPauseStateChangesAreIdempotent() {
        var timeline = RecordingPauseTimeline()

        timeline.setPaused(true, at: seconds(10))
        timeline.setPaused(true, at: seconds(12))
        timeline.setPaused(false, at: seconds(15))
        timeline.setPaused(false, at: seconds(18))

        XCTAssertEqualTime(timeline.outputTimeForWriting(sourceTime: seconds(15)), seconds(10))
        XCTAssertEqualTime(timeline.accumulatedDuration, seconds(5))
        XCTAssertFalse(timeline.isPaused)
        XCTAssertEqualTime(timeline.outputTimeForWriting(sourceTime: seconds(25)), seconds(20))
    }

    func testPauseAgainBeforeWritingKeepsContinuousPausedInterval() {
        var timeline = RecordingPauseTimeline()

        timeline.setPaused(true, at: seconds(10))
        timeline.setPaused(false, at: seconds(14))
        timeline.setPaused(true, at: seconds(16))
        timeline.setPaused(false, at: seconds(20))

        XCTAssertEqualTime(timeline.outputTimeForWriting(sourceTime: seconds(20)), seconds(10))
        XCTAssertEqualTime(timeline.accumulatedDuration, seconds(10))
    }

    private func seconds(_ value: Double) -> CMTime {
        CMTime(seconds: value, preferredTimescale: 600)
    }

    private func XCTAssertEqualTime(
        _ lhs: CMTime,
        _ rhs: CMTime,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertEqual(CMTimeCompare(lhs, rhs), 0, file: file, line: line)
    }
}
