import Darwin
import Foundation

/// Prepare a complete file beside its destination, then publish it with one atomic
/// rename. A failed copy, export or cancellation leaves the previous file intact.
enum RecordingFileStore {
    static func resolved(_ url: URL) -> URL {
        url.standardizedFileURL.resolvingSymlinksInPath()
    }

    static func stagedURL(for destination: URL) -> URL {
        resolved(destination).deletingLastPathComponent()
            .appendingPathComponent(".BelloBox-export-\(UUID().uuidString).mov")
    }

    static func copy(from source: URL, to destination: URL) throws {
        try Task.checkCancellation()
        let source = resolved(source)
        let destination = resolved(destination)
        guard source != destination else { return }
        let staged = stagedURL(for: destination)
        defer { try? FileManager.default.removeItem(at: staged) }
        try FileManager.default.copyItem(at: source, to: staged)
        try publish(staged, to: destination)
    }

    static func publish(_ staged: URL, to destination: URL) throws {
        try Task.checkCancellation()
        let destination = resolved(destination)
        // The staging file lives on the destination volume. rename() replaces a
        // file atomically and fails without removing an existing directory.
        guard Darwin.rename(staged.path, destination.path) == 0 else {
            let code = errno
            throw NSError(domain: NSPOSIXErrorDomain, code: Int(code),
                          userInfo: [NSFilePathErrorKey: destination.path])
        }
    }
}
