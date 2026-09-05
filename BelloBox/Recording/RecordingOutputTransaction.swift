import Foundation

/// Owns only the files produced by one recording. Cancellation and the final rename
/// share a lock so a late export cannot publish after the recording was discarded.
final class RecordingOutputTransaction {
    let destinationURL: URL
    let captureURL: URL
    let mixedURL: URL
    private let lock = NSLock()
    private var cancelled = false
    private var published = false

    init(destinationURL: URL) {
        self.destinationURL = RecordingFileStore.resolved(destinationURL)
        let directory = self.destinationURL.deletingLastPathComponent()
        let name = destinationURL.deletingPathExtension().lastPathComponent
        captureURL = directory.appendingPathComponent("\(name)-capture-\(UUID().uuidString).mov")
        mixedURL = directory.appendingPathComponent("\(name)-mixed-\(UUID().uuidString).mov")
    }

    var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    func checkActive() throws {
        try Task.checkCancellation()
        if isCancelled { throw CancellationError() }
    }

    func cancelPublication() {
        lock.lock()
        cancelled = true
        lock.unlock()
    }

    func publish(_ completedURL: URL) throws -> URL {
        lock.lock()
        defer { lock.unlock() }
        guard !cancelled else { throw CancellationError() }
        try RecordingFileStore.publish(completedURL, to: destinationURL)
        published = true
        return destinationURL
    }

    /// Call only once the writer has stopped using its capture file.
    func discard() {
        lock.lock()
        defer { lock.unlock() }
        cancelled = true
        try? FileManager.default.removeItem(at: captureURL)
        try? FileManager.default.removeItem(at: mixedURL)
        // An older file at this path belongs to the user, not this transaction.
        if published { try? FileManager.default.removeItem(at: destinationURL) }
    }
}
