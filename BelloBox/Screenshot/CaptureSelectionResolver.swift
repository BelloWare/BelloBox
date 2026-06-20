import AppKit
import CoreGraphics

enum CaptureSelectionResolver {
    static func resolve(
        startLocal: CGPoint?,
        endLocal: CGPoint,
        hoveredWindow: CaptureWindow?,
        screenFrame: CGRect,
        displayID: CGDirectDisplayID?,
        dragThreshold: CGFloat = RegionCaptureGeometry.dragThreshold
    ) -> CaptureSelection? {
        let start = startLocal ?? endLocal
        let localRect = RegionCaptureGeometry.selectionRect(from: start, to: endLocal)
        let isClick = localRect.width < dragThreshold && localRect.height < dragThreshold

        if isClick, let hoveredWindow {
            return .window(hoveredWindow)
        }

        guard let displayID else { return nil }
        if isClick {
            return .display(CaptureDisplay(displayID: displayID, frame: screenFrame))
        }

        let cocoaRect = RegionCaptureGeometry.localFlippedRectToGlobalCocoa(localRect, screenFrame: screenFrame)
        guard cocoaRect.width >= 8, cocoaRect.height >= 8 else { return nil }
        return .area(CaptureArea(cocoaRect: cocoaRect, displayID: displayID))
    }
}
