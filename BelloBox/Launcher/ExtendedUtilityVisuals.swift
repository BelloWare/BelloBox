import AppKit
import SwiftUI

struct UtilityTableVisual: View {
    let table: DataTable
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
        if table.rows.count < table.totalRows || table.columns.count < (table.totalColumns ?? table.columns.count) {
            Text("Preview: \(table.rows.count) of \(table.totalRows) rows · \(table.columns.count) of \(table.totalColumns ?? table.columns.count) columns · Copy includes all")
                .font(.system(size: 10)).foregroundStyle(BoxTheme.secondaryText).padding(.horizontal, 8).padding(.top, 6)
        }
        GeometryReader { proxy in
            let scroller = NSScroller.scrollerWidth(for: .regular, scrollerStyle: .legacy) + 2
            ScrollView([.horizontal, .vertical]) {
                LazyVStack(alignment: .leading, spacing: 0, pinnedViews: [.sectionHeaders]) {
                    Section {
                        ForEach(Array(table.rows.enumerated()), id: \.offset) { i, row in
                            cells(row, header: false).background { if i.isMultiple(of: 2) { ToolSurface(role: .card) } }
                        }
                    } header: { cells(table.columns, header: true).background(BoxTheme.surface) }
                }.frame(minWidth: max(0, proxy.size.width - scroller), minHeight: max(0, proxy.size.height - scroller), alignment: .topLeading)
            }
        }.accessibilityLabel("Table preview: \(table.rows.count) of \(table.totalRows) rows, up to 30 columns. Copy includes the complete result.")
        }
    }
    private func cells(_ values: [String], header: Bool) -> some View {
        HStack(spacing: 0) {
            ForEach(Array(values.enumerated()), id: \.offset) { _, text in
                Text(previewText(text)).font(.system(size: 11, weight: header ? .semibold : .regular, design: .monospaced))
                    .lineLimit(2).textSelection(.enabled).frame(width: 150, alignment: .leading).padding(.horizontal, 8).padding(.vertical, 7)
                    .help(previewText(text))
            }
        }
    }
    private func previewText(_ text: String) -> String {
        let prefix = String.UnicodeScalarView(text.unicodeScalars.prefix(512))
        return String(prefix) + (text.unicodeScalars.dropFirst(512).isEmpty ? "" : "…")
    }
}

struct BezierGraphView: View {
    let curve: BezierCurve
    var onChange: (([Double]) -> Void)?
    @State private var draft: [Double]?
    var body: some View {
        GeometryReader { proxy in
            let w = max(1, Double(proxy.size.width) - 40), h = max(1, Double(proxy.size.height) - 28)
            let lower = min(0, curve.y1, curve.y2), upper = max(1, curve.y1, curve.y2), span = upper - lower
            let points = draft ?? curve.points
            let p1 = CGPoint(x: 20 + points[0] * w, y: 10 + (upper - points[1]) / span * h)
            let p2 = CGPoint(x: 20 + points[2] * w, y: 10 + (upper - points[3]) / span * h)
            let start = CGPoint(x: 20, y: 10 + upper / span * h), end = CGPoint(x: 20 + w, y: 10 + (upper - 1) / span * h)
            ZStack(alignment: .topLeading) {
                Canvas { context, _ in
                    var grid = Path()
                    for i in 0...4 { let x = 20 + w * Double(i) / 4; grid.move(to: CGPoint(x: x, y: 10)); grid.addLine(to: CGPoint(x: x, y: 10 + h)) }
                    for i in 0...4 { let y = 10 + h * Double(i) / 4; grid.move(to: CGPoint(x: 20, y: y)); grid.addLine(to: CGPoint(x: 20 + w, y: y)) }
                    context.stroke(grid, with: .color(BoxTheme.separator), lineWidth: 1)
                    var handles = Path(); handles.move(to: start); handles.addLine(to: p1); handles.move(to: end); handles.addLine(to: p2)
                    context.stroke(handles, with: .color(BoxTheme.accent.opacity(0.5)), style: StrokeStyle(lineWidth: 1, dash: [3,3]))
                    var path = Path(); path.move(to: start); path.addCurve(to: end, control1: p1, control2: p2)
                    context.stroke(path, with: .color(BoxTheme.accent), style: StrokeStyle(lineWidth: 2.5, lineCap: .round))
                }.allowsHitTesting(false)
                ForEach(0..<2) { i in
                    let point = i == 0 ? p1 : p2
                    Circle().fill(BoxTheme.surface).overlay(Circle().strokeBorder(BoxTheme.accent, lineWidth: 2))
                        .frame(width: 12, height: 12).padding(8).contentShape(Circle()).position(point)
                        .gesture(DragGesture(minimumDistance: 0, coordinateSpace: .named("bezierGraph")).onChanged { gesture in
                            guard onChange != nil else { return }
                            var next = draft ?? curve.points
                            next[i * 2] = min(1, max(0, (Double(gesture.location.x) - 20) / w))
                            next[i * 2 + 1] = min(10, max(-10, upper - (Double(gesture.location.y) - 10) / h * span))
                            draft = next
                        }.onEnded { _ in if let draft { onChange?(draft.map { ($0 * 1000).rounded() / 1000 }) }; draft = nil })
                        .accessibilityLabel("Bézier control point \(i + 1)")
                        .accessibilityValue("X \(MathTool.display(points[i * 2])), Y \(MathTool.display(points[i * 2 + 1])). Edit exact coordinates in the input field.")
                }
                Text("Time →").font(.system(size: 9)).foregroundStyle(BoxTheme.secondaryText).position(x: 20 + w / 2, y: h + 22)
            }.coordinateSpace(name: "bezierGraph")
        }
    }
}

struct StatisticsChartView: View {
    let value: StatisticsVisual
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 14) {
                metric("Mean", value.mean); metric("Median", value.median)
                Spacer(minLength: 0)
                Text("\(value.count) values").font(.system(size: 10)).foregroundStyle(BoxTheme.secondaryText)
            }
            GeometryReader { proxy in
                HStack(alignment: .bottom, spacing: 4) {
                    ForEach(Array(value.bins.enumerated()), id: \.offset) { _, count in
                        RoundedRectangle(cornerRadius: 3).fill(BoxTheme.accent.opacity(0.65))
                            .frame(maxWidth: .infinity).frame(height: max(1, proxy.size.height * Double(count) / Double(max(1, value.bins.max() ?? 1))))
                            .help("\(count) values")
                    }
                }.frame(maxHeight: .infinity, alignment: .bottom)
            }
            HStack { Text(MathTool.display(value.minimum)); Spacer(); Text(MathTool.display(value.maximum)) }
                .font(.system(size: 9, design: .monospaced)).foregroundStyle(BoxTheme.secondaryText)
        }.padding(10)
        .accessibilityLabel("Histogram of \(value.count) values. Mean \(MathTool.display(value.mean)), median \(MathTool.display(value.median)), range \(MathTool.display(value.minimum)) to \(MathTool.display(value.maximum)).")
    }
    private func metric(_ title: String, _ number: Double) -> some View {
        HStack(spacing: 5) { Text(title).foregroundStyle(BoxTheme.secondaryText); Text(MathTool.display(number)).fontWeight(.semibold) }.font(.system(size: 11, design: .monospaced))
    }
}

struct ShadowPreviewView: View {
    let value: BoxShadowSpec
    var body: some View {
        GeometryReader { proxy in
            let scale = min(1, max(0.15, (Double(proxy.size.height) - 20) / (80 + value.blur + 2 * max(0, value.spread) + abs(value.y))))
            ZStack {
                ShadowPreviewCanvas(value: value, scale: scale)
                RoundedRectangle(cornerRadius: 10 * scale).fill(BoxTheme.surface)
                    .overlay(RoundedRectangle(cornerRadius: 10 * scale).strokeBorder(BoxTheme.separator))
                    .frame(width: 160 * scale, height: 64 * scale)
                    .overlay(Text("Bello Box").font(.system(size: max(9, 14 * scale), weight: .semibold)))
            }.frame(maxWidth: .infinity, maxHeight: .infinity)
        }.clipped().accessibilityLabel("Scaled preview of the outer box shadow")
    }
}

/// Draw the shadow in Core Graphics. A composited SwiftUI blur can disappear
/// when the palette is rasterized; the native drawing retains its soft edge.
struct ShadowPreviewCanvas: NSViewRepresentable {
    let value: BoxShadowSpec
    let scale: Double
    func makeNSView(context: Context) -> ShadowPreviewNSView { ShadowPreviewNSView() }
    func updateNSView(_ view: ShadowPreviewNSView, context: Context) { view.value = value; view.scale = scale; view.needsDisplay = true }
}
final class ShadowPreviewNSView: NSView {
    var value: BoxShadowSpec?
    var scale = 1.0
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func draw(_ dirtyRect: NSRect) {
        guard let value, let context = NSGraphicsContext.current?.cgContext else { return }
        Self.draw(value, size: bounds.size, scale: scale, in: context)
    }
    static func draw(_ value: BoxShadowSpec, size: CGSize, scale: Double, in context: CGContext) {
        let width = max(0, 160 + 2 * value.spread) * scale, height = max(0, 64 + 2 * value.spread) * scale
        guard width > 0, height > 0 else { return }
        // Keep the unblurred caster outside the image; only its shadow lands
        // behind the card. AppKit's shadow offset already uses screen orientation.
        let shift = Double(size.width) * 3 + 1_000
        let rect = CGRect(x: (Double(size.width) - width) / 2 - shift, y: (Double(size.height) - height) / 2, width: width, height: height)
        let radius = max(0, 10 + value.spread) * scale
        let color = CGColor(srgbRed: value.color.red, green: value.color.green, blue: value.color.blue, alpha: value.color.alpha)
        context.saveGState(); defer { context.restoreGState() }
        context.setShadow(offset: CGSize(width: shift + value.x * scale, height: value.y * scale), blur: value.blur * scale / 2, color: color)
        context.setFillColor(CGColor(gray: 0, alpha: 1))
        context.addPath(CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil)); context.fillPath()
    }
}
