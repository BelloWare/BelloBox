import SwiftUI

struct OCRTextRegionsOverlayView: View {
    let regions: [OCRTextRegion]
    let viewport: ImageViewport

    var body: some View {
        ZStack(alignment: .topLeading) {
            ForEach(lineRegions) { region in
                if let rect = region.boundingBox?.rect {
                    Rectangle()
                        .stroke(BoxTheme.accent.opacity(0.75), lineWidth: 1.5)
                        .background(Rectangle().fill(BoxTheme.accent.opacity(0.08)))
                        .frame(width: viewport.imageRectToViewRect(rect).width, height: viewport.imageRectToViewRect(rect).height)
                        .position(
                            x: viewport.imageRectToViewRect(rect).midX,
                            y: viewport.imageRectToViewRect(rect).midY
                        )
                        .help(region.text)
                }
            }
        }
    }

    private var lineRegions: [OCRTextRegion] {
        regions.filter { $0.kind == .line || $0.kind == .paragraph || $0.kind == .block }
    }
}

