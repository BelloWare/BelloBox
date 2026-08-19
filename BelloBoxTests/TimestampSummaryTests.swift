import XCTest
@testable import BelloBox

final class TimestampSummaryTests: XCTestCase {
    private let locale = Locale(identifier: "en_US_POSIX")
    private let utc = TimeZone(secondsFromGMT: 0)!

    func testParsesUnixSecondsAndMillisecondsAsTheSameDate() throws {
        let now = Date(timeIntervalSince1970: 1_704_070_800)
        let seconds = try XCTUnwrap(
            TimestampSummary.make(
                from: "1704067200",
                relativeTo: now,
                locale: locale,
                timeZone: utc
            )
        )
        let milliseconds = try XCTUnwrap(
            TimestampSummary.make(
                from: "1704067200000",
                relativeTo: now,
                locale: locale,
                timeZone: utc
            )
        )

        XCTAssertEqual(seconds.date, milliseconds.date)
        XCTAssertEqual(seconds.date.timeIntervalSince1970, 1_704_067_200, accuracy: 0.001)
        XCTAssertEqual(seconds.relativeTime, "1 hour ago")
        XCTAssertTrue(seconds.localDateTime.contains("Jan 1, 2024"))
        XCTAssertTrue(seconds.localDateTime.hasSuffix("GMT"))
    }

    func testUsesRequestedLocalTimeZone() throws {
        let singapore = try XCTUnwrap(TimeZone(identifier: "Asia/Singapore"))
        let summary = try XCTUnwrap(
            TimestampSummary.make(
                from: "1704067200",
                relativeTo: Date(timeIntervalSince1970: 1_704_067_200),
                locale: locale,
                timeZone: singapore
            )
        )

        XCTAssertTrue(summary.localDateTime.contains("8:00:00"))
        XCTAssertTrue(summary.localDateTime.contains("AM"))
        XCTAssertTrue(summary.localDateTime.hasSuffix("GMT+8"))
    }

    func testTrimsOuterWhitespace() {
        XCTAssertNotNil(
            TimestampSummary.make(
                from: "  1704067200\n",
                locale: locale,
                timeZone: utc
            )
        )
    }

    func testRejectsNonTimestampSelections() {
        let invalidSelections = [
            "",
            "123456789",
            "12345678901",
            "123456789012",
            "12345678901234",
            "1704067200.0",
            "+1704067200",
            "-1704067200",
            "170406720x",
            "1704 067200",
        ]

        for selection in invalidSelections {
            XCTAssertNil(TimestampSummary.make(from: selection), "Expected rejection for \(selection)")
        }
    }
}
