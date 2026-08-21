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

    func testParsesCommonUnixPrecisionsAndFractionalSeconds() throws {
        let milliseconds = try date(from: "1704067200125")
        let microseconds = try date(from: "1704067200125000")
        let nanoseconds = try date(from: "1704067200125000000")
        let fractionalSeconds = try date(from: "1704067200.125")

        XCTAssertEqual(milliseconds.timeIntervalSince1970, 1_704_067_200.125, accuracy: 0.001)
        XCTAssertEqual(microseconds, milliseconds)
        XCTAssertEqual(nanoseconds, milliseconds)
        XCTAssertEqual(fractionalSeconds, milliseconds)
    }

    func testParsesJavaScriptISOStringWithAndWithoutFractionalSeconds() throws {
        let milliseconds = try date(from: "2024-01-01T00:00:00.125Z")
        let seconds = try date(from: "2024-01-01T00:00:00Z")

        XCTAssertEqual(milliseconds.timeIntervalSince1970, 1_704_067_200.125, accuracy: 0.001)
        XCTAssertEqual(seconds.timeIntervalSince1970, 1_704_067_200, accuracy: 0.001)
    }

    func testParsesISO8601OffsetsAndCommonUTCNames() throws {
        let expected = try date(from: "2024-01-01T00:00:00Z")

        XCTAssertEqual(try date(from: "2024-01-01T08:00:00+08:00"), expected)
        XCTAssertEqual(try date(from: "2024-01-01T08:00:00.000+08:00"), expected)
        XCTAssertEqual(try date(from: "2024-01-01T08:00:00+0800"), expected)
        XCTAssertEqual(try date(from: "2024-01-01T08:00:00+08"), expected)
        XCTAssertEqual(try date(from: "2024-01-01t00:00:00z"), expected)
        XCTAssertEqual(try date(from: "2024-01-01 00:00:00 UTC"), expected)
        XCTAssertEqual(try date(from: "2024-01-01 00:00:00 GMT"), expected)
        XCTAssertEqual(try date(from: "2024-01-01 00:00:00 +0000"), expected)
    }

    func testTreatsTimezoneLessISOAndDatabaseTimestampsAsLocalTime() throws {
        let singapore = try XCTUnwrap(TimeZone(identifier: "Asia/Singapore"))
        let summary = try XCTUnwrap(
            TimestampSummary.make(
                from: "2024-01-01 08:00:00.125",
                locale: locale,
                timeZone: singapore
            )
        )

        XCTAssertEqual(summary.date.timeIntervalSince1970, 1_704_067_200.125, accuracy: 0.001)
    }

    func testParsesRFC2822HTTPAndJavaScriptDateStrings() throws {
        let expected = try date(from: "2026-08-21T12:34:56Z")

        XCTAssertEqual(try date(from: "Fri, 21 Aug 2026 12:34:56 GMT"), expected)
        XCTAssertEqual(try date(from: "21 Aug 2026 12:34:56 +0000"), expected)
        XCTAssertEqual(
            try date(from: "Fri Aug 21 2026 20:34:56 GMT+0800 (Singapore Standard Time)"),
            expected
        )
    }

    func testParsesTimestampWrappedInCommonSourceCodeQuotes() throws {
        let expected = try date(from: "2024-01-01T00:00:00Z")

        XCTAssertEqual(try date(from: "\"2024-01-01T00:00:00.000Z\""), expected)
        XCTAssertEqual(try date(from: "'2024-01-01T00:00:00Z'"), expected)
        XCTAssertEqual(try date(from: "`2024-01-01T00:00:00Z`"), expected)
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
            "+1704067200",
            "-1704067200",
            "170406720x",
            "1704 067200",
            "12345678",
            "2026-08-21",
            "2026-02-30T12:34:56Z",
            "2026-08-21T25:00:00Z",
            "2026-08-21T12:34:56Z trailing text",
            "08/21/2026 12:34:56",
        ]

        for selection in invalidSelections {
            XCTAssertNil(TimestampSummary.make(from: selection), "Expected rejection for \(selection)")
        }
    }

    private func date(from value: String) throws -> Date {
        try XCTUnwrap(
            TimestampSummary.make(
                from: value,
                locale: locale,
                timeZone: utc
            )
        ).date
    }
}
