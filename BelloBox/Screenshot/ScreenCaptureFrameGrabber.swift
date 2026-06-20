import AppKit
import CoreImage
import CoreMedia
import ScreenCaptureKit

final class ScreenCaptureFrameGrabber: NSObject, SCStreamOutput, SCStreamDelegate {
    private let context = CIContext()
    private var continuation: CheckedContinuation<CGImage, Error>?
    private var stream: SCStream?

    func capture(filter: SCContentFilter, configuration: SCStreamConfiguration) async throws -> CGImage {
        try await withCheckedThrowingContinuation { continuation in
            self.continuation = continuation
            let queue = DispatchQueue(label: "BelloBox.ScreenCaptureFrameGrabber")
            let stream = SCStream(filter: filter, configuration: configuration, delegate: self)
            self.stream = stream
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
        guard let continuation else { return }
        self.continuation = nil
        let stream = self.stream
        self.stream = nil
        Task {
            try? await stream?.stopCapture()
            continuation.resume(with: result)
        }
    }
}

