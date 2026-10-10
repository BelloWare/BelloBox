// Layout oracle: Swift Box (e43b1c4) OCR region ordering, overlap
// de-duplication and plain-text formatting on generated region sets.
// No Vision: inputs are synthetic regions in image pixels (top-left origin).
// Output JSON: [{regions, order, dedup, plain}] with order/dedup as indices.
import Foundation
import CoreGraphics

struct SplitMix: RandomNumberGenerator { var state: UInt64
    mutating func next() -> UInt64 { state &+= 0x9E3779B97F4A7C15; var z = state
        z = (z ^ (z >> 30)) &* 0xBF58476D1CE4E5B9; z = (z ^ (z >> 27)) &* 0x94D049BB133111EB; return z ^ (z >> 31) }
    mutating func int(_ n: Int) -> Int { Int(next() % UInt64(n)) }
    mutating func quarter(_ n: Int) -> CGFloat { CGFloat(int(n * 4)) / 4 } }

let words = ["Left", "Right", "total", "Line", "the", "parser", "  padded  ", "", "\u{3000}wide\u{3000}", "OK", "会議", "e\u{301}"]
var rng = SplitMix(state: 20261010)
struct Case: Encodable { var regions: [[String: AnyEncodable]]; var order: [Int]; var dedup: [Int]; var plain: String }
struct AnyEncodable: Encodable { let encode: (Encoder) throws -> Void
    init<T: Encodable>(_ v: T) { encode = v.encode }
    func encode(to e: Encoder) throws { try encode(e) } }

var cases: [Case] = []
for c in 0..<80 {
    var regions: [OCRTextRegion] = []
    let rows = 1 + rng.int(12)
    let columns = 1 + rng.int(3)
    var y: CGFloat = rng.quarter(40)
    for _ in 0..<rows {
        let h = 10 + rng.quarter(20)
        for col in 0..<columns where rng.int(5) > 0 {
            let jitter = rng.quarter(Int(h * 0.6)) - h * 0.3
            let rect = CGRect(x: CGFloat(col) * 300 + rng.quarter(30), y: y + jitter, width: 40 + rng.quarter(200), height: h + rng.quarter(4))
            let text = "\(words[rng.int(words.count)]) \(c)-\(regions.count)"
            regions.append(OCRTextRegion(kind: .line, text: rng.int(9) == 0 ? words[rng.int(words.count)] : text,
                                         boundingBox: rng.int(15) == 0 ? nil : CGRectCodable(rect)))
            // A tile overlap re-reads some regions at nearly the same place.
            if rng.int(6) == 0, let box = regions.last?.boundingBox?.rect {
                let shifted = box.offsetBy(dx: rng.quarter(8) - 4, dy: rng.quarter(8) - 4)
                regions.append(OCRTextRegion(kind: .line, text: rng.int(4) == 0 ? "other" : regions.last!.text, boundingBox: CGRectCodable(shifted)))
            }
        }
        y += h + rng.quarter(Int(h * (rng.int(4) == 0 ? 4 : 1)))
    }
    regions.shuffle(using: &rng)
    let index = Dictionary(uniqueKeysWithValues: regions.enumerated().map { ($1.id, $0) })
    let encoded: [[String: AnyEncodable]] = regions.map { r in
        var d: [String: AnyEncodable] = ["text": AnyEncodable(r.text)]
        if let b = r.boundingBox?.rect { d["rect"] = AnyEncodable([b.minX, b.minY, b.width, b.height]) }
        return d
    }
    cases.append(Case(regions: encoded,
                      order: regions.sortedByReadingOrder().map { index[$0.id]! },
                      dedup: OCRTileSegmenter.deduplicateOverlapRegions(regions).map { index[$0.id]! },
                      plain: OCRResultFormatter.plainText(from: OCRTileSegmenter.deduplicateOverlapRegions(regions))))
}
// Tile plans: Swift cuts bands of at most 3200 px overlapping by 160.
struct Plan: Encodable { var height: Int; var bands: [[Int]] }
var plans: [Plan] = []
for height in [1, 3199, 3200, 3201, 3360, 6240, 6241, 9000, 16384] {
    let space = CGColorSpaceCreateDeviceGray()
    let ctx = CGContext(data: nil, width: 2, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: space, bitmapInfo: 0)!
    let image = ctx.makeImage()!
    plans.append(Plan(height: height, bands: OCRTileSegmenter.tiles(from: image).map { [$0.yOffset, $0.image.height] }))
}
struct Out: Encodable { var cases: [Case]; var plans: [Plan] }
let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
FileHandle.standardOutput.write(try! encoder.encode(Out(cases: cases, plans: plans)))
