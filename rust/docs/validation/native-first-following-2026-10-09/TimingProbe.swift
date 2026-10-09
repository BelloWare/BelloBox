// Generated media only. Preserve writer output and each independent read trace.
import AVFoundation
import CoreVideo
import CryptoKit
import Foundation

func time(_ seconds: Double) -> CMTime {
    CMTime(seconds: seconds, preferredTimescale: 600)
}
func timing(_ t: CMTime) -> [String: Any] {
    ["value": t.value, "timescale": t.timescale, "flags": t.flags.rawValue,
     "epoch": t.epoch, "seconds": t.isNumeric ? t.seconds as Any : NSNull()]
}
func sha(_ data: Data) -> String {
    SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw NSError(domain: "TimingProbe", code: 1,
                                 userInfo: [NSLocalizedDescriptionKey: message]) }
}

func write(_ url: URL, session: Double, times: [Double]) throws {
    let writer = try AVAssetWriter(outputURL: url, fileType: .mov)
    let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
        AVVideoCodecKey: AVVideoCodecType.h264,
        AVVideoWidthKey: 64, AVVideoHeightKey: 48
    ])
    input.expectsMediaDataInRealTime = false
    try require(writer.canAdd(input), "writer input rejected")
    writer.add(input)
    try require(writer.startWriting(), "startWriting: \(String(describing: writer.error))")
    defer { if writer.status == .writing { writer.cancelWriting() } }
    writer.startSession(atSourceTime: time(session))
    let deadline = Date().addingTimeInterval(20)
    for (index, pts) in times.enumerated() {
        while !input.isReadyForMoreMediaData {
            try require(Date() < deadline, "writer readiness deadline")
            Thread.sleep(forTimeInterval: 0.005)
        }
        var pixel: CVPixelBuffer?
        try require(CVPixelBufferCreate(nil, 64, 48, kCVPixelFormatType_32BGRA,
                                        nil, &pixel) == kCVReturnSuccess, "pixel allocation")
        let p = pixel!
        CVPixelBufferLockBaseAddress(p, [])
        let base = CVPixelBufferGetBaseAddress(p)!.assumingMemoryBound(to: UInt8.self)
        let stride = CVPixelBufferGetBytesPerRow(p)
        for y in 0..<48 {
            for x in 0..<64 {
                let offset = y * stride + x * 4
                let color: [UInt8] = index == times.count - 1 ? [255, 0, 255, 255]
                    : y < 8 ? [0, UInt8(index * 70), 255, 255]
                    : x < 32 ? [0, 0, 255, 255] : [255, 0, 0, 255]
                for c in 0..<4 { base[offset + c] = color[c] }
            }
        }
        CVPixelBufferUnlockBaseAddress(p, [])
        var format: CMVideoFormatDescription?
        try require(CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil,
                    imageBuffer: p, formatDescriptionOut: &format) == noErr, "format")
        var info = CMSampleTimingInfo(duration: time(0.1), presentationTimeStamp: time(pts),
                                      decodeTimeStamp: .invalid)
        var sample: CMSampleBuffer?
        try require(CMSampleBufferCreateReadyWithImageBuffer(allocator: nil,
                    imageBuffer: p, formatDescription: format!, sampleTiming: &info,
                    sampleBufferOut: &sample) == noErr, "sample")
        try require(input.append(sample!), "append: \(String(describing: writer.error))")
    }
    input.markAsFinished()
    let done = DispatchSemaphore(value: 0)
    writer.finishWriting { done.signal() }
    try require(done.wait(timeout: .now() + 20) == .success, "writer completion deadline")
    try require(writer.status == .completed, "writer final status: \(String(describing: writer.error))")
}

func read(_ url: URL, decoded: Bool, start: Double = 0) throws -> [String: Any] {
    let asset = AVURLAsset(url: url)
    let tracks = asset.tracks(withMediaType: .video)
    try require(tracks.count == 1, "expected one video track")
    let track = tracks[0]
    let reader = try AVAssetReader(asset: asset)
    let duration = asset.duration.seconds
    reader.timeRange = CMTimeRange(start: time(start), duration: time(duration - start))
    let settings: [String: Any]? = decoded
        ? [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA] : nil
    let output = AVAssetReaderTrackOutput(track: track, outputSettings: settings)
    output.alwaysCopiesSampleData = false
    try require(reader.canAdd(output), "reader output rejected")
    reader.add(output)
    try require(reader.startReading(), "reader start: \(String(describing: reader.error))")
    defer { if reader.status == .reading { reader.cancelReading() } }
    var samples = [[String: Any]]()
    while let sample = output.copyNextSampleBuffer() {
        // Compressed reads also return zero-sample timing/control markers. Keep
        // those original records; the decoded image-frame bound remains eight.
        try require(samples.count < (decoded ? 8 : 32), "diagnostic sample bound")
        var row: [String: Any] = [
            "pts": timing(CMSampleBufferGetPresentationTimeStamp(sample)),
            "dts": timing(CMSampleBufferGetDecodeTimeStamp(sample)),
            "duration": timing(CMSampleBufferGetDuration(sample)),
            "sample_count": CMSampleBufferGetNumSamples(sample),
            "sample_bytes": CMSampleBufferGetTotalSampleSize(sample),
            "has_image": CMSampleBufferGetImageBuffer(sample) != nil,
            "attachments": String(describing: CMCopyDictionaryOfAttachments(
                allocator: nil, target: sample, attachmentMode: kCMAttachmentMode_ShouldPropagate))
        ]
        if let pixel = CMSampleBufferGetImageBuffer(sample) {
            CVPixelBufferLockBaseAddress(pixel, .readOnly)
            let width = CVPixelBufferGetWidth(pixel), height = CVPixelBufferGetHeight(pixel)
            let stride = CVPixelBufferGetBytesPerRow(pixel)
            let base = CVPixelBufferGetBaseAddress(pixel)!.assumingMemoryBound(to: UInt8.self)
            var rgba = Data()
            for y in 0..<height {
                for x in 0..<width {
                    let o = y * stride + x * 4
                    rgba.append(contentsOf: [base[o + 2], base[o + 1], base[o], base[o + 3]])
                }
            }
            CVPixelBufferUnlockBaseAddress(pixel, .readOnly)
            row["width"] = width; row["height"] = height; row["rgba_sha256"] = sha(rgba)
        }
        samples.append(row)
    }
    try require(reader.status == .completed, "reader final: \(String(describing: reader.error))")
    return ["decoded": decoded, "requested_start": start, "duration": timing(asset.duration),
            "track_start": timing(track.timeRange.start), "track_duration": timing(track.timeRange.duration),
            "segments": track.segments.map { ["empty": $0.isEmpty,
                "source_start": timing($0.timeMapping.source.start),
                "source_duration": timing($0.timeMapping.source.duration),
                "target_start": timing($0.timeMapping.target.start),
                "target_duration": timing($0.timeMapping.target.duration)] },
            "reader_status": reader.status.rawValue, "samples": samples]
}

let directory = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
                                       attributes: [.posixPermissions: 0o700])
let cases: [(String, Double, [Double], Bool)] = [
    ("regular", 0, [0, 0.1, 0.2, 0.3], false),
    ("delayed-first-control", 0, [0.1, 0.2, 0.3], false),
    ("session-at-first", 0.1, [0.1, 0.2, 0.3], false),
    ("negative-session", -0.1, [0, 0.1, 0.2], false),
    ("long-leading-gap", 0, [3.1, 3.2, 3.3], false),
    ("explicit-empty-edit", 0, [0, 0.1, 0.2, 0.3], true)
]
var results = [[String: Any]]()
do {
    for (name, session, times, edit) in cases {
        let url = directory.appendingPathComponent(name + ".mov")
        try write(url, session: session, times: times)
        let writerBytes = try Data(contentsOf: url)
        let writerCopy = directory.appendingPathComponent(name + ".writer.mov")
        try writerBytes.write(to: writerCopy, options: .withoutOverwriting)
        if edit {
            // This is generated fixture construction, before any admission/read.
            // The unedited writer output remains a separate immutable receipt.
            let movie = try AVMutableMovie(url: url, options: nil)
            movie.insertEmptyTimeRange(CMTimeRange(start: .zero, duration: time(0.1)))
            try movie.writeHeader(to: url, fileType: .mov, options: .addMovieHeaderToDestination)
        }
        let bytes = try Data(contentsOf: url)
        let compressed = try read(url, decoded: false)
        let decoded = try read(url, decoded: true)
        let again = try Data(contentsOf: url)
        try require(bytes == again, "source changed during read")
        results.append(["case": name, "submitted_session": session, "submitted_pts": times,
                        "submitted_duration": 0.1, "explicit_container_gap": edit,
                        "writer_sha256": sha(writerBytes), "file_sha256": sha(bytes),
                        "file_bytes": bytes.count, "source_unchanged_after_read": bytes == again,
                        "compressed": compressed, "decoded": decoded])
    }
    let data = try JSONSerialization.data(withJSONObject: ["cases": results],
                                         options: [.prettyPrinted, .sortedKeys])
    FileHandle.standardOutput.write(data); print("")
} catch {
    let data = try JSONSerialization.data(withJSONObject: ["cases_completed": results,
                   "error": String(describing: error)], options: [.prettyPrinted, .sortedKeys])
    FileHandle.standardOutput.write(data); print("")
    exit(1)
}
