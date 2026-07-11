import Charts
import SwiftUI

struct CodexTokenUsageDashboardView: View {
    @StateObject private var viewModel: CodexTokenUsageViewModel
    @State private var dragStartX: CGFloat?
    @State private var dragCurrentX: CGFloat?

    @MainActor
    init(viewModel: CodexTokenUsageViewModel? = nil) {
        _viewModel = StateObject(wrappedValue: viewModel ?? CodexTokenUsageViewModel())
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            header
            controls
            if let error = viewModel.errorMessage {
                errorView(error)
            }
            summary
            chartPanel
            modelBreakdown
        }
        .padding(22)
        .frame(minWidth: 940, minHeight: 720)
        .task { viewModel.loadIfNeeded() }
        .accessibilityIdentifier("codexUsageDashboard")
    }

    private var header: some View {
        HStack(spacing: 12) {
            Image(systemName: "chart.xyaxis.line")
                .font(.system(size: 20, weight: .bold))
                .foregroundStyle(.white)
                .frame(width: 42, height: 42)
                .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(BoxTheme.accentGradient))
            VStack(alignment: .leading, spacing: 2) {
                Text("Codex Token Usage")
                    .font(.title2.weight(.bold))
                Text("\(viewModel.scanResult.filesScanned) files scanned from \(viewModel.scanResult.codexHome.path)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer()
            if viewModel.isLoading {
                ProgressView()
                    .scaleEffect(0.75)
                    .accessibilityIdentifier("codexUsageLoading")
            }
            Button { viewModel.load() } label: {
                Label("Refresh", systemImage: "arrow.clockwise")
            }
            .buttonStyle(SecondaryButtonStyle())
        }
    }

    private var controls: some View {
        HStack(spacing: 12) {
            Picker("Range", selection: Binding(
                get: { viewModel.selectedRangePreset },
                set: { viewModel.setRangePreset($0) }
            )) {
                ForEach(CodexTokenUsageRangePreset.allCases) { preset in
                    Text(preset.title).tag(preset)
                }
            }
            .pickerStyle(.segmented)
            .frame(width: 240)
            .accessibilityIdentifier("codexUsageRangePicker")

            Picker("Metric", selection: $viewModel.selectedMetric) {
                ForEach(CodexTokenUsageMetric.allCases) { metric in
                    Text(metric.title).tag(metric)
                }
            }
            .frame(width: 170)
            .accessibilityIdentifier("codexUsageMetricPicker")

            Picker("Interval", selection: $viewModel.selectedInterval) {
                ForEach(CodexTokenUsageInterval.allCases) { interval in
                    Text(interval.title).tag(interval)
                }
            }
            .frame(width: 120)
            .accessibilityIdentifier("codexUsageIntervalPicker")

            Picker("Model", selection: $viewModel.selectedModelName) {
                Text("All models").tag(String?.none)
                ForEach(viewModel.availableModelNames, id: \.self) { modelName in
                    Text(modelName).tag(String?.some(modelName))
                }
            }
            .frame(minWidth: 180, maxWidth: 260)
            .accessibilityIdentifier("codexUsageModelPicker")

            Spacer()
        }
    }

    private var summary: some View {
        let report = viewModel.report
        let rates = report.activeOutputAverages
        return VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 10) {
                metricCard(title: "Total", value: report.total.totalTokens)
                metricCard(title: "Output", value: report.total.outputTokens)
                metricCard(title: "Input", value: report.total.inputTokens)
                metricCard(title: "Cached", value: report.total.cachedInputTokens)
                metricCard(title: "Uncached", value: report.total.uncachedInputTokens)
            }
            HStack(spacing: 10) {
                rateCard(
                    title: "Output / active min",
                    value: rates.perMinute,
                    help: "Average output tokens across wall-clock minutes containing reported output. This is usage, not streaming speed."
                )
                rateCard(
                    title: "Output / active hour",
                    value: rates.perHour,
                    help: "Average output tokens across wall-clock hours containing reported output. This is usage, not streaming speed."
                )
                rateCard(
                    title: "Output / active day",
                    value: rates.perDay,
                    help: "Average output tokens across wall-clock days containing reported output. This is usage, not streaming speed."
                )
            }
        }
    }

    private var chartPanel: some View {
        let report = viewModel.report
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("\(viewModel.selectedMetric.title) by \(report.resolvedInterval.title.lowercased())")
                    .font(.headline)
                Spacer()
                Text("\(report.eventCount) token events")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            chartRangeIndicator

            Group {
                if viewModel.isLoading && !viewModel.hasCompletedInitialLoad {
                    loadingChart
                } else if report.buckets.isEmpty {
                    emptyChart
                } else {
                    Chart(report.buckets) { bucket in
                        BarMark(
                            x: .value("Time", bucket.start),
                            y: .value(viewModel.selectedMetric.title, viewModel.selectedMetric.value(in: bucket.usage))
                        )
                        .foregroundStyle(BoxTheme.accentGradient)
                        .cornerRadius(2)
                    }
                    .chartXScale(domain: report.range.start...report.range.end)
                    .chartYAxis {
                        AxisMarks(position: .leading)
                    }
                    .chartOverlay { proxy in
                        GeometryReader { geometry in
                            chartInteractionOverlay(plotFrame: geometry[proxy.plotAreaFrame])
                        }
                    }
                }
            }
            .frame(height: 280)
            .accessibilityIdentifier("codexUsageChart")

            HStack {
                Text("Drag across the chart to zoom into a range.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Spacer()
                if !viewModel.scanResult.unreadableFiles.isEmpty {
                    Text("\(viewModel.scanResult.unreadableFiles.count) files could not be read")
                        .font(.caption2)
                        .foregroundStyle(.orange)
                }
            }
        }
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(.primary.opacity(0.045)))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.primary.opacity(0.06), lineWidth: 1))
    }

    private var chartRangeIndicator: some View {
        HStack(spacing: 7) {
            Image(systemName: viewModel.zoomStack.isEmpty ? "calendar" : "viewfinder")
                .foregroundStyle(viewModel.zoomStack.isEmpty ? Color.secondary : BoxTheme.accent)
            Text(viewModel.zoomStack.isEmpty ? "Showing" : "Selected range")
                .fontWeight(.semibold)
            Text(dateRangeText(viewModel.visibleRange))
                .monospacedDigit()
                .foregroundStyle(.secondary)
                .lineLimit(1)
            Spacer()
            if !viewModel.zoomStack.isEmpty {
                Button { viewModel.goBack() } label: {
                    Label("Previous range", systemImage: "arrow.uturn.backward")
                }
                .buttonStyle(.borderless)
                .accessibilityIdentifier("codexUsageBackButton")
            }
        }
        .font(.caption)
        .accessibilityIdentifier("codexUsageSelectedRange")
    }

    private var loadingChart: some View {
        VStack(spacing: 9) {
            ProgressView()
                .controlSize(.regular)
            Text("Loading local Codex usage…")
                .font(.callout.weight(.medium))
            Text("Scanning recent rollout files")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityIdentifier("codexUsageChartLoading")
    }

    private var emptyChart: some View {
        VStack(spacing: 8) {
            Image(systemName: "chart.bar.xaxis")
                .font(.system(size: 28, weight: .semibold))
                .foregroundStyle(.secondary)
            Text("No local Codex token events in this range")
                .font(.callout.weight(.medium))
            Text("Run Codex locally, then refresh this dashboard.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func chartInteractionOverlay(plotFrame: CGRect) -> some View {
        ZStack(alignment: .topLeading) {
            if let selection = dragSelection(width: plotFrame.width) {
                Rectangle()
                    .fill(BoxTheme.accent.opacity(0.24))
                    .overlay(Rectangle().stroke(BoxTheme.accent, lineWidth: 2))
                    .frame(width: selection.width, height: plotFrame.height)
                    .offset(x: plotFrame.minX + selection.minX, y: plotFrame.minY)
                    .allowsHitTesting(false)
            }

            if let dragRange = dragDateRange(width: plotFrame.width) {
                Text(dateRangeText(dragRange))
                    .font(.caption2.monospacedDigit().weight(.semibold))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: 6, style: .continuous)
                            .strokeBorder(BoxTheme.accent.opacity(0.5), lineWidth: 1)
                    )
                    .position(x: plotFrame.midX, y: plotFrame.minY + 16)
                    .allowsHitTesting(false)
            }

            Rectangle()
                .fill(.clear)
                .contentShape(Rectangle())
                .frame(width: plotFrame.width, height: plotFrame.height)
                .offset(x: plotFrame.minX, y: plotFrame.minY)
                .gesture(chartDragGesture(width: plotFrame.width))
        }
    }

    private var modelBreakdown: some View {
        let report = viewModel.report
        return VStack(alignment: .leading, spacing: 8) {
            Text("Model breakdown")
                .font(.headline)
            VStack(spacing: 0) {
                modelHeader
                Divider()
                if report.modelTotals.isEmpty {
                    Text("No model usage to show.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, minHeight: 42)
                } else {
                    ForEach(report.modelTotals) { row in
                        modelRow(row)
                        if row.id != report.modelTotals.last?.id {
                            Divider()
                        }
                    }
                }
            }
            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(.primary.opacity(0.035)))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.primary.opacity(0.06), lineWidth: 1))
        }
    }

    private var modelHeader: some View {
        HStack(spacing: 8) {
            tableText("Model", weight: .semibold, alignment: .leading)
            tableText("Input", weight: .semibold)
            tableText("Cached", weight: .semibold)
            tableText("Uncached", weight: .semibold)
            tableText("Output", weight: .semibold)
            tableText("Total", weight: .semibold)
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
    }

    private func modelRow(_ row: CodexTokenUsageModelTotal) -> some View {
        HStack(spacing: 8) {
            tableText(row.modelName, alignment: .leading)
            tableText(format(row.usage.inputTokens))
            tableText(format(row.usage.cachedInputTokens))
            tableText(format(row.usage.uncachedInputTokens))
            tableText(format(row.usage.outputTokens))
            tableText(format(row.usage.totalTokens), weight: .semibold)
        }
        .font(.caption)
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
    }

    private func metricCard(title: String, value: Int) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Text(format(value))
                .font(.title3.monospacedDigit().weight(.bold))
                .lineLimit(1)
                .minimumScaleFactor(0.7)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.primary.opacity(0.045)))
    }

    private func rateCard(title: String, value: Double, help: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Text(formatRate(value))
                .font(.callout.monospacedDigit().weight(.bold))
        }
        .padding(10)
        .frame(width: 150, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(BoxTheme.accentSoft))
        .help(help)
    }

    private func errorView(_ error: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
            Text(error)
                .font(.caption)
                .foregroundStyle(.secondary)
            Spacer()
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.orange.opacity(0.08)))
    }

    private func tableText(
        _ value: String,
        weight: Font.Weight = .regular,
        alignment: Alignment = .trailing
    ) -> some View {
        Text(value)
            .fontWeight(weight)
            .lineLimit(1)
            .truncationMode(.middle)
            .frame(maxWidth: .infinity, alignment: alignment)
    }

    private func chartDragGesture(width: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 8)
            .onChanged { value in
                let start = dragStartX ?? value.startLocation.x
                dragStartX = clamp(start, min: 0, max: width)
                dragCurrentX = clamp(value.location.x, min: 0, max: width)
            }
            .onEnded { value in
                let start = dragStartX ?? value.startLocation.x
                let end = dragCurrentX ?? value.location.x
                defer {
                    dragStartX = nil
                    dragCurrentX = nil
                }
                guard abs(end - start) >= 8 else { return }
                let startDate = date(at: clamp(start, min: 0, max: width), width: width)
                let endDate = date(at: clamp(end, min: 0, max: width), width: width)
                viewModel.zoom(to: DateInterval(start: min(startDate, endDate), end: max(startDate, endDate)))
            }
    }

    private func dragSelection(width: CGFloat) -> (minX: CGFloat, width: CGFloat)? {
        guard let start = dragStartX, let current = dragCurrentX else { return nil }
        let minX = clamp(min(start, current), min: 0, max: width)
        let maxX = clamp(max(start, current), min: 0, max: width)
        guard maxX > minX else { return nil }
        return (minX, maxX - minX)
    }

    private func dragDateRange(width: CGFloat) -> DateInterval? {
        guard let start = dragStartX, let current = dragCurrentX, abs(current - start) >= 8 else { return nil }
        let startDate = date(at: clamp(start, min: 0, max: width), width: width)
        let endDate = date(at: clamp(current, min: 0, max: width), width: width)
        return DateInterval(start: min(startDate, endDate), end: max(startDate, endDate))
    }

    private func date(at x: CGFloat, width: CGFloat) -> Date {
        guard width > 0 else { return viewModel.visibleRange.start }
        let fraction = min(1, max(0, Double(x / width)))
        return viewModel.visibleRange.start.addingTimeInterval(viewModel.visibleRange.duration * fraction)
    }

    private func dateRangeText(_ range: DateInterval) -> String {
        "\(formatDate(range.start)) - \(formatDate(range.end))"
    }

    private func formatDate(_ date: Date) -> String {
        Self.dateFormatter.string(from: date)
    }

    private func format(_ value: Int) -> String {
        Self.integerFormatter.string(from: NSNumber(value: value)) ?? "\(value)"
    }

    private func formatRate(_ value: Double) -> String {
        if value < 10 {
            return String(format: "%.2f", value)
        }
        if value < 100 {
            return String(format: "%.1f", value)
        }
        return format(Int(value.rounded()))
    }

    private func clamp(_ value: CGFloat, min minimum: CGFloat, max maximum: CGFloat) -> CGFloat {
        Swift.min(Swift.max(value, minimum), maximum)
    }

    private static let integerFormatter: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        return formatter
    }()

    private static let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .short
        formatter.timeStyle = .short
        return formatter
    }()
}
