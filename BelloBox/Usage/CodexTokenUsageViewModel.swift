import Combine
import Foundation

@MainActor
final class CodexTokenUsageViewModel: ObservableObject {
    @Published private(set) var scanResult: CodexTokenUsageScanResult
    @Published private(set) var isLoading = false
    @Published private(set) var hasCompletedInitialLoad = false
    @Published private(set) var errorMessage: String?
    @Published var selectedMetric: CodexTokenUsageMetric = .output
    @Published var selectedInterval: CodexTokenUsageInterval = .automatic {
        didSet {
            if selectedInterval != oldValue { rebuildReport() }
        }
    }
    @Published var selectedRangePreset: CodexTokenUsageRangePreset = .threeDays
    @Published var selectedModelName: String? {
        didSet {
            if selectedModelName != oldValue { rebuildReport() }
        }
    }
    @Published private(set) var visibleRange: DateInterval
    @Published private(set) var zoomStack: [DateInterval] = []
    @Published private(set) var report: CodexTokenUsageReport
    @Published private(set) var availableModelNames: [String] = []

    private let store: CodexTokenUsageStore
    private let nowProvider: @MainActor () -> Date
    private var sourceRange: DateInterval
    private var hasLoaded = false
    private var loadGeneration = 0
    private var loadTask: Task<Void, Never>?

    init(
        store: CodexTokenUsageStore = CodexTokenUsageStore(),
        nowProvider: @escaping @MainActor () -> Date = { Date() }
    ) {
        self.store = store
        self.nowProvider = nowProvider
        let initialRange = CodexTokenUsageRangePreset.threeDays.dateInterval(endingAt: nowProvider())
        scanResult = .empty(codexHome: store.codexHome)
        visibleRange = initialRange
        sourceRange = initialRange
        report = CodexTokenUsageAggregator.report(
            events: [],
            range: initialRange,
            intervalPreference: .automatic,
            modelFilter: nil
        )
    }

    func loadIfNeeded() {
        guard !hasLoaded else { return }
        hasLoaded = true
        load()
    }

    func load() {
        loadTask?.cancel()
        loadGeneration += 1
        let generation = loadGeneration
        let range = sourceRange
        isLoading = true
        errorMessage = nil
        loadTask = Task { [weak self] in
            await self?.performReload(range: range, generation: generation)
        }
    }

    func reload() async {
        loadTask?.cancel()
        loadGeneration += 1
        let generation = loadGeneration
        let range = sourceRange
        isLoading = true
        errorMessage = nil
        await performReload(range: range, generation: generation)
    }

    private func performReload(range: DateInterval, generation: Int) async {
        let store = store
        let worker = Task.detached(priority: .userInitiated) {
            try await store.loadEventsConcurrently(in: range)
        }

        do {
            let result = try await withTaskCancellationHandler {
                try await worker.value
            } onCancel: {
                worker.cancel()
            }
            guard generation == loadGeneration else { return }
            scanResult = result
            rebuildDerivedData()
        } catch is CancellationError {
            guard generation == loadGeneration else { return }
        } catch {
            guard generation == loadGeneration else { return }
            errorMessage = error.localizedDescription
        }
        if generation == loadGeneration {
            hasCompletedInitialLoad = true
            isLoading = false
        }
    }

    func setRangePreset(_ preset: CodexTokenUsageRangePreset) {
        selectedRangePreset = preset
        zoomStack.removeAll()
        sourceRange = preset.dateInterval(endingAt: nowProvider())
        visibleRange = sourceRange
        rebuildReport()
        if hasLoaded { load() }
    }

    func zoom(to range: DateInterval) {
        let bounded = boundedRange(range)
        guard bounded.duration >= 60 else { return }
        zoomStack.append(visibleRange)
        visibleRange = bounded
        rebuildReport()
    }

    func goBack() {
        guard let previous = zoomStack.popLast() else { return }
        visibleRange = previous
        rebuildReport()
    }

    private func rebuildDerivedData() {
        availableModelNames = Set(scanResult.events.map(\.displayModel)).sorted()
        if let selectedModelName, !availableModelNames.contains(selectedModelName) {
            self.selectedModelName = nil
            return
        }
        rebuildReport()
    }

    private func rebuildReport() {
        report = CodexTokenUsageAggregator.report(
            events: scanResult.events,
            range: visibleRange,
            intervalPreference: selectedInterval,
            modelFilter: selectedModelName
        )
    }

    private func boundedRange(_ range: DateInterval) -> DateInterval {
        let start = min(range.start, range.end)
        let end = max(range.start, range.end)
        let minimum = visibleRange.start.addingTimeInterval(-selectedRangePreset.duration * 10)
        let maximum = nowProvider().addingTimeInterval(60)
        return DateInterval(start: max(start, minimum), end: min(end, maximum))
    }
}
