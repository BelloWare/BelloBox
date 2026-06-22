import Foundation

final class LLMOCRService: OCRService {
    static let maxUploadBytes = 20 * 1024 * 1024

    private let configProvider: () -> AIConfig
    private let client: AIImageClient
    private let macOCRService: OCRService

    init(settings: AppSettings, client: AIImageClient = AIImageClient(), macOCRService: OCRService = MacVisionOCRService()) {
        self.configProvider = { settings.currentConfig }
        self.client = client
        self.macOCRService = macOCRService
    }

    init(config: AIConfig, client: AIImageClient = AIImageClient(), macOCRService: OCRService = MacVisionOCRService()) {
        self.configProvider = { config }
        self.client = client
        self.macOCRService = macOCRService
    }

    func recognize(document: ScreenshotDocument, options: OCROptions) async throws -> OCRResult {
        let config = configProvider()
        guard config.kind != .codexCLI else {
            throw OCRError.unsupportedProvider("Codex app-server does not support image OCR yet. Use Mac OCR or an image-capable HTTP provider.")
        }

        var localResult: OCRResult?
        if options.engine == .hybrid || options.includeLocalOCRHintForLLM {
            localResult = document.activeOCRResult
            if localResult == nil, options.engine == .hybrid {
                var localOptions = options
                localOptions.engine = .appleVision
                do {
                    localResult = try await macOCRService.recognize(document: document, options: localOptions)
                } catch is CancellationError {
                    throw CancellationError()
                } catch {
                    localResult = nil
                }
            }
        }

        try Task.checkCancellation()
        let prepared = try OCRImagePreprocessor.prepare(document: document, options: options, forExternalUpload: true)
        guard let data = prepared.encodedData, let mimeType = prepared.mimeType else {
            throw OCRError.imageEncodingFailed
        }
        guard data.count <= Self.maxUploadBytes else {
            throw OCRError.requestTooLarge(maxBytes: Self.maxUploadBytes)
        }
        let payload = AIImagePayload(
            data: data,
            mimeType: mimeType,
            width: prepared.image.width,
            height: prepared.image.height,
            digest: prepared.digest
        )

        let localHint = options.includeLocalOCRHintForLLM ? localResult?.plainText : nil
        let prompt: String
        switch options.outputFormat {
        case .plainText:
            prompt = OCRPromptTemplates.exactTranscription(localOCRHint: localHint)
        case .plainTextAndMarkdown:
            prompt = OCRPromptTemplates.layoutPreservingMarkdown(localOCRHint: localHint)
        case .tableMarkdown:
            prompt = OCRPromptTemplates.tableMarkdown(localOCRHint: localHint)
        }

        try Task.checkCancellation()
        let raw = try await client.completeVision(config: config, image: payload, prompt: prompt, responseFormat: .json)
        let parsed = try AIImageClient.parseLLMOCRResponse(raw)

        let engine: OCREngine
        let regions: [OCRTextRegion]
        if options.engine == .hybrid {
            let localSummary = OCRLocalEngineSummary(revision: nil, recognitionLevel: options.recognitionLevel)
            let llmSummary = OCRLLMEngineSummary(provider: config.kind, model: config.model)
            engine = .hybrid(local: localSummary, llm: llmSummary)
            regions = localResult?.regions ?? []
        } else {
            engine = .llm(provider: config.kind, model: config.model)
            regions = []
        }

        return OCRResult(
            id: UUID(),
            engine: engine,
            target: options.target,
            plainText: parsed.plainText,
            markdownText: parsed.markdownText,
            regions: regions,
            languageHints: options.languageHints,
            imageDigest: prepared.digest,
            warnings: prepared.warnings + parsed.warnings,
            createdAt: Date()
        )
    }
}
