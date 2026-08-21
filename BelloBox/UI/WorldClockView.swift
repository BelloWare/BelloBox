import SwiftUI

struct WorldClockView: View {
    @ObservedObject var viewModel: WorldClockViewModel
    var onOpenSettings: () -> Void

    @State private var showingZonePicker = false
    @State private var showingAI = false

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            navigation
            Divider()
            timelineSection
            Divider()
            locationsSection
            Divider()
            addAndAISection
        }
        .frame(minWidth: 760, minHeight: 590)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var header: some View {
        HStack(spacing: 13) {
            Image(systemName: "globe.americas.fill")
                .font(.system(size: 23, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 44, height: 44)
                .background(RoundedRectangle(cornerRadius: 8).fill(BoxTheme.accentGradient))

            VStack(alignment: .leading, spacing: 2) {
                Text("World Clock")
                    .font(.title2.weight(.semibold))
                Text("Compare a single moment across your locations.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()
            meetingLegend
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 15)
    }

    private var meetingLegend: some View {
        HStack(spacing: 12) {
            legendItem("Working", quality: .working)
            legendItem("Fringe", quality: .extended)
            legendItem("Night", quality: .poor)
        }
        .font(.caption2)
        .foregroundStyle(.secondary)
    }

    private func legendItem(_ text: String, quality: MeetingTimeQuality) -> some View {
        HStack(spacing: 4) {
            Circle().fill(quality.color).frame(width: 8, height: 8)
            Text(text)
        }
    }

    private var navigation: some View {
        HStack(spacing: 8) {
            Button { viewModel.moveDay(by: -1) } label: {
                Image(systemName: "chevron.left")
            }
            .buttonStyle(.borderless)
            .help("Previous day")

            DatePicker(
                "Date",
                selection: Binding(
                    get: { viewModel.timeline.start },
                    set: { viewModel.selectDay(containing: $0) }
                ),
                displayedComponents: .date
            )
            .labelsHidden()
            .environment(\.timeZone, viewModel.anchorTimeZone)
            .accessibilityIdentifier("worldClockDatePicker")

            Button("Now") { viewModel.goToNow() }
                .controlSize(.small)
                .help("Return to the current date and time")

            Button { viewModel.moveDay(by: 1) } label: {
                Image(systemName: "chevron.right")
            }
            .buttonStyle(.borderless)
            .help("Next day")

            Spacer()

            Menu {
                ForEach(viewModel.zonePresentations) { zone in
                    Button {
                        viewModel.setAnchorZone(zone.id)
                    } label: {
                        if zone.isAnchor {
                            Label(zone.name, systemImage: "checkmark")
                        } else {
                            Text(zone.name)
                        }
                    }
                }
            } label: {
                Label("Reference: \(viewModel.anchorName)", systemImage: "mappin.and.ellipse")
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .help("The date and timeline use this location")
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 10)
        .background(Color.primary.opacity(0.025))
    }

    private var timelineSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(viewModel.selectedTimeTitle)
                        .font(.headline)
                    Text("Reference time in \(viewModel.anchorName)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                QualityBadge(quality: viewModel.selectedMeetingQuality)
            }

            MeetingTimelineView(
                qualities: viewModel.timelineQualities,
                offset: Binding(
                    get: { viewModel.selectedOffset },
                    set: { viewModel.selectedOffset = $0 }
                ),
                duration: viewModel.timeline.duration
            )
            .frame(height: 22)

            Slider(
                value: Binding(
                    get: { viewModel.selectedOffset },
                    set: { viewModel.selectedOffset = $0 }
                ),
                in: 0...viewModel.timeline.duration,
                step: viewModel.timelineStep
            )
            .accessibilityLabel("Meeting time")
            .accessibilityValue(viewModel.selectedTimeTitle)
            .accessibilityIdentifier("worldClockTimeSlider")

            HStack {
                Text(viewModel.dayStartLabel)
                Spacer()
                Text("Drag the colored range or slider")
                Spacer()
                Text(viewModel.dayEndLabel)
            }
            .font(.caption2)
            .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
    }

    private var locationsSection: some View {
        let zones = viewModel.zonePresentations
        return ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(Array(zones.enumerated()), id: \.element.id) { index, zone in
                    WorldClockZoneRow(
                        zone: zone,
                        canRemove: viewModel.canRemoveZone,
                        onMakeAnchor: { viewModel.setAnchorZone(zone.id) },
                        onRemove: { viewModel.removeZone(zone.id) }
                    )
                    if index < zones.count - 1 {
                        Divider().padding(.leading, 50)
                    }
                }
            }
        }
        .frame(minHeight: 170)
        .accessibilityIdentifier("worldClockLocationList")
    }

    private var addAndAISection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Button {
                    showingZonePicker.toggle()
                } label: {
                    Label("Add Location", systemImage: "plus")
                }
                .buttonStyle(SecondaryButtonStyle())
                .accessibilityLabel("Add Location")
                .accessibilityIdentifier("worldClockAddLocationButton")
                .popover(isPresented: $showingZonePicker, arrowEdge: .bottom) {
                    WorldClockZonePicker(viewModel: viewModel) {
                        showingZonePicker = false
                    }
                }

                Button {
                    withAnimation(.easeInOut(duration: 0.16)) { showingAI.toggle() }
                } label: {
                    Label("Fill with AI", systemImage: "sparkles")
                }
                .buttonStyle(SecondaryButtonStyle())
                .accessibilityLabel("Fill with AI")
                .accessibilityIdentifier("worldClockFillWithAIButton")
                .help("Turn a description such as 'Singapore, London, and San Francisco next Tuesday' into locations and a time")

                Spacer()
            }

            if showingAI {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        TextField(
                            "Singapore, London, and San Francisco next Tuesday afternoon",
                            text: $viewModel.aiRequest
                        )
                        .textFieldStyle(.roundedBorder)
                        .onSubmit { viewModel.fillWithAI() }
                        .accessibilityIdentifier("worldClockAIRequest")

                        if viewModel.isResolvingAI {
                            ProgressView().controlSize(.small)
                        }

                        Button("Fill") { viewModel.fillWithAI() }
                            .buttonStyle(PrimaryButtonStyle())
                            .disabled(viewModel.isResolvingAI)
                    }

                    if !viewModel.canUseAI {
                        HStack(spacing: 5) {
                            Text("An AI provider is not configured.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Button("Open Settings") { onOpenSettings() }
                                .buttonStyle(.link)
                                .font(.caption)
                        }
                    }

                    if let message = viewModel.aiMessage {
                        Text(message)
                            .font(.caption)
                            .foregroundStyle(viewModel.aiMessageIsError ? Color.red : Color.secondary)
                            .textSelection(.enabled)
                    }
                }
                .padding(10)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.035)))
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
        .background(Color.primary.opacity(0.025))
    }
}

private struct MeetingTimelineView: View {
    let qualities: [MeetingTimeQuality]
    @Binding var offset: TimeInterval
    let duration: TimeInterval

    var body: some View {
        GeometryReader { proxy in
            let width = max(proxy.size.width, 1)
            ZStack(alignment: .leading) {
                HStack(spacing: 0) {
                    ForEach(Array(qualities.enumerated()), id: \.offset) { _, quality in
                        Rectangle().fill(quality.color)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 5))

                Rectangle()
                    .fill(.white)
                    .frame(width: 2, height: proxy.size.height + 4)
                    .shadow(color: .black.opacity(0.5), radius: 1)
                    .offset(x: markerX(width: width) - 1)

                Circle()
                    .fill(.white)
                    .overlay(Circle().stroke(Color.black.opacity(0.28), lineWidth: 1))
                    .frame(width: 10, height: 10)
                    .offset(x: markerX(width: width) - 5)
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        offset = min(max(value.location.x / width, 0), 1) * duration
                    }
            )
        }
        .accessibilityElement()
        .accessibilityLabel("Meeting quality across the day")
    }

    private func markerX(width: CGFloat) -> CGFloat {
        guard duration > 0 else { return 0 }
        return min(max(offset / duration, 0), 1) * width
    }
}

private struct WorldClockZoneRow: View {
    let zone: WorldClockZonePresentation
    let canRemove: Bool
    let onMakeAnchor: () -> Void
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Circle()
                .fill(zone.quality.color)
                .frame(width: 9, height: 9)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(zone.name).font(.callout.weight(.semibold))
                    if zone.isAnchor {
                        Text("REFERENCE")
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(BoxTheme.accent)
                    }
                }
                Text("\(zone.dateText) - \(zone.zoneText)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()
            QualityBadge(quality: zone.quality)
            Text(zone.timeText)
                .font(.system(size: 24, weight: .medium, design: .rounded))
                .monospacedDigit()
                .frame(minWidth: 112, alignment: .trailing)

            Button(action: onMakeAnchor) {
                Image(systemName: zone.isAnchor ? "mappin.circle.fill" : "mappin.circle")
            }
            .buttonStyle(.borderless)
            .disabled(zone.isAnchor)
            .help(zone.isAnchor ? "Reference location" : "Use this location for the date and timeline")

            Button(action: onRemove) {
                Image(systemName: "xmark.circle")
            }
            .buttonStyle(.borderless)
            .disabled(!canRemove)
            .help(canRemove ? "Remove \(zone.name)" : "Keep at least one location")
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 10)
        .background(zone.quality.color.opacity(0.035))
        .accessibilityElement(children: .contain)
    }
}

private struct QualityBadge: View {
    let quality: MeetingTimeQuality

    var body: some View {
        HStack(spacing: 5) {
            Circle().fill(quality.color).frame(width: 7, height: 7)
            Text(quality.shortLabel)
        }
        .font(.caption2.weight(.semibold))
        .foregroundStyle(quality.color)
        .padding(.horizontal, 7)
        .padding(.vertical, 4)
        .background(Capsule().fill(quality.color.opacity(0.11)))
        .accessibilityLabel(quality.label)
    }
}

private struct WorldClockZonePicker: View {
    @ObservedObject var viewModel: WorldClockViewModel
    var onAdded: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Add a location")
                .font(.headline)
            TextField("City or IANA time zone", text: $viewModel.searchQuery)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("worldClockZoneSearch")

            ScrollView {
                LazyVStack(spacing: 2) {
                    ForEach(viewModel.searchResults) { option in
                        Button {
                            viewModel.addZone(option.id)
                            onAdded()
                        } label: {
                            HStack {
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(option.name).font(.callout.weight(.medium))
                                    Text(option.subtitle)
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                        .lineLimit(1)
                                }
                                Spacer()
                                Image(systemName: "plus.circle")
                                    .foregroundStyle(BoxTheme.accent)
                            }
                            .padding(.horizontal, 8)
                            .padding(.vertical, 6)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .frame(height: 240)
        }
        .padding(14)
        .frame(width: 360)
    }
}

private extension MeetingTimeQuality {
    var color: Color {
        switch self {
        case .working: return Color(red: 0.12, green: 0.62, blue: 0.31)
        case .extended: return Color(red: 0.92, green: 0.52, blue: 0.08)
        case .poor: return Color(red: 0.82, green: 0.20, blue: 0.19)
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
