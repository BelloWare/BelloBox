import AppKit
import SwiftUI

/// Colored quality bands for one reference-zone day with a draggable marker.
/// Drag or click to jump, or scroll horizontally over it to nudge the time by
/// one step; vertical scrolling keeps reaching the enclosing list.
struct MeetingTimelineView: View {
    let qualities: [MeetingTimeQuality]
    @Binding var offset: TimeInterval
    let duration: TimeInterval
    var step: TimeInterval = 15 * 60
    /// Called for scroll-wheel steps that would leave the day, so hosts can
    /// roll into the previous or next day instead of clamping.
    var onOverflow: ((Int) -> Void)?

    var body: some View {
        GeometryReader { proxy in
            let width = max(proxy.size.width, 1)
            let marker = min(max(duration > 0 ? offset / duration : 0, 0), 1) * (width - 10) + 5
            ZStack(alignment: .leading) {
                HStack(spacing: 1) {
                    ForEach(Array(qualities.enumerated()), id: \.offset) { _, quality in
                        Rectangle().fill(quality.color.opacity(0.65))
                    }
                }.clipShape(RoundedRectangle(cornerRadius: 5))
                Capsule().fill(BoxTheme.surface)
                    .frame(width: 10, height: proxy.size.height + 6)
                    .overlay(Capsule().strokeBorder(BoxTheme.accent, lineWidth: 2))
                    .offset(x: marker - 5)
                TimelineScrubber(
                    onScrub: { fraction in offset = fraction * duration },
                    onStep: { steps in
                        let target = offset + Double(steps) * step
                        if target < 0 || target > duration, let onOverflow {
                            onOverflow(steps)
                        } else {
                            offset = min(max(target, 0), duration)
                        }
                    }
                )
            }
        }
    }
}

/// AppKit input for the timeline. SwiftUI has no scroll-wheel modifier on
/// macOS 13, and an NSView also keeps drags from reaching any row button.
struct TimelineScrubber: NSViewRepresentable {
    var onScrub: (CGFloat) -> Void
    var onStep: (Int) -> Void

    func makeNSView(context: Context) -> TimelineScrubberView {
        let view = TimelineScrubberView()
        view.onScrub = onScrub
        view.onStep = onStep
        return view
    }

    func updateNSView(_ view: TimelineScrubberView, context: Context) {
        view.onScrub = onScrub
        view.onStep = onStep
    }
}

/// Drag and scroll input for the timeline.
///
/// The palette's timeline lives inside SwiftUI's vertical `ScrollView`, an
/// `NSScrollView` that receives wheel events ahead of the hit-tested subview,
/// so `scrollWheel(with:)` never sees horizontal deltas there. A local wheel
/// monitor, attached only while the view is in a window, claims horizontal
/// events whose point is inside this view's visible rect and lets every other
/// event through, so vertical scrolling of the list keeps working and nothing
/// outside the timeline is touched. A consumed event is never dispatched, so
/// the monitor and the direct override cannot both step for one event.
final class TimelineScrubberView: NSView {
    var onScrub: (CGFloat) -> Void = { _ in }
    var onStep: (Int) -> Void = { _ in }
    private var accumulator = TimelineScrollAccumulator()
    private var wheelMonitor: Any?

    override var acceptsFirstResponder: Bool { false }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    var isWheelMonitorInstalled: Bool { wheelMonitor != nil }

    deinit { removeWheelMonitor() }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        removeWheelMonitor()
        guard window != nil else { return }
        wheelMonitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
            guard let self else { return event }
            return self.routeMonitoredWheel(event)
        }
    }

    private func removeWheelMonitor() {
        if let wheelMonitor { NSEvent.removeMonitor(wheelMonitor) }
        wheelMonitor = nil
    }

    override func mouseDown(with event: NSEvent) { scrub(event) }
    override func mouseDragged(with event: NSEvent) { scrub(event) }

    /// Returns nil when the event was consumed as a horizontal timeline step.
    func routeMonitoredWheel(_ event: NSEvent) -> NSEvent? {
        let deltas = Self.deltas(for: event)
        guard let window, window.isVisible, !isHiddenOrHasHiddenAncestor else {
            TimelineWheelDiagnostics.record("monitor", event: event, deltas: deltas, windowMatch: "detached", inside: false, consumed: false, steps: 0)
            return event
        }
        if let eventWindow = event.window, eventWindow !== window {
            TimelineWheelDiagnostics.record("monitor", event: event, deltas: deltas, windowMatch: "other", inside: false, consumed: false, steps: 0)
            return event
        }
        // Events without a window carry screen coordinates.
        let pointInWindow = event.window == nil ? window.convertPoint(fromScreen: event.locationInWindow) : event.locationInWindow
        let point = convert(pointInWindow, from: nil)
        // visibleRect reports the enclosing clip region in local coordinates,
        // which can be larger than this view; only the unclipped part of the
        // view itself counts. A scrubber scrolled out of view has an empty region.
        let region = visibleRect.intersection(bounds)
        let inside = !region.isEmpty && region.contains(point)
        guard inside, accumulator.isHorizontal(deltaX: deltas.x, deltaY: deltas.y) else {
            TimelineWheelDiagnostics.record("monitor", event: event, deltas: deltas, windowMatch: "match", inside: inside, consumed: false, steps: 0,
                                            point: point, visible: region)
            return event
        }
        let steps = step(event, deltaX: deltas.x)
        TimelineWheelDiagnostics.record("monitor", event: event, deltas: deltas, windowMatch: "match", inside: true, consumed: true, steps: steps,
                                        point: point, visible: region)
        return nil
    }

    /// Direct delivery, for hosts without a scroll view in the way. Vertical
    /// scrolling always continues up the responder chain.
    override func scrollWheel(with event: NSEvent) {
        let deltas = Self.deltas(for: event)
        guard accumulator.isHorizontal(deltaX: deltas.x, deltaY: deltas.y) else {
            TimelineWheelDiagnostics.record("direct", event: event, deltas: deltas, windowMatch: "match", inside: true, consumed: false, steps: 0)
            super.scrollWheel(with: event)
            return
        }
        let steps = step(event, deltaX: deltas.x)
        TimelineWheelDiagnostics.record("direct", event: event, deltas: deltas, windowMatch: "match", inside: true, consumed: true, steps: steps)
    }

    static func deltas(for event: NSEvent) -> (x: CGFloat, y: CGFloat) {
        let scale: CGFloat = event.hasPreciseScrollingDeltas ? 1 : TimelineScrollAccumulator.lineHeight
        return (event.scrollingDeltaX * scale, event.scrollingDeltaY * scale)
    }

    @discardableResult
    private func step(_ event: NSEvent, deltaX: CGFloat) -> Int {
        if event.phase == .began || event.momentumPhase == .began { accumulator.reset() }
        let steps = accumulator.consume(deltaX: deltaX)
        if steps != 0 { onStep(steps) }
        return steps
    }

    private func scrub(_ event: NSEvent) {
        let x = convert(event.locationInWindow, from: nil).x
        let fraction = min(max((x - 5) / max(bounds.width - 10, 1), 0), 1)
        onScrub(fraction)
    }
}

/// Opt-in wheel-routing log for reviews: BELLOBOX_E2E_WHEEL_DIAGNOSTICS=/path.
/// Lines carry deltas, phases, hit status, and routing only; never any text.
enum TimelineWheelDiagnostics {
    static func record(_ source: String, event: NSEvent, deltas: (x: CGFloat, y: CGFloat), windowMatch: String,
                       inside: Bool, consumed: Bool, steps: Int, point: NSPoint = .zero, visible: NSRect = .zero) {
#if DEBUG
        guard let path = ProcessInfo.processInfo.environment["BELLOBOX_E2E_WHEEL_DIAGNOSTICS"], !path.isEmpty else { return }
        let line = String(format: "t=%.3f source=%@ dx=%.1f dy=%.1f precise=%d phase=%d momentum=%d window=%@ point=(%.1f,%.1f) visible=(%.1f,%.1f,%.1f,%.1f) inside=%d horizontal=%d consumed=%d steps=%d\n",
                          Date().timeIntervalSince1970, source, deltas.x, deltas.y, event.hasPreciseScrollingDeltas ? 1 : 0,
                          Int(event.phase.rawValue), Int(event.momentumPhase.rawValue), windowMatch,
                          point.x, point.y, visible.origin.x, visible.origin.y, visible.width, visible.height, inside ? 1 : 0,
                          (deltas.x != 0 && abs(deltas.x) >= abs(deltas.y)) ? 1 : 0, consumed ? 1 : 0, steps)
        guard let data = line.data(using: .utf8) else { return }
        if let handle = FileHandle(forWritingAtPath: path) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: data)
        } else {
            try? data.write(to: URL(fileURLWithPath: path))
        }
#endif
    }
}

/// Turns trackpad deltas into whole steps. Scrolling "forward" (the direction
/// that reveals later content in a scroll view) moves later in time.
struct TimelineScrollAccumulator: Equatable {
    static let lineHeight: CGFloat = 10
    static let pixelsPerStep: CGFloat = 14
    private var pending: CGFloat = 0

    func isHorizontal(deltaX: CGFloat, deltaY: CGFloat) -> Bool {
        deltaX != 0 && abs(deltaX) >= abs(deltaY)
    }

    mutating func reset() { pending = 0 }

    mutating func consume(deltaX: CGFloat) -> Int {
        pending -= deltaX
        let steps = Int((pending / Self.pixelsPerStep).rounded(.towardZero))
        pending -= CGFloat(steps) * Self.pixelsPerStep
        return steps
    }
}

struct QualityBadge: View {
    let quality: MeetingTimeQuality
    var body: some View {
        Label(quality.shortLabel, systemImage: quality.symbol)
            .font(.system(size: 10, weight: .semibold)).foregroundStyle(quality.color)
            .padding(.horizontal, 8).padding(.vertical, 5)
            .background(quality.color.opacity(0.09), in: RoundedRectangle(cornerRadius: 6))
            .accessibilityLabel(quality.label)
    }
}

func clockIconButton(_ symbol: String, label: String, size: CGFloat = 28, action: @escaping () -> Void) -> some View {
    Button(action: action) {
        Image(systemName: symbol).font(.system(size: size * 0.43, weight: .medium))
            .frame(width: size, height: size)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 7))
            .contentShape(RoundedRectangle(cornerRadius: 7))
    }.buttonStyle(.plain).help(label).accessibilityLabel(label)
}

extension MeetingTimeQuality {
    var color: Color {
        switch self {
        case .working: return BoxTheme.success
        case .extended: return BoxTheme.warning
        case .poor: return BoxTheme.purple
        }
    }
    var symbol: String {
        switch self {
        case .working: return "sun.max"
        case .extended: return "sun.horizon"
        case .poor: return "moon.stars"
        }
    }
    var shortLabel: String {
        switch self {
        case .working: return "Working"
        case .extended: return "Fringe"
        case .poor: return "Night"
        }
    }
}
