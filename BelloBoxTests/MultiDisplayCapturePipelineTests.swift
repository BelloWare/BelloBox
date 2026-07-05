import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class MultiDisplayCapturePipelineTests: XCTestCase {
    func testSecondaryDisplayPipelineRightAtOneX() throws {
        try assertPipelineCropsSelectedDisplay(
            frame: CGRect(x: 300, y: 0, width: 300, height: 200),
            scale: 1,
            displayID: 101,
            displayCode: 60
        )
    }

    func testSecondaryDisplayPipelineLeftAtOneX() throws {
        try assertPipelineCropsSelectedDisplay(
            frame: CGRect(x: -300, y: 0, width: 300, height: 200),
            scale: 1,
            displayID: 102,
            displayCode: 80
        )
    }

    func testSecondaryDisplayPipelineAboveAtOneX() throws {
        try assertPipelineCropsSelectedDisplay(
            frame: CGRect(x: 0, y: 200, width: 300, height: 200),
            scale: 1,
            displayID: 103,
            displayCode: 100
        )
    }

    func testSecondaryDisplayPipelineBelowAtOneX() throws {
        try assertPipelineCropsSelectedDisplay(
            frame: CGRect(x: 0, y: -200, width: 300, height: 200),
            scale: 1,
            displayID: 104,
            displayCode: 120
        )
    }

    func testNegativeOriginSecondaryDisplayPipelineAtTwoX() throws {
        try assertPipelineCropsSelectedDisplay(
            frame: CGRect(x: -300, y: -200, width: 300, height: 200),
            scale: 2,
            displayID: 105,
            displayCode: 140
        )
    }

    func testPositiveOriginSecondaryDisplayPipelineAtTwoX() throws {
        try assertPipelineCropsSelectedDisplay(
            frame: CGRect(x: 300, y: 200, width: 300, height: 200),
            scale: 2,
            displayID: 106,
            displayCode: 160
        )
    }

    private func assertPipelineCropsSelectedDisplay(
        frame: CGRect,
        scale: CGFloat,
        displayID: CGDirectDisplayID,
        displayCode: UInt8,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws {
        let primarySnapshot = DisplaySnapshot(
            displayID: 1,
            screenFrame: CGRect(x: 0, y: 0, width: 300, height: 200),
            scale: 1,
            image: encodedImage(width: 300, height: 200, displayCode: 20)
        )
        let targetSnapshot = DisplaySnapshot(
            displayID: displayID,
            screenFrame: frame,
            scale: scale,
            image: encodedImage(
                width: Int(frame.width * scale),
                height: Int(frame.height * scale),
                displayCode: displayCode
            )
        )
        let snapshots = [primarySnapshot, targetSnapshot]
        let start = CGPoint(x: 40, y: 50)
        let end = CGPoint(x: 150, y: 110)
        let selection = try XCTUnwrap(
            CaptureSelectionResolver.resolve(
                startLocal: start,
                endLocal: end,
                hoveredWindow: nil,
                screenFrame: frame,
                displayID: displayID,
                policy: .areaOnly
            ),
            file: file,
            line: line
        )
        guard case let .area(area) = selection else {
            XCTFail("Expected area selection.", file: file, line: line)
            return
        }
        let snapshot = try XCTUnwrap(
            snapshots.first(where: { $0.displayID == area.displayID }),
            file: file,
            line: line
        )

        let document = try ScreenCaptureService().document(
            fromSnapshot: snapshot,
            cocoaRect: area.cocoaRect,
            source: .area(rect: area.cocoaRect, displayID: area.displayID)
        )

        XCTAssertEqual(document.baseImage.width, Int((end.x - start.x) * scale), file: file, line: line)
        XCTAssertEqual(document.baseImage.height, Int((end.y - start.y) * scale), file: file, line: line)
        XCTAssertEqual(document.scale, scale, file: file, line: line)

        let expectedTopLeft = encodedPixel(
            displayCode: displayCode,
            x: Int(start.x * scale),
            y: Int(start.y * scale)
        )
        let actualTopLeft = ScreenshotTestHelpers.pixel(document.baseImage, x: 0, y: 0)
        XCTAssertEqual(Array(actualTopLeft.prefix(3)), expectedTopLeft, file: file, line: line)

        let centerSourceX = Int(((start.x + end.x) / 2) * scale)
        let centerSourceY = Int(((start.y + end.y) / 2) * scale)
        let expectedCenter = encodedPixel(displayCode: displayCode, x: centerSourceX, y: centerSourceY)
        let actualCenter = ScreenshotTestHelpers.pixel(
            document.baseImage,
            x: document.baseImage.width / 2,
            y: document.baseImage.height / 2
        )
        XCTAssertEqual(Array(actualCenter.prefix(3)), expectedCenter, file: file, line: line)
    }

    private func encodedImage(width: Int, height: Int, displayCode: UInt8) -> CGImage {
        let bytesPerRow = width * 4
        var data = Data(count: bytesPerRow * height)
        data.withUnsafeMutableBytes { rawBuffer in
            let bytes = rawBuffer.bindMemory(to: UInt8.self)
            for y in 0..<height {
                for x in 0..<width {
                    let offset = y * bytesPerRow + x * 4
                    let pixel = encodedPixel(displayCode: displayCode, x: x, y: y)
                    bytes[offset] = pixel[0]
                    bytes[offset + 1] = pixel[1]
                    bytes[offset + 2] = pixel[2]
                    bytes[offset + 3] = 255
                }
            }
        }
        let provider = CGDataProvider(data: data as CFData)!
        return CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        )!
    }

    private func encodedPixel(displayCode: UInt8, x: Int, y: Int) -> [UInt8] {
        [
            displayCode,
            UInt8(x % 251),
            UInt8(y % 251),
        ]
    }
}
