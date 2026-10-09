// Read the two fixed synthetic movies; never rewrite input media or sample timing.
import Foundation
import AVFoundation
import CoreVideo
import CryptoKit

func require(_ ok: Bool, _ message: String) throws {
    if !ok { throw NSError(domain: "NativeRead", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: message]) }
}
func sha(_ data: Data) -> String {
    SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}
func timing(_ t: CMTime) -> [String: Any] {
    ["value": t.value, "timescale": t.timescale, "flags": t.flags.rawValue,
     "epoch": t.epoch, "seconds": t.isNumeric ? t.seconds as Any : NSNull()]
}
func range(_ r: CMTimeRange) -> [String: Any] {
    ["start": timing(r.start), "duration": timing(r.duration)]
}
func fourCC(_ value: FourCharCode) -> String {
    String(bytes: [UInt8((value >> 24) & 255), UInt8((value >> 16) & 255),
                   UInt8((value >> 8) & 255), UInt8(value & 255)], encoding: .ascii) ?? String(value)
}

func read(_ movie: URL, decoded: Bool, destination: URL, caseName: String) async throws -> [String: Any] {
    let asset = AVURLAsset(url: movie)
    let tracks = try await asset.loadTracks(withMediaType: .video)
    let assetDuration = try await asset.load(.duration)
    try require(tracks.count == 1, "expected one video track")
    let track = tracks[0]
    let (trackRange, segments) = try await track.load(.timeRange, .segments)
    let reader = try AVAssetReader(asset: asset)
    // Leave AVAssetReader's default full-range request untouched.
    let requestedRange = reader.timeRange
    let settings: [String: Any]? = decoded
        ? [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA] : nil
    let output = AVAssetReaderTrackOutput(track: track, outputSettings: settings)
    output.alwaysCopiesSampleData = false
    try require(reader.canAdd(output), "reader output rejected")
    reader.add(output)
    let started = reader.startReading()
    defer { if reader.status == .reading { reader.cancelReading() } }
    var rows = [[String: Any]]()
    var images = 0
    if started {
        while let sample = output.copyNextSampleBuffer() {
            try require(rows.count < 32, "native record bound exceeded")
            let index = rows.count
            let format = CMSampleBufferGetFormatDescription(sample)
            var row: [String: Any] = ["ordinal": index,
                "pts": timing(CMSampleBufferGetPresentationTimeStamp(sample)),
                "dts": timing(CMSampleBufferGetDecodeTimeStamp(sample)),
                "duration": timing(CMSampleBufferGetDuration(sample)),
                "sample_count": CMSampleBufferGetNumSamples(sample),
                "sample_bytes": CMSampleBufferGetTotalSampleSize(sample),
                "has_image": CMSampleBufferGetImageBuffer(sample) != nil,
                "media_subtype": format.map { fourCC(CMFormatDescriptionGetMediaSubType($0)) } ?? "none",
                "attachments": String(describing: CMCopyDictionaryOfAttachments(
                    allocator: nil, target: sample, attachmentMode: kCMAttachmentMode_ShouldPropagate)),
                "sample_attachments": String(describing:
                    CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false))]
            if let block = CMSampleBufferGetDataBuffer(sample) {
                let count = CMBlockBufferGetDataLength(block)
                try require(count < 100_000, "native sample byte bound")
                var bytes = Data(count: count)
                if count > 0 {
                    let copied = bytes.withUnsafeMutableBytes {
                        CMBlockBufferCopyDataBytes(block, atOffset: 0, dataLength: count, destination: $0.baseAddress!)
                    }
                    try require(copied == noErr, "copy native sample bytes")
                }
                let name = "\(caseName)-\(decoded ? "decoded" : "passthrough")-\(index).bin"
                try bytes.write(to: destination.appendingPathComponent(name), options: .withoutOverwriting)
                row["block_file"] = name; row["block_sha256"] = sha(bytes)
            }
            if let pixel = CMSampleBufferGetImageBuffer(sample) {
                images += 1
                try require(images <= 8, "decoded image bound exceeded")
                try require(CVPixelBufferGetPixelFormatType(pixel) == kCVPixelFormatType_32BGRA, "unexpected decoded pixel format")
                try require(CVPixelBufferLockBaseAddress(pixel, .readOnly) == kCVReturnSuccess, "lock decoded pixels")
                let width = CVPixelBufferGetWidth(pixel), height = CVPixelBufferGetHeight(pixel)
                let stride = CVPixelBufferGetBytesPerRow(pixel)
                try require(width == 64 && height == 48, "unexpected decoded dimensions")
                let base = CVPixelBufferGetBaseAddress(pixel)!.assumingMemoryBound(to: UInt8.self)
                var rgba = Data()
                for y in 0..<height { for x in 0..<width {
                    let i = y * stride + x * 4
                    rgba.append(contentsOf: [base[i + 2], base[i + 1], base[i], base[i + 3]])
                } }
                try require(CVPixelBufferUnlockBaseAddress(pixel, .readOnly) == kCVReturnSuccess, "unlock decoded pixels")
                let name = "\(caseName)-decoded-\(index).rgba"
                try rgba.write(to: destination.appendingPathComponent(name), options: .withoutOverwriting)
                row["width"] = width; row["height"] = height
                row["rgba_file"] = name; row["rgba_sha256"] = sha(rgba)
            }
            rows.append(row)
        }
    }
    return ["mode": decoded ? "decoded-BGRA-to-visible-RGBA" : "passthrough-nil-settings",
        "started": started, "default_time_range_unmodified": true,
        "requested_time_range": range(requestedRange), "asset_duration": timing(assetDuration),
        "track_range": range(trackRange), "reader_status": reader.status.rawValue,
        "reader_error": reader.error.map { String(describing: $0) } as Any? ?? NSNull(),
        "segments": segments.map { ["empty": $0.isEmpty,
            "source": range($0.timeMapping.source), "target": range($0.timeMapping.target)] },
        "decoded_image_count": images, "records": rows]
}

@main struct NativeRead {
    static func main() async {
        var cases = [[String: Any]]()
        do {
            try require(CommandLine.arguments.count == 3, "expected movie directory and new output directory")
            let source = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
            let output = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
            try require(!FileManager.default.fileExists(atPath: output.path), "output directory exists")
            try FileManager.default.createDirectory(at: output, withIntermediateDirectories: false,
                                                   attributes: [.posixPermissions: 0o700])
            for name in ["zero-origin", "positive-composition"] {
                let url = source.appendingPathComponent(name + ".mov")
                let before = try Data(contentsOf: url)
                let compressed = try await read(url, decoded: false, destination: output, caseName: name)
                let between = try Data(contentsOf: url)
                let decoded = try await read(url, decoded: true, destination: output, caseName: name)
                let after = try Data(contentsOf: url)
                try require(before == between && before == after, "original movie changed during read")
                cases.append(["name": name, "file_bytes": before.count,
                    "sha256_before": sha(before), "sha256_between_reads": sha(between),
                    "sha256_after": sha(after), "source_unchanged": true,
                    "compressed": compressed, "decoded": decoded])
                if (compressed["reader_status"] as? Int) != AVAssetReader.Status.completed.rawValue
                    || (decoded["reader_status"] as? Int) != AVAssetReader.Status.completed.rawValue {
                    throw NSError(domain: "NativeRead", code: 2,
                                  userInfo: [NSLocalizedDescriptionKey: "native reader rejection; bounded experiment stopped"])
                }
            }
            FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject:
                ["cases": cases], options: [.prettyPrinted, .sortedKeys])); print("")
        } catch {
            let report: [String: Any] = ["cases_completed": cases, "error": String(describing: error)]
            if let data = try? JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys]) {
                FileHandle.standardOutput.write(data); print("")
            }
            exit(1)
        }
    }
}
