// Synthetic pixels only. VideoToolbox supplies elementary samples, not a movie.
import Foundation
import CoreMedia
import CoreVideo
import VideoToolbox
import CryptoKit

func require(_ ok: Bool, _ message: String) throws {
    if !ok { throw NSError(domain: "GenerateSamples", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: message]) }
}
func check(_ status: OSStatus, _ operation: String) throws {
    try require(status == noErr, "\(operation): \(status)")
}
func sha(_ data: Data) -> String {
    SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}
func timing(_ t: CMTime) -> [String: Any] {
    ["value": t.value, "timescale": t.timescale, "flags": t.flags.rawValue,
     "epoch": t.epoch, "seconds": t.isNumeric ? t.seconds as Any : NSNull()]
}
final class FrameResult: @unchecked Sendable {
    private let lock = NSLock()
    private var sample: CMSampleBuffer?
    private var status: OSStatus = -1
    private var flags = VTEncodeInfoFlags()
    let done = DispatchSemaphore(value: 0)
    func set(_ status: OSStatus, _ flags: VTEncodeInfoFlags, _ sample: CMSampleBuffer?) {
        lock.lock(); self.status = status; self.flags = flags; self.sample = sample
        lock.unlock(); done.signal()
    }
    func get() -> (OSStatus, VTEncodeInfoFlags, CMSampleBuffer?) {
        lock.lock(); defer { lock.unlock() }; return (status, flags, sample)
    }
}

var rows = [[String: Any]]()
do {
    try require(CommandLine.arguments.count == 2, "expected a new output directory")
    let directory = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
    try require(!FileManager.default.fileExists(atPath: directory.path), "directory already exists")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
                                           attributes: [.posixPermissions: 0o700])
    var session: VTCompressionSession?
    try check(VTCompressionSessionCreate(allocator: nil, width: 64, height: 48,
        codecType: kCMVideoCodecType_H264, encoderSpecification: nil,
        imageBufferAttributes: nil, compressedDataAllocator: nil, outputCallback: nil,
        refcon: nil, compressionSessionOut: &session), "create encoder")
    let encoder = session!
    defer { VTCompressionSessionInvalidate(encoder) }
    let properties: [(CFString, CFTypeRef)] = [
        (kVTCompressionPropertyKey_AllowFrameReordering, kCFBooleanFalse),
        (kVTCompressionPropertyKey_MaxKeyFrameInterval, NSNumber(value: 1)),
        (kVTCompressionPropertyKey_ProfileLevel, kVTProfileLevel_H264_Baseline_AutoLevel),
        (kVTCompressionPropertyKey_RealTime, kCFBooleanFalse)
    ]
    for (key, value) in properties { try check(VTSessionSetProperty(encoder, key: key, value: value), String(describing: key)) }
    try check(VTCompressionSessionPrepareToEncodeFrames(encoder), "prepare encoder")
    var configuration: Data?
    for index in 0..<3 {
        var pixel: CVPixelBuffer?
        try check(CVPixelBufferCreate(nil, 64, 48, kCVPixelFormatType_32BGRA,
                                     nil, &pixel), "allocate pixels")
        let p = pixel!
        try check(CVPixelBufferLockBaseAddress(p, []), "lock pixels")
        let base = CVPixelBufferGetBaseAddress(p)!.assumingMemoryBound(to: UInt8.self)
        let stride = CVPixelBufferGetBytesPerRow(p)
        var rgba = Data()
        for y in 0..<48 { for x in 0..<64 {
            let r = UInt8((3 * x + 71 * index) % 256)
            let g = UInt8((5 * y + 43 * index) % 256)
            let b = UInt8((2 * x + 3 * y + 97 * index) % 256)
            let offset = y * stride + x * 4
            base[offset] = b; base[offset + 1] = g; base[offset + 2] = r; base[offset + 3] = 255
            rgba.append(contentsOf: [r, g, b, 255])
        } }
        try check(CVPixelBufferUnlockBaseAddress(p, []), "unlock pixels")
        try rgba.write(to: directory.appendingPathComponent("frame-\(index).rgba"), options: .withoutOverwriting)
        let result = FrameResult()
        let pts = CMTime(value: Int64(index * 60), timescale: 600)
        let duration = CMTime(value: 60, timescale: 600)
        var flags = VTEncodeInfoFlags()
        try check(VTCompressionSessionEncodeFrameWithOutputHandler(encoder,
            imageBuffer: p, presentationTimeStamp: pts, duration: duration,
            frameProperties: [kVTEncodeFrameOptionKey_ForceKeyFrame: true] as CFDictionary,
            infoFlagsOut: &flags) { status, outputFlags, sample in
                result.set(status, outputFlags, sample)
            }, "encode frame")
        try check(VTCompressionSessionCompleteFrames(encoder, untilPresentationTimeStamp: pts), "complete frame")
        try require(result.done.wait(timeout: .now() + 20) == .success, "encoder callback timeout")
        let (status, outputFlags, sample) = result.get()
        try check(status, "encoder callback")
        try require(sample != nil && !outputFlags.contains(.frameDropped), "missing/dropped sample")
        let s = sample!
        try require(CMSampleBufferGetNumSamples(s) == 1, "expected one encoded sample")
        let attachments = CMSampleBufferGetSampleAttachmentsArray(s, createIfNecessary: false) as? [[String: Any]]
        let notSync = attachments?.first?[kCMSampleAttachmentKey_NotSync as String] as? Bool ?? false
        try require(!notSync, "expected independent key frame")
        let format = CMSampleBufferGetFormatDescription(s)!
        let extensions = CMFormatDescriptionGetExtension(format,
            extensionKey: kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms) as? [String: Any]
        guard let avcC = extensions?["avcC"] as? Data else {
            throw NSError(domain: "GenerateSamples", code: 2,
                          userInfo: [NSLocalizedDescriptionKey: "missing avcC configuration"])
        }
        if let previous = configuration { try require(previous == avcC, "configuration changed") }
        else { configuration = avcC; try avcC.write(to: directory.appendingPathComponent("avcC.bin"), options: .withoutOverwriting) }
        let block = CMSampleBufferGetDataBuffer(s)!
        let count = CMBlockBufferGetDataLength(block)
        try require(count > 0 && count < 100_000, "encoded sample size bound")
        var bytes = Data(count: count)
        let copied = bytes.withUnsafeMutableBytes {
            CMBlockBufferCopyDataBytes(block, atOffset: 0, dataLength: count, destination: $0.baseAddress!)
        }
        try check(copied, "copy encoded bytes")
        try bytes.write(to: directory.appendingPathComponent("frame-\(index).avc"), options: .withoutOverwriting)
        rows.append(["index": index, "source_rgba_sha256": sha(rgba),
            "encoded_sha256": sha(bytes), "encoded_bytes": bytes.count, "avcC_sha256": sha(avcC),
            "input_pts": timing(pts), "input_duration": timing(duration),
            "output_pts": timing(CMSampleBufferGetPresentationTimeStamp(s)),
            "output_dts": timing(CMSampleBufferGetDecodeTimeStamp(s)),
            "output_duration": timing(CMSampleBufferGetDuration(s)),
            "sync": !notSync, "submission_flags": flags.rawValue,
            "callback_flags": outputFlags.rawValue,
            "attachments": String(describing: attachments)])
    }
    let result: [String: Any] = ["width": 64, "height": 48, "sample_count": 3,
        "codec": "avc1", "reordering": false, "all_intra": true, "samples": rows]
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])); print("")
} catch {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject:
        ["samples_completed": rows, "error": String(describing: error)], options: [.prettyPrinted, .sortedKeys])); print("")
    exit(1)
}
