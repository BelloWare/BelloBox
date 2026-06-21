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
        let bounds = CGRect(origin: .zero, size: screenFrame.size)
        let localRect = RegionCaptureGeometry.clampedSelectionRect(from: start, to: endLocal, bounds: bounds)
        let clickThreshold = max(dragThreshold, RegionCaptureGeometry.minimumAreaSize)
        let isClick = localRect.width < clickThreshold && localRect.height < clickThreshold

        if isClick, let hoveredWindow {
            return .window(hoveredWindow)
        }

        guard let displayID else { return nil }
        if isClick {
            return .display(CaptureDisplay(displayID: displayID, frame: screenFrame))
        }

        let cocoaRect = RegionCaptureGeometry.localFlippedRectToGlobalCocoa(localRect, screenFrame: screenFrame)
        guard cocoaRect.width >= RegionCaptureGeometry.minimumAreaSize,
              cocoaRect.height >= RegionCaptureGeometry.minimumAreaSize
        else { return nil }
        return .area(CaptureArea(cocoaRect: cocoaRect, displayID: displayID))
    }
}
