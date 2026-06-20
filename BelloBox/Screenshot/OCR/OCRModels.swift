import CoreGraphics
import Foundation

enum OCREngine: Equatable, Codable {
    case appleVision(revision: Int?, recognitionLevel: OCRRecognitionLevel)
    case llm(provider: ProviderKind, model: String)
    case hybrid(local: OCRLocalEngineSummary, llm: OCRLLMEngineSummary)
}

enum OCRRecognitionLevel: String, Equatable, Codable, CaseIterable, Identifiable {
    case fast
    case accurate

    var id: String { rawValue }

    var label: String {
        switch self {
        case .fast: return "Fast"
        case .accurate: return "Accurate"
        }
    }
}

struct OCRLocalEngineSummary: Equatable, Codable {
    var revision: Int?
    var recognitionLevel: OCRRecognitionLevel
}

struct OCRLLMEngineSummary: Equatable, Codable {
    var provider: ProviderKind
    var model: String
}

struct OCRResult: Identifiable, Equatable, Codable {
    let id: UUID
    var engine: OCREngine
    var target: OCRTarget
    var plainText: String
    var markdownText: String?
    var regions: [OCRTextRegion]
    var languageHints: [String]
    var imageDigest: String
    var warnings: [String]
    var createdAt: Date
}

enum OCRTarget: Equatable, Codable {
    case fullImage
    case crop(CGRectCodable)
    case visibleAfterRedactions(crop: CGRectCodable?)
}

enum OCRRegionKind: String, Equatable, Codable {
    case block
    case paragraph
    case line
    case word
    case table
    case tableRow
    case tableCell
}

struct OCRTextRegion: Identifiable, Equatable, Codable {
    let id: UUID
    var kind: OCRRegionKind
    var text: String
    var confidence: Float?
    var boundingBox: CGRectCodable?
    var children: [OCRTextRegion]

    init(
        id: UUID = UUID(),
        kind: OCRRegionKind,
        text: String,
        confidence: Float? = nil,
        boundingBox: CGRectCodable? = nil,
        children: [OCRTextRegion] = []
    ) {
        self.id = id
        self.kind = kind
        self.text = text
        self.confidence = confidence
        self.boundingBox = boundingBox
        self.children = children
    }
}

struct OCROptions: Equatable, Codable {
    var engine: OCRRequestedEngine
    var recognitionLevel: OCRRecognitionLevel
    var languageHints: [String]
    var usesLanguageCorrection: Bool
    var customWords: [String]
    var target: OCRTarget
    var outputFormat: OCROutputFormat
    var maxUploadLongEdge: Int
    var includeLocalOCRHintForLLM: Bool

    static let `default` = OCROptions(
        engine: .appleVision,
        recognitionLevel: .accurate,
        languageHints: [],
        usesLanguageCorrection: true,
        customWords: [],
        target: .fullImage,
        outputFormat: .plainTextAndMarkdown,
        maxUploadLongEdge: 2200,
        includeLocalOCRHintForLLM: true
    )
}

enum OCRRequestedEngine: String, Equatable, Codable {
    case appleVision
    case llm
    case hybrid
}

enum OCROutputFormat: String, Equatable, Codable {
    case plainText
    case plainTextAndMarkdown
    case tableMarkdown
}

enum OCRDisplayMode: String, CaseIterable, Identifiable {
    case text
    case markdown

    var id: String { rawValue }

    var label: String {
        switch self {
        case .text: return "Text"
        case .markdown: return "Markdown"
        }
    }
}

enum OCRError: LocalizedError, Equatable {
    case noTextFound
    case unsupportedProvider(String)
    case unsupportedModel(String)
    case imageEncodingFailed
    case uploadRequiresConfirmation
    case requestTooLarge(maxBytes: Int)
    case providerReturnedInvalidJSON
    case providerReturnedEmptyText
    case staleResult
    case cancelled
    case failed(String)

    var errorDescription: String? {
        switch self {
        case .noTextFound:
            return "No text was found in this screenshot."
        case let .unsupportedProvider(message):
            return message
        case let .unsupportedModel(message):
            return message
        case .imageEncodingFailed:
            return "The screenshot could not be encoded for OCR."
        case .uploadRequiresConfirmation:
            return "LLM OCR requires confirmation before uploading screenshot pixels."
        case let .requestTooLarge(maxBytes):
            return "The OCR upload is too large. Maximum size is \(maxBytes) bytes."
        case .providerReturnedInvalidJSON:
            return "The provider returned OCR data in an unexpected format."
        case .providerReturnedEmptyText:
            return "The provider returned no OCR text."
        case .staleResult:
            return "The OCR result is out of date for the current crop or redactions."
        case .cancelled:
            return "OCR was cancelled."
        case let .failed(message):
            return message
        }
    }
}

