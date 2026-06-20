protocol OCRService {
    func recognize(document: ScreenshotDocument, options: OCROptions) async throws -> OCRResult
}

