import AppKit
import CoreImage
import CoreMedia
import ScreenCaptureKit

final class ScreenCaptureFrameGrabber: NSObject, SCStreamOutput, SCStreamDelegate {
    private let context = CIContext()
    private let lock = NSLock()
    private var continuation: CheckedContinuation<CGImage, Error>?
    private var stream: SCStream?
    private var timeoutTask: Task<Void, Never>?

    func capture(filter: SCContentFilter, configuration: SCStreamConfiguration) async throws -> CGImage {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                let queue = DispatchQueue(label: "BelloBox.ScreenCaptureFrameGrabber")
                let stream = SCStream(filter: filter, configuration: configuration, delegate: self)
                self.lock.lock()
                self.continuation = continuation
                self.stream = stream
                self.timeoutTask = Task { [weak self] in
                    try? await Task.sleep(nanoseconds: 10_000_000_000)
                    self?.finish(.failure(ScreenCaptureService.CaptureError.captureFailed("Timed out waiting for a screenshot frame.")))
                }
                self.lock.unlock()
                if Task.isCancelled {
                    self.finish(.failure(CancellationError()))
                    return
                }
                do {
                    try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: queue)
                    Task {
                        do {
                            try await stream.startCapture()
                        } catch {
                            self.finish(.failure(error))
                        }
                    }
                } catch {
                    self.finish(.failure(error))
                }
            }
        } onCancel: {
            finish(.failure(CancellationError()))
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        finish(.failure(error))
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard outputType == .screen, sampleBuffer.isValid else { return }
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let ciImage = CIImage(cvPixelBuffer: pixelBuffer)
        guard let cgImage = context.createCGImage(ciImage, from: ciImage.extent) else { return }
        finish(.success(cgImage))
    }

    private func finish(_ result: Result<CGImage, Error>) {
        lock.lock()
        guard let continuation else {
            lock.unlock()
            return
        }
        self.continuation = nil
        let stream = self.stream
        self.stream = nil
        timeoutTask?.cancel()
        timeoutTask = nil
        lock.unlock()
        Task { try? await stream?.stopCapture() }
        continuation.resume(with: result)
    }
}
