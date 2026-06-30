import AppKit
import CoreGraphics

enum CaptureSelectionResolver {
    static func resolve(
        startLocal: CGPoint?,
        endLocal: CGPoint,
        hoveredWindow: CaptureWindow?,
        screenFrame: CGRect,
        displayID: CGDirectDisplayID?,
        policy: CaptureSelectionPolicy = .any,
        dragThreshold: CGFloat = RegionCaptureGeometry.dragThreshold
    ) -> CaptureSelection? {
        let start = startLocal ?? endLocal
        let bounds = CGRect(origin: .zero, size: screenFrame.size)
        let localRect = RegionCaptureGeometry.clampedSelectionRect(from: start, to: endLocal, bounds: bounds)
        let clickThreshold = max(dragThreshold, RegionCaptureGeometry.minimumAreaSize)
        let isClick = localRect.width < clickThreshold && localRect.height < clickThreshold

        guard let displayID else { return nil }

        switch policy {
        case .displayOnly:
            return .display(CaptureDisplay(displayID: displayID, frame: screenFrame))
        case .windowOnly:
            guard isClick, let hoveredWindow else { return nil }
            return .window(hoveredWindow)
        case .areaOnly:
            guard !isClick else { return nil }
            return areaSelection(localRect: localRect, screenFrame: screenFrame, displayID: displayID)
        case .areaOrWindow:
            if isClick, let hoveredWindow {
                return .window(hoveredWindow)
            }
            guard !isClick else { return nil }
            return areaSelection(localRect: localRect, screenFrame: screenFrame, displayID: displayID)
        case .any:
            if isClick, let hoveredWindow {
                return .window(hoveredWindow)
            }
            if isClick {
                return .display(CaptureDisplay(displayID: displayID, frame: screenFrame))
            }
            return areaSelection(localRect: localRect, screenFrame: screenFrame, displayID: displayID)
        }
    }

    private static func areaSelection(
        localRect: CGRect,
        screenFrame: CGRect,
        displayID: CGDirectDisplayID
    ) -> CaptureSelection? {
        let cocoaRect = RegionCaptureGeometry.localFlippedRectToGlobalCocoa(localRect, screenFrame: screenFrame)
        guard cocoaRect.width >= RegionCaptureGeometry.minimumAreaSize,
              cocoaRect.height >= RegionCaptureGeometry.minimumAreaSize
        else { return nil }
        return .area(CaptureArea(cocoaRect: cocoaRect, displayID: displayID))
    }
}
