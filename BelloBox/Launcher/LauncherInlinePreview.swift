import SwiftUI

struct LauncherInlinePreview: View {
    let preview: LauncherPreview?
    var height: CGFloat = LauncherModel.previewHeight
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            if let preview {
                HStack(spacing: 6) {
                    Image(systemName: preview.isWarning ? "exclamationmark.circle" : "bolt.fill")
                        .foregroundStyle(preview.isWarning ? BoxTheme.warning : BoxTheme.accent)
                    Text(preview.title).fontWeight(.medium)
                    Spacer(minLength: 4)
                    Text(preview.subtitle).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
                }.font(.system(size: 10)).lineLimit(1)
                content(preview.content)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .transition(.opacity)
            } else {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Preparing preview…").font(.system(size: 11)).foregroundStyle(.secondary)
                }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
            }
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height)
        .animation(reduceMotion ? nil : .easeOut(duration: 0.14), value: preview != nil)
    }

    @ViewBuilder private func content(_ content: LauncherPreview.Content) -> some View {
        switch content {
        case .clocks(let clocks, _):
            HStack(spacing: 8) {
                ForEach(Array(clocks.enumerated()), id: \.offset) { _, clock in
                    VStack(alignment: .leading, spacing: 5) {
                        HStack {
                            Text(clock.name).font(.system(size: 11, weight: .medium)).lineLimit(1)
                            Spacer(minLength: 2)
                            Image(systemName: clock.quality.symbol).foregroundStyle(clock.quality.color)
                        }
                        Text(clock.time).font(.system(size: 23, weight: .medium, design: .rounded)).monospacedDigit()
                            .lineLimit(1).minimumScaleFactor(0.8)
                        Text(clock.date).font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(1)
                        Text(clock.zone).font(.system(size: 9, design: .monospaced)).foregroundStyle(.secondary)
                    }.padding(10).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                        .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 9))
                }
            }
        case .code(let text):
            Text(text).font(.system(size: 11, design: .monospaced)).lineSpacing(2).lineLimit(6)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(9).background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 8))
        case .fields(let fields):
            VStack(alignment: .leading, spacing: 6) {
                ForEach(Array(fields.enumerated()), id: \.offset) { _, field in
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text(field.label).foregroundStyle(.secondary).frame(width: 76, alignment: .leading)
                        Text(field.value).font(.system(size: 11, design: .monospaced)).frame(maxWidth: .infinity, alignment: .leading)
                    }.font(.system(size: 11)).lineLimit(1).truncationMode(.middle)
                }
            }.padding(10).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 8))
        case .statistics(let fields):
            HStack(spacing: 8) {
                ForEach(Array(fields.enumerated()), id: \.offset) { _, field in
                    VStack(alignment: .leading, spacing: 10) {
                        Text(field.label).font(.system(size: 10)).foregroundStyle(.secondary)
                        Text(field.value).font(.system(size: 23, weight: .medium, design: .rounded)).monospacedDigit()
                            .lineLimit(1).minimumScaleFactor(0.8)
                    }.padding(12).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                        .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 9))
                }
            }
        case .notice(let text):
            Text(text).font(.system(size: 12)).foregroundStyle(.secondary).lineLimit(4)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                .padding(12).background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 9))
        }
    }
}

/// The interactive World Clock planner inside the palette. Nothing here is a
/// row button: the timeline, cards, menus, and copilot own their own input,
/// while Enter in the search field still opens the full tool.
struct LauncherClockPreviewView: View {
    @ObservedObject var clock: WorldClockViewModel
    let preview: LauncherPreview?
    /// The row height the palette reserved; nil lets the content size itself.
    let height: CGFloat?
    var onOpenSettings: () -> Void
    var onEscapeCopilot: () -> Void
    var onCopilotFieldReady: (LauncherSearchTextField) -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            header
            planner
            cards
            WorldClockCopilotView(
                session: clock.copilot,
                style: .compact,
                plan: clock.copilotPlan(for:),
                deferredPlan: clock.deferredCopilotPlan(for:),
                onApply: clock.applyCopilotPlan(_:from:),
                onOpenSettings: onOpenSettings,
                onEscape: onEscapeCopilot,
                onFieldReady: onCopilotFieldReady
            )
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
        .animation(reduceMotion ? nil : .easeOut(duration: 0.14), value: clock.copilot.hasTranscript)
    }

    private var header: some View {
        HStack(spacing: 6) {
            Image(systemName: "bolt.fill").foregroundStyle(BoxTheme.accent)
            Text(preview?.title ?? "Timestamp recognized").fontWeight(.medium)
            if let subtitle = preview?.subtitle {
                Text("·").foregroundStyle(.tertiary)
                Text(subtitle).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
            }
            Spacer(minLength: 4)
            Menu {
                ForEach(clock.zonePresentations) { zone in
                    Button { clock.setAnchorZone(zone.id) } label: {
                        if zone.isAnchor { Label(zone.name, systemImage: "checkmark") } else { Text(zone.name) }
                    }
                }
            } label: {
                Label("Reference: \(clock.anchorName)", systemImage: "mappin.and.ellipse").font(.system(size: 10))
            }
            .menuStyle(.borderlessButton).fixedSize()
            .help("The day and timeline follow this location. Changes stay in the preview.")
            .accessibilityIdentifier("launcherClockReference")
        }.font(.system(size: 10)).lineLimit(1)
    }

    private var planner: some View {
        VStack(spacing: 6) {
            HStack(spacing: 8) {
                clockIconButton("chevron.left", label: "Previous day", size: 22) { clock.moveDay(by: -1) }
                    .accessibilityIdentifier("launcherClockPreviousDay")
                Text(clock.selectedTimeCompactTitle).font(.system(size: 12, weight: .semibold)).monospacedDigit().lineLimit(1)
                    .accessibilityIdentifier("launcherClockSelectedTime")
                clockIconButton("chevron.right", label: "Next day", size: 22) { clock.moveDay(by: 1) }
                    .accessibilityIdentifier("launcherClockNextDay")
                if clock.hasMovedFromSeed {
                    Button(action: clock.returnToSeed) {
                        Label("Selected time", systemImage: "arrow.uturn.backward").font(.system(size: 10, weight: .medium))
                    }
                    .buttonStyle(.plain).foregroundStyle(BoxTheme.accent)
                    .padding(.horizontal, 7).padding(.vertical, 3)
                    .background(BoxTheme.accentSoft, in: Capsule())
                    .help("Return to the timestamp you selected")
                    .accessibilityIdentifier("launcherClockReturnToSeed")
                }
                Spacer(minLength: 4)
                QualityBadge(quality: clock.selectedMeetingQuality)
                    .help("Combined availability across the shown locations")
            }
            MeetingTimelineView(qualities: clock.timelineQualities, offset: Binding(
                get: { clock.selectedOffset }, set: { clock.selectedOffset = $0 }
            ), duration: clock.timeline.duration, step: clock.timelineStep, onOverflow: { clock.nudge(by: $0) })
            .frame(height: 14)
            .help("Drag to pick a time, scroll sideways for 15-minute steps, or use ← → while the search field is empty")
            .accessibilityElement()
            .accessibilityLabel("Meeting time")
            .accessibilityValue(clock.selectedTimeTitle)
            .accessibilityAdjustableAction { direction in
                clock.nudge(by: direction == .increment ? 1 : -1)
            }
            .accessibilityIdentifier("launcherClockTimeline")
            HStack {
                Text(clock.dayStartLabel)
                Spacer()
                ForEach([MeetingTimeQuality.working, .extended, .poor], id: \.self) { quality in
                    Label(quality.shortLabel, systemImage: quality.symbol).foregroundStyle(quality.color)
                }
                Spacer()
                Text(clock.dayEndLabel)
            }
            .font(.system(size: 9)).foregroundStyle(.secondary)
        }
        .padding(.horizontal, 10).padding(.vertical, 8).surfaceCard()
    }

    private var cards: some View {
        HStack(spacing: 8) {
            ForEach(clock.zonePresentations.prefix(4)) { zone in
                VStack(alignment: .leading, spacing: 3) {
                    HStack(spacing: 4) {
                        Text(zone.name).font(.system(size: 11, weight: .medium)).lineLimit(1)
                        if zone.isAnchor {
                            Image(systemName: "mappin.circle.fill").font(.system(size: 9)).foregroundStyle(BoxTheme.accent)
                                .accessibilityLabel("Reference")
                        }
                        Spacer(minLength: 2)
                        Image(systemName: zone.quality.symbol).font(.system(size: 10)).foregroundStyle(zone.quality.color)
                            .accessibilityLabel(zone.quality.label)
                    }
                    Text(zone.timeText).font(.system(size: 20, weight: .medium, design: .rounded)).monospacedDigit()
                        .lineLimit(1).minimumScaleFactor(0.8)
                    HStack(spacing: 4) {
                        Text(zone.dateText).lineLimit(1)
                        if zone.dayDifference != 0 {
                            Text(zone.dayDifference > 0 ? "+\(zone.dayDifference)d" : "\(zone.dayDifference)d")
                                .foregroundStyle(BoxTheme.accent).fontWeight(.semibold)
                                .help("\(zone.dayDifference) calendar days from the reference location")
                        }
                    }.font(.system(size: 10)).foregroundStyle(.secondary)
                    // The offset gets its own line; four cards leave no room beside the date.
                    Text(zone.compactZoneText).font(.system(size: 9, design: .monospaced)).foregroundStyle(.secondary)
                        .lineLimit(1).help(zone.zoneText)
                }
                .padding(8).frame(maxWidth: .infinity, alignment: .topLeading)
                .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 9))
                .overlay(RoundedRectangle(cornerRadius: 9).strokeBorder(zone.isAnchor ? BoxTheme.accent.opacity(0.35) : BoxTheme.border))
                .accessibilityElement(children: .combine)
            }
        }
    }
}

extension LauncherPreview {
    var accessibilitySummary: String {
        let detail: String
        switch content {
        case .clocks(let clocks, _): detail = clocks.map { "\($0.name): \($0.time), \($0.date), \($0.zone)" }.joined(separator: "; ")
        case .code(let text), .notice(let text): detail = text
        case .fields(let fields), .statistics(let fields): detail = fields.map { "\($0.label): \($0.value)" }.joined(separator: "; ")
        }
        return "\(title). \(subtitle). \(detail)"
    }
}
