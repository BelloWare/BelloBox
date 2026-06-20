import Foundation
import Vision

final class MacVisionOCRService: OCRService {
    func recognize(document: ScreenshotDocument, options: OCROptions) async throws -> OCRResult {
        let prepared = try OCRImagePreprocessor.prepare(document: document, options: options, forExternalUpload: false)
        let tiles = OCRTileSegmenter.tiles(from: prepared.image)

        var allRegions: [OCRTextRegion] = []
        var warnings = prepared.warnings
        if tiles.count > 1 {
            warnings.append("Tall screenshot was processed in \(tiles.count) OCR tiles.")
        }

        for tile in tiles {
            let observations = try await performVisionOCR(on: tile.image, options: options)
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

        return OCRResult(
            id: UUID(),
            engine: .appleVision(revision: VNRecognizeTextRequestRevision3, recognitionLevel: options.recognitionLevel),
            target: options.target,
            plainText: text,
            markdownText: nil,
            regions: allRegions,
            languageHints: options.languageHints,
            imageDigest: prepared.digest,
            warnings: warnings,
            createdAt: Date()
        )
    }

    private func performVisionOCR(on image: CGImage, options: OCROptions) async throws -> [VNRecognizedTextObservation] {
        try await Task.detached(priority: .userInitiated) {
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
            do {
                try handler.perform([request])
            } catch {
                throw OCRError.failed(error.localizedDescription)
            }
            return request.results ?? []
        }.value
    }
}

