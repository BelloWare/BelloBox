import XCTest
@testable import BelloBox

final class ScreenCoordinateSpaceTests: XCTestCase {
    func testCocoaToDisplayPixelsOnOneXScreen() {
        let screen = CGRect(x: 0, y: 0, width: 1000, height: 800)
        let rect = CGRect(x: 100, y: 200, width: 300, height: 120)
        let pixels = ScreenCoordinateSpace.cocoaRectToDisplayPixelRect(rect, screenFrame: screen, scale: 1)
        XCTAssertEqual(pixels, CGRect(x: 100, y: 480, width: 300, height: 120))
    }

    func testCocoaToDisplayPixelsOnRetinaScreen() {
        let screen = CGRect(x: 0, y: 0, width: 1000, height: 800)
        let rect = CGRect(x: 100, y: 200, width: 300, height: 120)
        let pixels = ScreenCoordinateSpace.cocoaRectToDisplayPixelRect(rect, screenFrame: screen, scale: 2)
        XCTAssertEqual(pixels, CGRect(x: 200, y: 960, width: 600, height: 240))
    }

    func testSecondaryDisplayWithNegativeOriginRoundTrip() {
        let screen = CGRect(x: -1200, y: 80, width: 1200, height: 700)
        let rect = CGRect(x: -1000, y: 200, width: 400, height: 180)
        let pixels = ScreenCoordinateSpace.cocoaRectToDisplayPixelRect(rect, screenFrame: screen, scale: 2)
        let roundTrip = ScreenCoordinateSpace.displayPixelRectToCocoaRect(pixels, screenFrame: screen, scale: 2)
        XCTAssertEqual(roundTrip.origin.x, rect.origin.x, accuracy: 0.5)
        XCTAssertEqual(roundTrip.origin.y, rect.origin.y, accuracy: 0.5)
        XCTAssertEqual(roundTrip.width, rect.width, accuracy: 0.5)
        XCTAssertEqual(roundTrip.height, rect.height, accuracy: 0.5)
    }

    func testCocoaToImagePixelsUsesActualCapturedImageSize() {
        let screen = CGRect(x: 0, y: 0, width: 100, height: 80)
        let rect = CGRect(x: 10, y: 20, width: 30, height: 15)
        let pixels = ScreenCoordinateSpace.cocoaRectToImagePixelRect(
            rect,
            screenFrame: screen,
            imageSize: CGSize(width: 250, height: 200)
        )

        XCTAssertEqual(pixels, CGRect(x: 25, y: 112, width: 75, height: 38))
    }

    func testCocoaToDisplayPixelsCoversFractionalScaledEdges() {
        let screen = CGRect(x: 0, y: 0, width: 100, height: 80)
        let rect = CGRect(x: 10.2, y: 20.1, width: 30.2, height: 15.2)
        let pixels = ScreenCoordinateSpace.cocoaRectToDisplayPixelRect(rect, screenFrame: screen, scale: 1.5)

        XCTAssertEqual(pixels, CGRect(x: 15, y: 67, width: 46, height: 23))
    }

    func testCocoaToImagePixelsCoversFractionalCapturedImageEdges() {
        let screen = CGRect(x: 0, y: 0, width: 100, height: 80)
        let rect = CGRect(x: 10.2, y: 20.1, width: 30.2, height: 15.2)
        let pixels = ScreenCoordinateSpace.cocoaRectToImagePixelRect(
            rect,
            screenFrame: screen,
            imageSize: CGSize(width: 250, height: 200)
        )

        XCTAssertEqual(pixels, CGRect(x: 25, y: 111, width: 76, height: 39))
    }

    func testImageScaleUsesCapturedPixelWidth() {
        let screen = CGRect(x: 0, y: 0, width: 1440, height: 900)
        XCTAssertEqual(ScreenCoordinateSpace.imageScale(pixelWidth: 2880, screenFrame: screen), 2)
    }

    func testPixelSizeUsesActualDisplayPixels() {
        let screen = CGRect(x: 0, y: 0, width: 100, height: 80)
        let size = ScreenCoordinateSpace.pixelSize(
            forCocoaSize: CGSize(width: 30, height: 15),
            screenFrame: screen,
            displayPixelSize: CGSize(width: 250, height: 200)
        )

        XCTAssertEqual(size.width, 75)
        XCTAssertEqual(size.height, 37.5)
    }

    func testResolvedDisplayPixelSizePrefersBackingScaleWhenCoreGraphicsReportsPointSize() {
        let size = ScreenCoordinateSpace.resolvedDisplayPixelSize(
            cgPixelSize: CGSize(width: 1728, height: 992),
            screenFrame: CGRect(x: 0, y: 0, width: 1728, height: 992),
            backingScale: 2
        )

        XCTAssertEqual(size, CGSize(width: 3456, height: 1984))
    }

    func testResolvedDisplayPixelSizeKeepsLargerCoreGraphicsPixelSize() {
        let size = ScreenCoordinateSpace.resolvedDisplayPixelSize(
            cgPixelSize: CGSize(width: 5120, height: 2880),
            screenFrame: CGRect(x: 0, y: 0, width: 2560, height: 1440),
            backingScale: 1
        )

        XCTAssertEqual(size, CGSize(width: 5120, height: 2880))
    }

    func testCGWindowBoundsConvertUsingPrimaryScreenTopEdge() {
        let frames = [CGRect(x: 0, y: 0, width: 1440, height: 900)]
        let bounds = CGRect(x: 100, y: 80, width: 300, height: 200)

        let rect = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(bounds, screenFrames: frames)

        XCTAssertEqual(rect, CGRect(x: 100, y: 620, width: 300, height: 200))
    }

    func testTopLeftPointConvertUsingPrimaryScreenTopEdge() {
        let frames = [
            CGRect(x: 0, y: 0, width: 1440, height: 900),
            CGRect(x: 0, y: 900, width: 1920, height: 1080),
        ]
        let point = CGPoint(x: 250, y: -300)

        let cocoa = ScreenCoordinateSpace.topLeftPointToCocoaPoint(point, screenFrames: frames)

        XCTAssertEqual(cocoa, CGPoint(x: 250, y: 1200))
    }

    func testCocoaPointConvertsBackToTopLeftCoordinates() {
        let frames = [
            CGRect(x: 0, y: 0, width: 1440, height: 900),
            CGRect(x: 0, y: 900, width: 1920, height: 1080),
            CGRect(x: 0, y: -1080, width: 1920, height: 1080),
        ]
        let cocoa = CGPoint(x: 250, y: 1200)

        let topLeft = ScreenCoordinateSpace.cocoaPointToTopLeftPoint(cocoa, screenFrames: frames)

        XCTAssertEqual(topLeft, CGPoint(x: 250, y: -300))
        XCTAssertEqual(ScreenCoordinateSpace.topLeftPointToCocoaPoint(topLeft, screenFrames: frames), cocoa)
    }

    func testCGWindowBoundsConvertForDisplayBelowPrimary() {
        let frames = [
            CGRect(x: 0, y: 0, width: 1440, height: 900),
            CGRect(x: 0, y: -1080, width: 1920, height: 1080),
        ]
        let bounds = CGRect(x: 240, y: 1600, width: 500, height: 200)

        let rect = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(bounds, screenFrames: frames)

        XCTAssertEqual(rect, CGRect(x: 240, y: -900, width: 500, height: 200))
    }

    func testCGWindowBoundsConvertForDisplayAbovePrimary() {
        let frames = [
            CGRect(x: 0, y: 0, width: 1440, height: 900),
            CGRect(x: 0, y: 900, width: 1920, height: 1080),
        ]
        let bounds = CGRect(x: 240, y: -500, width: 500, height: 200)

        let rect = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(bounds, screenFrames: frames)

        XCTAssertEqual(rect, CGRect(x: 240, y: 1200, width: 500, height: 200))
    }

    func testScreenFrameChoosesLargestIntersectionForCrossDisplayRect() {
        let frames = [
            CGRect(x: 0, y: 0, width: 1000, height: 800),
            CGRect(x: 1000, y: 0, width: 900, height: 800),
        ]
        let rect = CGRect(x: 900, y: 100, width: 320, height: 200)

        let frame = ScreenCoordinateSpace.screenFrame(for: rect, in: frames)

        XCTAssertEqual(frame, frames[1])
    }

    func testScreenFrameFallsBackToNearestDisplayForPointInGap() {
        let frames = [
            CGRect(x: 0, y: 0, width: 1000, height: 800),
            CGRect(x: 1120, y: 0, width: 900, height: 800),
        ]
        let pointInGapCloserToRightDisplay = CGPoint(x: 1085, y: 400)

        let frame = ScreenCoordinateSpace.screenFrame(containingOrNearestTo: pointInGapCloserToRightDisplay, in: frames)

        XCTAssertEqual(frame, frames[1])
    }

    func testScreenFrameFallsBackToNearestNegativeOriginDisplayForOffscreenRect() {
        let frames = [
            CGRect(x: -1280, y: 0, width: 1280, height: 800),
            CGRect(x: 0, y: 0, width: 1440, height: 900),
        ]
        let rectJustPastLeftDisplay = CGRect(x: -1360, y: 300, width: 40, height: 40)

        let frame = ScreenCoordinateSpace.screenFrame(for: rectJustPastLeftDisplay, in: frames)

        XCTAssertEqual(frame, frames[0])
    }

    func testStrictScreenFrameReturnsNilForOffscreenRect() {
        let frames = [
            CGRect(x: -1280, y: 0, width: 1280, height: 800),
            CGRect(x: 0, y: 0, width: 1440, height: 900),
        ]
        let rectJustPastLeftDisplay = CGRect(x: -1360, y: 300, width: 40, height: 40)

        let frame = ScreenCoordinateSpace.strictScreenFrame(for: rectJustPastLeftDisplay, in: frames)

        XCTAssertNil(frame)
    }

    func testStrictScreenFrameChoosesLargestIntersection() {
        let frames = [
            CGRect(x: 0, y: 0, width: 1000, height: 800),
            CGRect(x: 1000, y: 0, width: 900, height: 800),
        ]
        let rect = CGRect(x: 900, y: 100, width: 320, height: 200)

        let frame = ScreenCoordinateSpace.strictScreenFrame(for: rect, in: frames)

        XCTAssertEqual(frame, frames[1])
    }
}
