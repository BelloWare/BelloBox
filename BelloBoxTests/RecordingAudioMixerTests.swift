import XCTest
@testable import BelloBox

final class RecordingAudioMixerTests: XCTestCase {
    func testStagedExportURLUsesDestinationWhenSourceAndDestinationDiffer() {
        let source = URL(fileURLWithPath: "/tmp/source.mov")
        let destination = URL(fileURLWithPath: "/tmp/destination.mov")

        XCTAssertEqual(
            RecordingAudioMixer.stagedExportURL(sourceURL: source, destinationURL: destination),
            destination
        )
    }

    func testStagedExportURLAvoidsInPlaceExport() {
        let destination = URL(fileURLWithPath: "/tmp/recording.mov")
        let staged = RecordingAudioMixer.stagedExportURL(sourceURL: destination, destinationURL: destination)

        XCTAssertNotEqual(staged, destination)
        XCTAssertEqual(staged.deletingLastPathComponent(), destination.deletingLastPathComponent())
        XCTAssertEqual(staged.pathExtension, "mov")
        XCTAssertTrue(staged.lastPathComponent.hasPrefix("recording-mixed-"))
    }
}
