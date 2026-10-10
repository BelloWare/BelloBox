// Oracle: Swift Box 0.0.77 (e43b1c4) local OCR on an imported, unannotated image.
// MacVisionOCRService.recognize without OCRImagePreprocessor, which renders such
// an image unchanged for local OCR. Vision request setup copied verbatim from
// MacVisionOCRService.performVisionOCR with OCROptions.default.
import Foundation
import ImageIO
import Vision

func performVisionOCR(on image: CGImage, options: OCROptions) throws -> [VNRecognizedTextObservation] {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = options.recognitionLevel == .fast ? .fast : .accurate
    request.usesLanguageCorrection = options.usesLanguageCorrection
    if !options.languageHints.isEmpty {
        request.recognitionLanguages = options.languageHints
    } else if #available(macOS 13.0, *) {
        request.automaticallyDetectsLanguage = true
    }
    if !options.customWords.isEmpty {
        request.customWords = options.customWords
    }
    let handler = VNImageRequestHandler(cgImage: image, options: [:])
    try handler.perform([request])
    return request.results ?? []
}

func recognize(_ image: CGImage) throws -> String {
    let options = OCROptions.default
    let tiles = OCRTileSegmenter.tiles(from: image)
    var allRegions: [OCRTextRegion] = []
    for tile in tiles {
        let observations = try performVisionOCR(on: tile.image, options: options)
        let regions = OCRBoundingBoxConverter.regions(
            from: observations,
            imageSize: CGSize(width: tile.image.width, height: tile.image.height)
        )
        allRegions.append(contentsOf: OCRTileSegmenter.offset(regions, by: tile))
    }
    allRegions = OCRTileSegmenter.deduplicateOverlapRegions(allRegions)
    let text = OCRResultFormatter.plainText(from: allRegions)
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
        throw OCRError.noTextFound
    }
    return text
}

let url = URL(fileURLWithPath: CommandLine.arguments[1])
guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
    FileHandle.standardError.write(Data("cannot read image\n".utf8)); exit(2)
}
let start = DispatchTime.now().uptimeNanoseconds
do {
    let text = try recognize(image)
    let ms = Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6
    FileHandle.standardError.write(Data(String(format: "swift-ms %.1f\n", ms).utf8))
    print(text, terminator: "")
} catch {
    FileHandle.standardError.write(Data("error: \(error.localizedDescription)\n".utf8)); exit(1)
}
