import Foundation

/// Keeps the source app in focus until its Accessibility selection has settled.
/// A late reply is discarded if the user changes app/window or cancels the request.
@MainActor
final class SelectionRequest {
    private var task: Task<Void, Never>?
    private var generation = UUID()
    var isPending: Bool { task != nil }

    func cancel() {
        generation = UUID()
        task?.cancel()
        task = nil
    }

    func start(read: @escaping () -> TextSelection?, isCurrent: @escaping () -> Bool,
               sleep: @escaping (UInt64) async throws -> Void = { try await Task.sleep(nanoseconds: $0) },
               completion: @escaping (TextSelection?) -> Void) {
        cancel()
        guard isCurrent() else { return }
        if let selection = read() {
            if isCurrent() { completion(selection) }
            return
        }
        let id = generation
        task = Task { [weak self] in
            // Double-click selection can reach AX after the keyboard shortcut.
            // Never activate our panel during these bounded, nonblocking retries.
            for delay in [UInt64(35_000_000), 65_000_000, 100_000_000] {
                do { try await sleep(delay) } catch { return }
                guard let self, self.generation == id, !Task.isCancelled else { return }
                guard isCurrent() else { self.task = nil; return }
                if let selection = read() {
                    guard isCurrent() else { self.task = nil; return }
                    self.task = nil
                    completion(selection)
                    return
                }
            }
            guard let self, self.generation == id, !Task.isCancelled else { return }
            self.task = nil
            if isCurrent() { completion(nil) }
        }
    }
}
