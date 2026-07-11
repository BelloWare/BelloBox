import Combine
import Foundation

@MainActor
final class CodexTokenUsageViewModel: ObservableObject {
    @Published private(set) var scanResult: CodexTokenUsageScanResult
    @Published private(set) var isLoading = false
    @Published private(set) var errorMessage: String?
    @Published var selectedMetric: CodexTokenUsageMetric = .output
    @Published var selectedInterval: CodexTokenUsageInterval = .automatic
    @Published var selectedRangePreset: CodexTokenUsageRangePreset = .threeDays
    @Published var selectedModelName: String?
    @Published private(set) var visibleRange: DateInterval
    @Published private(set) var zoomStack: [DateInterval] = []

    private let store: CodexTokenUsageStore
    private let nowProvider: @MainActor () -> Date
    private var hasLoaded = false

    init(
        store: CodexTokenUsageStore = CodexTokenUsageStore(),
        nowProvider: @escaping @MainActor () -> Date = { Date() }
    ) {
        self.store = store
        self.nowProvider = nowProvider
        let initialRange = CodexTokenUsageRangePreset.threeDays.dateInterval(endingAt: nowProvider())
        scanResult = .empty(codexHome: store.codexHome)
        visibleRange = initialRange
    }

    var report: CodexTokenUsageReport {
        CodexTokenUsageAggregator.report(
            events: scanResult.events,
            range: visibleRange,
            intervalPreference: selectedInterval,
            modelFilter: selectedModelName
        )
    }

    var availableModelNames: [String] {
        let names = Set(scanResult.events.map(\.displayModel))
        return names.sorted()
    }

    var outputRates: (perMinute: Double, perHour: Double, perDay: Double) {
        let output = Double(report.total.outputTokens)
        let duration = max(1, visibleRange.duration)
        return (
            perMinute: output / (duration / 60),
            perHour: output / (duration / 3_600),
            perDay: output / (duration / 86_400)
        )
    }

    func loadIfNeeded() {
        guard !hasLoaded else { return }
        hasLoaded = true
        load()
    }

    func load() {
        Task { await reload() }
    }

    func reload() async {
        isLoading = true
        errorMessage = nil
        let store = store

        do {
            let result = try await Task.detached(priority: .userInitiated) {
                try store.loadEvents()
            }.value
            scanResult = result
            selectedModelName = normalizedModelSelection(selectedModelName)
        } catch {
            errorMessage = error.localizedDescription
        }
        isLoading = false
    }

    func setRangePreset(_ preset: CodexTokenUsageRangePreset) {
        selectedRangePreset = preset
        zoomStack.removeAll()
        visibleRange = preset.dateInterval(endingAt: nowProvider())
    }

    func zoom(to range: DateInterval) {
        let bounded = boundedRange(range)
        guard bounded.duration >= 60 else { return }
        zoomStack.append(visibleRange)
        visibleRange = bounded
    }

    func goBack() {
        guard let previous = zoomStack.popLast() else { return }
        visibleRange = previous
    }

    private func normalizedModelSelection(_ modelName: String?) -> String? {
        guard let modelName else { return nil }
        return availableModelNames.contains(modelName) ? modelName : nil
    }

    private func boundedRange(_ range: DateInterval) -> DateInterval {
        let start = min(range.start, range.end)
        let end = max(range.start, range.end)
        let minimum = visibleRange.start.addingTimeInterval(-selectedRangePreset.duration * 10)
        let maximum = nowProvider().addingTimeInterval(60)
        return DateInterval(start: max(start, minimum), end: min(end, maximum))
    }
}
