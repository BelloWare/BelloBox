import SwiftUI

struct WorldClockView: View {
    @ObservedObject var viewModel: WorldClockViewModel
    var onOpenSettings: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var showingZonePicker = false
    @State private var showingAI = false
    @FocusState private var aiFocused: Bool
    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    timelineSection
                    locationsSection
                }
                .padding(20)
            }
            .accessibilityIdentifier("worldClockLocationList")
            Divider()
            footer
        }
        .frame(minWidth: 780, minHeight: 640)
        .background(WorkspaceBackground()).tint(BoxTheme.accent)
        .onReceive(timer) { viewModel.refreshCurrentTime($0) }
        .sheet(isPresented: $showingZonePicker) {
            WorldClockZonePicker(viewModel: viewModel) { showingZonePicker = false }
        }
        .onChange(of: showingAI) { visible in
            if !visible { aiFocused = false; viewModel.cancelAI() }
        }
    }

    private var header: some View {
        HStack(spacing: 12) {
            ToolBadge(symbol: "globe.americas.fill", size: 40)
            VStack(alignment: .leading, spacing: 4) {
                Text("World Clock").font(.system(size: 22, weight: .semibold))
                Text("One moment. Every time zone.").font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            Label(viewModel.isFollowingNow ? "Live time" : "Planning", systemImage: viewModel.isFollowingNow ? "dot.radiowaves.left.and.right" : "calendar")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(viewModel.isFollowingNow ? BoxTheme.success : BoxTheme.accent)
                .padding(.horizontal, 10).padding(.vertical, 6)
                .background(BoxTheme.well, in: Capsule())
                .accessibilityIdentifier("worldClockTimeMode")
            Button("Now", action: viewModel.goToNow)
                .buttonStyle(SecondaryButtonStyle())
                .keyboardShortcut("n", modifiers: .command)
                .help("Return to live time (⌘N)")
        }
        .padding(20)
    }

    private var timelineSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 8) {
                Label("MEETING PLANNER", systemImage: "calendar.badge.clock")
                    .font(.system(size: 10, weight: .semibold)).tracking(1).foregroundStyle(.secondary)
                Spacer()
                Menu {
                    ForEach(viewModel.zonePresentations) { zone in
                        Button { viewModel.setAnchorZone(zone.id) } label: {
                            if zone.isAnchor { Label(zone.name, systemImage: "checkmark") }
                            else { Text(zone.name) }
                        }
                    }
                } label: {
                    Label("Reference: \(viewModel.anchorName)", systemImage: "mappin.and.ellipse")
                        .font(.caption)
                }
                .menuStyle(.borderlessButton).fixedSize()
                .help("The date and timeline use this location")
            }

            HStack(spacing: 10) {
                clockIconButton("chevron.left", label: "Previous day") { viewModel.moveDay(by: -1) }
                DatePicker("Date", selection: Binding(
                    get: { viewModel.timeline.start }, set: { viewModel.selectDay(containing: $0) }
                ), displayedComponents: .date)
                .labelsHidden().environment(\.timeZone, viewModel.anchorTimeZone)
                .accessibilityIdentifier("worldClockDatePicker")
                clockIconButton("chevron.right", label: "Next day") { viewModel.moveDay(by: 1) }
                Divider().frame(height: 22).padding(.horizontal, 3)
                DatePicker("Time", selection: Binding(
                    get: { viewModel.selectedInstant }, set: { viewModel.focus(on: $0) }
                ), displayedComponents: .hourAndMinute)
                .environment(\.timeZone, viewModel.anchorTimeZone).fixedSize()
                .accessibilityIdentifier("worldClockExactTime")
                Spacer(minLength: 8)
                QualityBadge(quality: viewModel.selectedMeetingQuality)
                    .help("Combined availability across all locations; based on local hours")
            }

            VStack(spacing: 8) {
                MeetingTimelineView(qualities: viewModel.timelineQualities, offset: Binding(
                    get: { viewModel.selectedOffset }, set: { viewModel.selectedOffset = $0 }
                ), duration: viewModel.timeline.duration)
                .frame(height: 20)
                // The native slider below exposes the same value with keyboard support.
                .accessibilityHidden(true)
                Slider(value: Binding(
                    get: { viewModel.selectedOffset }, set: { viewModel.selectedOffset = $0 }
                ), in: 0...viewModel.timeline.duration, step: viewModel.timelineStep)
                .accessibilityLabel("Meeting time")
                .accessibilityValue(viewModel.selectedTimeTitle)
                .accessibilityIdentifier("worldClockTimeSlider")
                HStack {
                    Text(viewModel.dayStartLabel)
                    Spacer()
                    ForEach([MeetingTimeQuality.working, .extended, .poor], id: \.self) { quality in
                        Label(quality.shortLabel, systemImage: quality.symbol)
                            .foregroundStyle(quality.color)
                    }
                    Spacer()
                    Text(viewModel.dayEndLabel)
                }
                .font(.system(size: 10)).foregroundStyle(.secondary)
            }
        }
        .padding(16).surfaceCard()
    }

    private var locationsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("YOUR LOCATIONS").font(.system(size: 10, weight: .semibold)).tracking(1)
                Text("\(viewModel.zoneIDs.count)").font(.caption).monospacedDigit()
                Spacer()
                Text("All times stay in sync").font(.caption)
            }.foregroundStyle(.secondary)
            LazyVStack(spacing: 8) {
                ForEach(viewModel.zonePresentations) { zone in
                    WorldClockZoneRow(zone: zone, canRemove: viewModel.canRemoveZone,
                        onMakeAnchor: { viewModel.setAnchorZone(zone.id) },
                        onRemove: { viewModel.removeZone(zone.id) })
                }
            }
        }
    }

    private var footer: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Button { showingZonePicker = true } label: { Label("Add Location", systemImage: "plus") }
                    .buttonStyle(SecondaryButtonStyle())
                    .keyboardShortcut("l", modifiers: .command)
                    .help("Add a location (⌘L)")
                    .accessibilityIdentifier("worldClockAddLocationButton")
                Button {
                    withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.16)) { showingAI.toggle() }
                } label: { Label(showingAI ? "Close AI" : "Fill with AI", systemImage: "sparkles") }
                    .buttonStyle(SecondaryButtonStyle())
                    .accessibilityIdentifier("worldClockFillWithAIButton")
                    .help("Describe locations and a time to your configured AI provider")
                Spacer()
                if let message = viewModel.copyMessage {
                    Text(message).font(.caption).foregroundStyle(.secondary)
                }
                Button(action: viewModel.copyMeeting) { Label("Copy Times", systemImage: "doc.on.doc") }
                    .buttonStyle(PrimaryButtonStyle())
                    .keyboardShortcut("c", modifiers: [.command, .shift])
                    .help("Copy the selected date and time for every location (⇧⌘C)")
            }
            if showingAI { aiSection }
        }
        .padding(16).background(BoxTheme.surface)
    }

    private var aiSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Describe your locations and meeting time").font(.caption.weight(.semibold))
            HStack(spacing: 8) {
                TextField("Singapore, London, and San Francisco next Tuesday afternoon", text: $viewModel.aiRequest)
                    .textFieldStyle(.roundedBorder).focused($aiFocused)
                    .task { aiFocused = true }
                    .onSubmit { viewModel.fillWithAI() }
                    .accessibilityIdentifier("worldClockAIRequest")
                if viewModel.isResolvingAI {
                    ProgressView().controlSize(.small)
                    Button("Cancel", action: viewModel.cancelAI).buttonStyle(SecondaryButtonStyle())
                }
                Button("Fill", action: viewModel.fillWithAI).buttonStyle(PrimaryButtonStyle())
                    .disabled(viewModel.isResolvingAI || !viewModel.canUseAI || viewModel.aiRequest.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            if !viewModel.canUseAI {
                HStack(spacing: 5) {
                    Text("Connect an AI provider to use this option.").foregroundStyle(.secondary)
                    Button("Open Settings", action: onOpenSettings).buttonStyle(.link)
                }.font(.caption)
            }
            if let message = viewModel.aiMessage {
                Text(message).font(.caption)
                    .foregroundStyle(viewModel.aiMessageIsError ? BoxTheme.danger : Color.secondary)
                    .textSelection(.enabled)
            }
        }.padding(12).background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 10))
    }
}

private func clockIconButton(_ symbol: String, label: String, action: @escaping () -> Void) -> some View {
    Button(action: action) {
        Image(systemName: symbol).font(.system(size: 12, weight: .medium))
            .frame(width: 28, height: 28)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 7))
            .contentShape(RoundedRectangle(cornerRadius: 7))
    }.buttonStyle(.plain).help(label).accessibilityLabel(label)
}

private struct MeetingTimelineView: View {
    let qualities: [MeetingTimeQuality]
    @Binding var offset: TimeInterval
    let duration: TimeInterval

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
            }
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0).onChanged { value in
                offset = min(max((value.location.x - 5) / max(width - 10, 1), 0), 1) * duration
            })
        }
    }
}

private struct WorldClockZoneRow: View {
    let zone: WorldClockZonePresentation
    let canRemove: Bool
    let onMakeAnchor: () -> Void
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: zone.quality.symbol).font(.system(size: 17))
                .foregroundStyle(zone.quality.color).frame(width: 38, height: 38)
                .background(zone.quality.color.opacity(0.09), in: RoundedRectangle(cornerRadius: 11))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 5) {
                HStack(spacing: 8) {
                    Text(zone.name).font(.system(size: 14, weight: .semibold)).lineLimit(1)
                    if zone.isAnchor {
                        Text("REFERENCE").font(.system(size: 8, weight: .bold)).tracking(0.5)
                            .foregroundStyle(BoxTheme.accent)
                            .padding(.horizontal, 6).padding(.vertical, 3)
                            .background(BoxTheme.accentSoft, in: RoundedRectangle(cornerRadius: 4))
                    }
                }
                Text(zone.zoneText).font(.system(size: 11)).foregroundStyle(.secondary)
            }
            Spacer(minLength: 6)
            QualityBadge(quality: zone.quality).frame(width: 86)
            VStack(alignment: .trailing, spacing: 4) {
                Text(zone.timeText).font(.system(size: 26, weight: .medium, design: .rounded)).monospacedDigit()
                HStack(spacing: 5) {
                    Text(zone.dateText)
                    if zone.dayDifference != 0 {
                        Text(zone.dayDifference > 0 ? "+\(zone.dayDifference)d" : "\(zone.dayDifference)d")
                            .foregroundStyle(BoxTheme.accent).fontWeight(.semibold)
                            .help("\(zone.dayDifference) calendar days from the reference location")
                    }
                }.font(.system(size: 10)).foregroundStyle(.secondary)
            }.frame(minWidth: 145, alignment: .trailing)
            clockIconButton(zone.isAnchor ? "mappin.circle.fill" : "mappin", label: "Use \(zone.name) as reference", action: onMakeAnchor)
                .foregroundStyle(zone.isAnchor ? BoxTheme.accent : .secondary).disabled(zone.isAnchor)
            clockIconButton("xmark", label: canRemove ? "Remove \(zone.name)" : "Keep at least one location", action: onRemove)
                .foregroundStyle(.secondary).disabled(!canRemove)
        }
        .padding(14).surfaceCard()
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(zone.isAnchor ? BoxTheme.accent.opacity(0.4) : .clear))
        .accessibilityElement(children: .contain)
    }
}

private struct QualityBadge: View {
    let quality: MeetingTimeQuality
    var body: some View {
        Label(quality.shortLabel, systemImage: quality.symbol)
            .font(.system(size: 10, weight: .semibold)).foregroundStyle(quality.color)
            .padding(.horizontal, 8).padding(.vertical, 5)
            .background(quality.color.opacity(0.09), in: RoundedRectangle(cornerRadius: 6))
            .accessibilityLabel(quality.label)
    }
}

private struct WorldClockZonePicker: View {
    @ObservedObject var viewModel: WorldClockViewModel
    var onClose: () -> Void
    @State private var selectedID: String?

    private var results: [WorldClockZoneOption] { viewModel.searchResults }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                ToolBadge(symbol: "globe", size: 32)
                Text("Add a location").font(.system(size: 16, weight: .semibold))
                Spacer()
                clockIconButton("xmark", label: "Cancel (Esc)", action: onClose)
            }.padding(16)
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                LauncherSearchField(text: $viewModel.searchQuery, onMove: move, onSubmit: addSelected,
                    onEscape: onClose, onReady: { _ in }, placeholder: "Search city or time zone…",
                    accessibilityID: "worldClockZoneSearch", accessibilityLabel: "Search locations", fontSize: 16)
                    .frame(height: 26)
                if !viewModel.searchQuery.isEmpty {
                    Button { viewModel.searchQuery = "" } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary) }
                        .buttonStyle(.plain).help("Clear search").accessibilityLabel("Clear search")
                }
            }.padding(12).background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 10)).padding(.horizontal, 16)
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 4) {
                        if results.isEmpty {
                            VStack(spacing: 8) {
                                Image(systemName: "globe").font(.title2).foregroundStyle(BoxTheme.teal)
                                Text("No matching locations").font(.headline)
                                Text("Try a nearby city or Asia/Tokyo. Locations already added are hidden.")
                                    .font(.caption).foregroundStyle(.secondary)
                            }.multilineTextAlignment(.center).padding(24)
                        }
                        ForEach(results) { option in
                            Button { selectedID = option.id; addSelected() } label: {
                                HStack(spacing: 10) {
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(option.name).font(.system(size: 13, weight: .medium))
                                        Text(option.subtitle).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1)
                                    }
                                    Spacer()
                                    Image(systemName: selectedID == option.id ? "return" : "plus").foregroundStyle(BoxTheme.accent)
                                }
                                .padding(10).frame(maxWidth: .infinity, alignment: .leading)
                                .background(selectedID == option.id ? BoxTheme.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 8))
                                .contentShape(Rectangle())
                            }.buttonStyle(.plain).id(option.id)
                                .accessibilityValue(selectedID == option.id ? "Selected" : "Not selected")
                        }
                    }.padding(10)
                }.frame(height: 280)
                    .onChange(of: selectedID) { id in if let id { proxy.scrollTo(id, anchor: .center) } }
            }
            Divider()
            HStack(spacing: 8) {
                ShortcutBadge(text: "↑↓")
                Text("Navigate").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("Cancel", action: onClose).buttonStyle(SecondaryButtonStyle())
                Button("Add Location", action: addSelected).buttonStyle(PrimaryButtonStyle()).disabled(selectedID == nil)
            }.padding(12).background(BoxTheme.surface)
        }
        .frame(width: 440).background(WorkspaceBackground()).tint(BoxTheme.accent)
        .onAppear { viewModel.searchQuery = ""; selectedID = results.first?.id }
        .onChange(of: viewModel.searchQuery) { _ in selectedID = results.first?.id }
        .onExitCommand(perform: onClose)
    }

    private func move(_ direction: Int) {
        guard !results.isEmpty else { selectedID = nil; return }
        let current = results.firstIndex { $0.id == selectedID } ?? 0
        selectedID = results[(current + direction % results.count + results.count) % results.count].id
    }
    private func addSelected() {
        guard let selectedID, results.contains(where: { $0.id == selectedID }) else { return }
        viewModel.addZone(selectedID)
        onClose()
    }
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
