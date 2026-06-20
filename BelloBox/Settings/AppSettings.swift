import Foundation
import Combine
import AppKit

enum AppearancePreference: String, CaseIterable, Identifiable {
    case system
    case light
    case dark

    var id: String { rawValue }
    var label: String {
        switch self {
        case .system: return "System"
        case .light: return "Light"
        case .dark: return "Dark"
        }
    }
    var symbol: String {
        switch self {
        case .system: return "circle.lefthalf.filled"
        case .light: return "sun.max"
        case .dark: return "moon"
        }
    }
}

enum ScreenshotDefaultMode: String, CaseIterable, Identifiable, Codable {
    case area
    case window
    case screen
    case scrolling

    var id: String { rawValue }

    var label: String {
        switch self {
        case .area: return "Area"
        case .window: return "Window"
        case .screen: return "Screen"
        case .scrolling: return "Scrolling"
        }
    }
}

enum OCRDefaultEngine: String, CaseIterable, Identifiable, Codable {
    case appleVision
    case askEachTime
    case llm
    case hybrid

    var id: String { rawValue }

    var label: String {
        switch self {
        case .appleVision: return "Mac OCR"
        case .askEachTime: return "Ask"
        case .llm: return "LLM OCR"
        case .hybrid: return "Hybrid"
        }
    }
}

/// User-facing configuration, persisted to UserDefaults (API keys go to the
/// Keychain). Per-provider base URL / model are kept independently so switching
/// providers does not lose the other configuration.
/// Touched only on the main thread (settings UI + app lifecycle); not annotated
/// `@MainActor` so the shared instance can be referenced from `App.init`.
final class AppSettings: ObservableObject {
    static let shared = AppSettings()

    private enum Keys {
        static let provider = "provider"
        static let openAIBase = "openAIBaseURL"
        static let anthropicBase = "anthropicBaseURL"
        static let openAIModel = "openAIModel"
        static let openAIAPIKind = "openAIAPIKind"
        static let anthropicModel = "anthropicModel"
        static let systemPrompt = "systemPrompt"
        static let temperatureMode = "temperatureMode"
        static let temperature = "temperature"
        static let floatingButtonEnabled = "floatingButtonEnabled"
        static let globalHotkeyEnabled = "globalHotkeyEnabled"
        static let globalHotkeyKeyCode = "globalHotkeyKeyCode"
        static let globalHotkeyModifiers = "globalHotkeyModifiers"
        static let screenshotHotkeyEnabled = "screenshotHotkeyEnabled"
        static let screenshotHotkeyKeyCode = "screenshotHotkeyKeyCode"
        static let screenshotHotkeyModifiers = "screenshotHotkeyModifiers"
        static let recordingHotkeyEnabled = "recordingHotkeyEnabled"
        static let recordingHotkeyKeyCode = "recordingHotkeyKeyCode"
        static let recordingHotkeyModifiers = "recordingHotkeyModifiers"
        static let hasCompletedSetup = "hasCompletedSetup"
        static let launchAtLoginEnabled = "launchAtLoginEnabled"
        static let appearance = "appearance"
        static let codexPath = "codexPath"
        static let codexModel = "codexModel"
        static let codexReasoningEffort = "codexReasoningEffort"
        static let screenshotIncludeCursor = "screenshotIncludeCursor"
        static let screenshotAutoCopy = "screenshotAutoCopy"
        static let screenshotDefaultMode = "screenshotDefaultMode"
        static let scrollingScreenshotMaxFrames = "scrollingScreenshotMaxFrames"
        static let scrollingScreenshotAutoCompact = "scrollingScreenshotAutoCompact"
        static let ocrDefaultEngine = "ocrDefaultEngine"
        static let ocrRecognitionLevel = "ocrRecognitionLevel"
        static let ocrLanguageHints = "ocrLanguageHints"
        static let ocrUseLanguageCorrection = "ocrUseLanguageCorrection"
        static let ocrShowTextRegions = "ocrShowTextRegions"
        static let llmOCRMaxUploadLongEdge = "llmOCRMaxUploadLongEdge"
        static let llmOCRIncludeLocalOCRHint = "llmOCRIncludeLocalOCRHint"
        static let recordingIncludeCursor = "recordingIncludeCursor"
        static let recordingAudioSource = "recordingAudioSource"
        static let recordingClickOverlayMode = "recordingClickOverlayMode"
        static let recordingKeystrokeMode = "recordingKeystrokeMode"
        static let recordingSecureFieldRedactionMode = "recordingSecureFieldRedactionMode"
        static let recordingQualityPreset = "recordingQualityPreset"
        static let recordingCountdownSeconds = "recordingCountdownSeconds"
        static let recordingLastMicrophoneDeviceID = "recordingLastMicrophoneDeviceID"
    }

    static let defaultSystemPrompt = """
    You are a writing assistant embedded in macOS. The user selects text in any \
    app and asks you to transform it. Unless the instruction asks a question, \
    reply with ONLY the transformed text — no preamble, no explanations, and no \
    surrounding quotation marks.
    """

    private let defaults: UserDefaults

    @Published var providerKind: ProviderKind {
        didSet {
            defaults.set(providerKind.rawValue, forKey: Keys.provider)
            apiKey = KeychainStore.get(account: KeychainStore.account(for: providerKind)) ?? ""
        }
    }

    @Published var openAIBaseURL: String { didSet { defaults.set(openAIBaseURL, forKey: Keys.openAIBase) } }
    @Published var anthropicBaseURL: String { didSet { defaults.set(anthropicBaseURL, forKey: Keys.anthropicBase) } }
    @Published var openAIModel: String { didSet { defaults.set(openAIModel, forKey: Keys.openAIModel) } }
    @Published var openAIAPIKind: OpenAIAPIKind { didSet { defaults.set(openAIAPIKind.rawValue, forKey: Keys.openAIAPIKind) } }
    @Published var anthropicModel: String { didSet { defaults.set(anthropicModel, forKey: Keys.anthropicModel) } }
    @Published var systemPrompt: String { didSet { defaults.set(systemPrompt, forKey: Keys.systemPrompt) } }
    @Published var temperatureMode: TemperatureMode { didSet { defaults.set(temperatureMode.rawValue, forKey: Keys.temperatureMode) } }
    @Published var temperature: Double { didSet { defaults.set(Self.normalizedTemperature(temperature), forKey: Keys.temperature) } }
    @Published var floatingButtonEnabled: Bool { didSet { defaults.set(floatingButtonEnabled, forKey: Keys.floatingButtonEnabled) } }
    @Published var globalHotkeyEnabled: Bool { didSet { defaults.set(globalHotkeyEnabled, forKey: Keys.globalHotkeyEnabled) } }
    @Published var globalHotkeyKeyCode: Int { didSet { defaults.set(globalHotkeyKeyCode, forKey: Keys.globalHotkeyKeyCode) } }
    @Published var globalHotkeyModifiersRawValue: Int { didSet { defaults.set(globalHotkeyModifiersRawValue, forKey: Keys.globalHotkeyModifiers) } }
    @Published var screenshotHotkeyEnabled: Bool { didSet { defaults.set(screenshotHotkeyEnabled, forKey: Keys.screenshotHotkeyEnabled) } }
    @Published var screenshotHotkeyKeyCode: Int { didSet { defaults.set(screenshotHotkeyKeyCode, forKey: Keys.screenshotHotkeyKeyCode) } }
    @Published var screenshotHotkeyModifiersRawValue: Int { didSet { defaults.set(screenshotHotkeyModifiersRawValue, forKey: Keys.screenshotHotkeyModifiers) } }
    @Published var recordingHotkeyEnabled: Bool { didSet { defaults.set(recordingHotkeyEnabled, forKey: Keys.recordingHotkeyEnabled) } }
    @Published var recordingHotkeyKeyCode: Int { didSet { defaults.set(recordingHotkeyKeyCode, forKey: Keys.recordingHotkeyKeyCode) } }
    @Published var recordingHotkeyModifiersRawValue: Int { didSet { defaults.set(recordingHotkeyModifiersRawValue, forKey: Keys.recordingHotkeyModifiers) } }
    @Published var launchAtLoginEnabled: Bool { didSet { defaults.set(launchAtLoginEnabled, forKey: Keys.launchAtLoginEnabled) } }
    @Published var appearance: AppearancePreference { didSet { defaults.set(appearance.rawValue, forKey: Keys.appearance) } }
    @Published var codexPath: String { didSet { defaults.set(codexPath, forKey: Keys.codexPath) } }
    @Published var codexModel: String { didSet { defaults.set(codexModel, forKey: Keys.codexModel) } }
    @Published var codexReasoningEffort: String { didSet { defaults.set(codexReasoningEffort, forKey: Keys.codexReasoningEffort) } }
    @Published var screenshotIncludeCursor: Bool { didSet { defaults.set(screenshotIncludeCursor, forKey: Keys.screenshotIncludeCursor) } }
    @Published var screenshotAutoCopy: Bool { didSet { defaults.set(screenshotAutoCopy, forKey: Keys.screenshotAutoCopy) } }
    @Published var screenshotDefaultMode: ScreenshotDefaultMode { didSet { defaults.set(screenshotDefaultMode.rawValue, forKey: Keys.screenshotDefaultMode) } }
    @Published var scrollingScreenshotMaxFrames: Int { didSet { defaults.set(scrollingScreenshotMaxFrames, forKey: Keys.scrollingScreenshotMaxFrames) } }
    @Published var scrollingScreenshotAutoCompact: Bool { didSet { defaults.set(scrollingScreenshotAutoCompact, forKey: Keys.scrollingScreenshotAutoCompact) } }
    @Published var ocrDefaultEngine: OCRDefaultEngine { didSet { defaults.set(ocrDefaultEngine.rawValue, forKey: Keys.ocrDefaultEngine) } }
    @Published var ocrRecognitionLevel: OCRRecognitionLevel { didSet { defaults.set(ocrRecognitionLevel.rawValue, forKey: Keys.ocrRecognitionLevel) } }
    @Published var ocrLanguageHints: [String] { didSet { defaults.set(ocrLanguageHints, forKey: Keys.ocrLanguageHints) } }
    @Published var ocrUseLanguageCorrection: Bool { didSet { defaults.set(ocrUseLanguageCorrection, forKey: Keys.ocrUseLanguageCorrection) } }
    @Published var ocrShowTextRegions: Bool { didSet { defaults.set(ocrShowTextRegions, forKey: Keys.ocrShowTextRegions) } }
    @Published var llmOCRMaxUploadLongEdge: Int { didSet { defaults.set(llmOCRMaxUploadLongEdge, forKey: Keys.llmOCRMaxUploadLongEdge) } }
    @Published var llmOCRIncludeLocalOCRHint: Bool { didSet { defaults.set(llmOCRIncludeLocalOCRHint, forKey: Keys.llmOCRIncludeLocalOCRHint) } }
    @Published var recordingIncludeCursor: Bool { didSet { defaults.set(recordingIncludeCursor, forKey: Keys.recordingIncludeCursor) } }
    @Published var recordingAudioSourceRawValue: String { didSet { defaults.set(recordingAudioSourceRawValue, forKey: Keys.recordingAudioSource) } }
    @Published var recordingClickOverlayModeRawValue: String { didSet { defaults.set(recordingClickOverlayModeRawValue, forKey: Keys.recordingClickOverlayMode) } }
    @Published var recordingKeystrokeModeRawValue: String { didSet { defaults.set(recordingKeystrokeModeRawValue, forKey: Keys.recordingKeystrokeMode) } }
    @Published var recordingSecureFieldRedactionModeRawValue: String { didSet { defaults.set(recordingSecureFieldRedactionModeRawValue, forKey: Keys.recordingSecureFieldRedactionMode) } }
    @Published var recordingQualityPresetRawValue: String { didSet { defaults.set(recordingQualityPresetRawValue, forKey: Keys.recordingQualityPreset) } }
    @Published var recordingCountdownSeconds: Int { didSet { defaults.set(Self.normalizedCountdown(recordingCountdownSeconds), forKey: Keys.recordingCountdownSeconds) } }
    @Published var recordingLastMicrophoneDeviceID: String? { didSet { defaults.set(recordingLastMicrophoneDeviceID, forKey: Keys.recordingLastMicrophoneDeviceID) } }

    /// API key for the currently-selected provider. Persisted to the Keychain.
    @Published var apiKey: String {
        didSet { KeychainStore.set(apiKey, account: KeychainStore.account(for: providerKind)) }
    }

    var hasCompletedSetup: Bool {
        get { defaults.bool(forKey: Keys.hasCompletedSetup) }
        set { defaults.set(newValue, forKey: Keys.hasCompletedSetup) }
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let kind = ProviderKind(rawValue: defaults.string(forKey: Keys.provider) ?? "") ?? .openAI
        providerKind = kind
        openAIBaseURL = defaults.string(forKey: Keys.openAIBase) ?? ProviderKind.openAI.defaultBaseURL
        anthropicBaseURL = defaults.string(forKey: Keys.anthropicBase) ?? ProviderKind.anthropic.defaultBaseURL
        openAIModel = defaults.string(forKey: Keys.openAIModel) ?? ProviderKind.openAI.defaultModel
        openAIAPIKind = OpenAIAPIKind(rawValue: defaults.string(forKey: Keys.openAIAPIKind) ?? "") ?? .chatCompletions
        anthropicModel = defaults.string(forKey: Keys.anthropicModel) ?? ProviderKind.anthropic.defaultModel
        systemPrompt = defaults.string(forKey: Keys.systemPrompt) ?? Self.defaultSystemPrompt
        temperatureMode = TemperatureMode(rawValue: defaults.string(forKey: Keys.temperatureMode) ?? "") ?? .providerDefault
        let storedTemperature = defaults.object(forKey: Keys.temperature) as? Double
        temperature = Self.normalizedTemperature(storedTemperature ?? 1.0)
        floatingButtonEnabled = (defaults.object(forKey: Keys.floatingButtonEnabled) as? Bool) ?? true
        globalHotkeyEnabled = (defaults.object(forKey: Keys.globalHotkeyEnabled) as? Bool) ?? true
        let storedHotkeyKeyCode = defaults.object(forKey: Keys.globalHotkeyKeyCode) as? Int
        let storedHotkeyModifiers = defaults.object(forKey: Keys.globalHotkeyModifiers) as? Int
        let storedHotkey = Self.normalizedHotkey(keyCode: storedHotkeyKeyCode, modifiersRawValue: storedHotkeyModifiers)
        globalHotkeyKeyCode = Int(storedHotkey.keyCode)
        globalHotkeyModifiersRawValue = Int(storedHotkey.modifiers.rawValue)
        screenshotHotkeyEnabled = (defaults.object(forKey: Keys.screenshotHotkeyEnabled) as? Bool) ?? false
        let storedScreenshotHotkeyKeyCode = defaults.object(forKey: Keys.screenshotHotkeyKeyCode) as? Int
        let storedScreenshotHotkeyModifiers = defaults.object(forKey: Keys.screenshotHotkeyModifiers) as? Int
        let storedScreenshotHotkey = Self.normalizedHotkey(
            keyCode: storedScreenshotHotkeyKeyCode,
            modifiersRawValue: storedScreenshotHotkeyModifiers,
            defaultHotkey: .defaultScreenshot
        )
        screenshotHotkeyKeyCode = Int(storedScreenshotHotkey.keyCode)
        screenshotHotkeyModifiersRawValue = Int(storedScreenshotHotkey.modifiers.rawValue)
        recordingHotkeyEnabled = (defaults.object(forKey: Keys.recordingHotkeyEnabled) as? Bool) ?? false
        let storedRecordingHotkeyKeyCode = defaults.object(forKey: Keys.recordingHotkeyKeyCode) as? Int
        let storedRecordingHotkeyModifiers = defaults.object(forKey: Keys.recordingHotkeyModifiers) as? Int
        let storedRecordingHotkey = Self.normalizedHotkey(
            keyCode: storedRecordingHotkeyKeyCode,
            modifiersRawValue: storedRecordingHotkeyModifiers,
            defaultHotkey: .defaultRecording
        )
        recordingHotkeyKeyCode = Int(storedRecordingHotkey.keyCode)
        recordingHotkeyModifiersRawValue = Int(storedRecordingHotkey.modifiers.rawValue)
        launchAtLoginEnabled = (defaults.object(forKey: Keys.launchAtLoginEnabled) as? Bool) ?? LaunchAtLoginController.isEnabled
        appearance = AppearancePreference(rawValue: defaults.string(forKey: Keys.appearance) ?? "") ?? .system
        codexPath = defaults.string(forKey: Keys.codexPath) ?? ""
        let storedCodexModel = defaults.string(forKey: Keys.codexModel) ?? ""
        codexModel = storedCodexModel.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            ? CodexCLI.defaultModel
            : storedCodexModel
        let storedEffort = defaults.string(forKey: Keys.codexReasoningEffort) ?? ""
        codexReasoningEffort = CodexCLI.reasoningEfforts.contains(storedEffort)
            ? storedEffort
            : CodexCLI.defaultReasoningEffort
        screenshotIncludeCursor = (defaults.object(forKey: Keys.screenshotIncludeCursor) as? Bool) ?? false
        screenshotAutoCopy = (defaults.object(forKey: Keys.screenshotAutoCopy) as? Bool) ?? false
        screenshotDefaultMode = ScreenshotDefaultMode(rawValue: defaults.string(forKey: Keys.screenshotDefaultMode) ?? "") ?? .area
        scrollingScreenshotMaxFrames = defaults.object(forKey: Keys.scrollingScreenshotMaxFrames) as? Int ?? 20
        scrollingScreenshotAutoCompact = (defaults.object(forKey: Keys.scrollingScreenshotAutoCompact) as? Bool) ?? true
        ocrDefaultEngine = OCRDefaultEngine(rawValue: defaults.string(forKey: Keys.ocrDefaultEngine) ?? "") ?? .appleVision
        ocrRecognitionLevel = OCRRecognitionLevel(rawValue: defaults.string(forKey: Keys.ocrRecognitionLevel) ?? "") ?? .accurate
        ocrLanguageHints = defaults.stringArray(forKey: Keys.ocrLanguageHints) ?? []
        ocrUseLanguageCorrection = (defaults.object(forKey: Keys.ocrUseLanguageCorrection) as? Bool) ?? true
        ocrShowTextRegions = (defaults.object(forKey: Keys.ocrShowTextRegions) as? Bool) ?? false
        llmOCRMaxUploadLongEdge = defaults.object(forKey: Keys.llmOCRMaxUploadLongEdge) as? Int ?? 2200
        llmOCRIncludeLocalOCRHint = (defaults.object(forKey: Keys.llmOCRIncludeLocalOCRHint) as? Bool) ?? true
        recordingIncludeCursor = (defaults.object(forKey: Keys.recordingIncludeCursor) as? Bool) ?? RecordingOptions.default.includeCursor
        recordingAudioSourceRawValue = Self.normalizedRawValue(
            defaults.string(forKey: Keys.recordingAudioSource),
            valid: RecordingAudioSource.allCases,
            defaultValue: RecordingOptions.default.audioSource
        )
        recordingClickOverlayModeRawValue = Self.normalizedRawValue(
            defaults.string(forKey: Keys.recordingClickOverlayMode),
            valid: ClickOverlayMode.allCases,
            defaultValue: RecordingOptions.default.clickOverlayMode
        )
        recordingKeystrokeModeRawValue = Self.normalizedRawValue(
            defaults.string(forKey: Keys.recordingKeystrokeMode),
            valid: KeystrokeCaptureMode.allCases,
            defaultValue: RecordingOptions.default.keystrokeMode
        )
        recordingSecureFieldRedactionModeRawValue = Self.normalizedRawValue(
            defaults.string(forKey: Keys.recordingSecureFieldRedactionMode),
            valid: SecureFieldRedactionMode.allCases,
            defaultValue: RecordingOptions.default.secureFieldRedactionMode
        )
        recordingQualityPresetRawValue = Self.normalizedRawValue(
            defaults.string(forKey: Keys.recordingQualityPreset),
            valid: RecordingQualityPreset.allCases,
            defaultValue: RecordingOptions.default.quality
        )
        let storedCountdown = defaults.object(forKey: Keys.recordingCountdownSeconds) as? Int
        recordingCountdownSeconds = Self.normalizedCountdown(storedCountdown ?? RecordingOptions.default.countdownSeconds)
        recordingLastMicrophoneDeviceID = defaults.string(forKey: Keys.recordingLastMicrophoneDeviceID)
        apiKey = KeychainStore.get(account: KeychainStore.account(for: kind)) ?? ""
    }

    /// The resolved configuration for the active provider.
    var currentConfig: AIConfig {
        switch providerKind {
        case .openAI:
            return AIConfig(
                kind: .openAI,
                baseURL: openAIBaseURL,
                model: openAIModel,
                apiKey: apiKey,
                systemPrompt: systemPrompt,
                openAIAPIKind: openAIAPIKind,
                temperature: resolvedTemperature(maximum: 2.0)
            )
        case .anthropic:
            return AIConfig(
                kind: .anthropic,
                baseURL: anthropicBaseURL,
                model: anthropicModel,
                apiKey: apiKey,
                systemPrompt: systemPrompt,
                temperature: resolvedTemperature(maximum: 1.0)
            )
        case .codexCLI:
            return AIConfig(
                kind: .codexCLI,
                baseURL: codexPath,
                model: codexModel,
                apiKey: "",
                systemPrompt: systemPrompt,
                codexReasoningEffort: codexReasoningEffort
            )
        }
    }

    var isConfigured: Bool { currentConfig.isUsable }

    var globalHotkey: GlobalHotkey {
        GlobalHotkey(
            keyCode: UInt16(clamping: globalHotkeyKeyCode),
            modifiers: NSEvent.ModifierFlags(rawValue: UInt(globalHotkeyModifiersRawValue))
        )
    }

    var screenshotHotkey: GlobalHotkey {
        GlobalHotkey(
            keyCode: UInt16(clamping: screenshotHotkeyKeyCode),
            modifiers: NSEvent.ModifierFlags(rawValue: UInt(screenshotHotkeyModifiersRawValue))
        )
    }

    var recordingHotkey: GlobalHotkey {
        GlobalHotkey(
            keyCode: UInt16(clamping: recordingHotkeyKeyCode),
            modifiers: NSEvent.ModifierFlags(rawValue: UInt(recordingHotkeyModifiersRawValue))
        )
    }

    var recordingAudioSource: RecordingAudioSource {
        get { RecordingAudioSource(rawValue: recordingAudioSourceRawValue) ?? RecordingOptions.default.audioSource }
        set { recordingAudioSourceRawValue = newValue.rawValue }
    }

    var recordingClickOverlayMode: ClickOverlayMode {
        get { ClickOverlayMode(rawValue: recordingClickOverlayModeRawValue) ?? RecordingOptions.default.clickOverlayMode }
        set { recordingClickOverlayModeRawValue = newValue.rawValue }
    }

    var recordingKeystrokeMode: KeystrokeCaptureMode {
        get { KeystrokeCaptureMode(rawValue: recordingKeystrokeModeRawValue) ?? RecordingOptions.default.keystrokeMode }
        set { recordingKeystrokeModeRawValue = newValue.rawValue }
    }

    var recordingSecureFieldRedactionMode: SecureFieldRedactionMode {
        get { SecureFieldRedactionMode(rawValue: recordingSecureFieldRedactionModeRawValue) ?? RecordingOptions.default.secureFieldRedactionMode }
        set { recordingSecureFieldRedactionModeRawValue = newValue.rawValue }
    }

    var recordingQualityPreset: RecordingQualityPreset {
        get { RecordingQualityPreset(rawValue: recordingQualityPresetRawValue) ?? RecordingOptions.default.quality }
        set { recordingQualityPresetRawValue = newValue.rawValue }
    }

    var recordingOptions: RecordingOptions {
        RecordingOptions(
            audioSource: recordingAudioSource,
            microphoneDeviceID: recordingLastMicrophoneDeviceID,
            includeCursor: recordingIncludeCursor,
            clickOverlayMode: recordingClickOverlayMode,
            keystrokeMode: recordingKeystrokeMode,
            secureFieldRedactionMode: recordingSecureFieldRedactionMode,
            quality: recordingQualityPreset,
            countdownSeconds: Self.normalizedCountdown(recordingCountdownSeconds),
            excludeBelloBoxWindows: true,
            excludesCurrentProcessAudio: true
        )
    }

    func setGlobalHotkey(_ hotkey: GlobalHotkey) {
        globalHotkeyKeyCode = Int(hotkey.keyCode)
        globalHotkeyModifiersRawValue = Int(hotkey.modifiers.rawValue)
    }

    func resetGlobalHotkey() {
        setGlobalHotkey(.default)
    }

    func setScreenshotHotkey(_ hotkey: GlobalHotkey) {
        screenshotHotkeyKeyCode = Int(hotkey.keyCode)
        screenshotHotkeyModifiersRawValue = Int(hotkey.modifiers.rawValue)
    }

    func resetScreenshotHotkey() {
        setScreenshotHotkey(.defaultScreenshot)
    }

    func setRecordingHotkey(_ hotkey: GlobalHotkey) {
        recordingHotkeyKeyCode = Int(hotkey.keyCode)
        recordingHotkeyModifiersRawValue = Int(hotkey.modifiers.rawValue)
    }

    func resetRecordingHotkey() {
        setRecordingHotkey(.defaultRecording)
    }

    func resetRecordingOptions() {
        let defaults = RecordingOptions.default
        recordingIncludeCursor = defaults.includeCursor
        recordingAudioSource = defaults.audioSource
        recordingClickOverlayMode = defaults.clickOverlayMode
        recordingKeystrokeMode = defaults.keystrokeMode
        recordingSecureFieldRedactionMode = defaults.secureFieldRedactionMode
        recordingQualityPreset = defaults.quality
        recordingCountdownSeconds = defaults.countdownSeconds
        recordingLastMicrophoneDeviceID = defaults.microphoneDeviceID
    }

    func resetSystemPrompt() { systemPrompt = Self.defaultSystemPrompt }

    func setTemperature(_ value: Double) {
        temperature = Self.normalizedTemperature(value)
    }

    func setOCRLanguageHintsText(_ text: String) {
        ocrLanguageHints = text
            .split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
    }

    var ocrLanguageHintsText: String {
        ocrLanguageHints.joined(separator: ", ")
    }

    /// Detects the codex binary off the main thread and stores it if found.
    func detectCodexPath() async {
        let path = await Task.detached { CodexCLI.detectPath() }.value
        if !path.isEmpty { codexPath = path }
    }

    private static func normalizedHotkey(
        keyCode: Int?,
        modifiersRawValue: Int?,
        defaultHotkey: GlobalHotkey = .default
    ) -> GlobalHotkey {
        guard
            let keyCode,
            (0...Int(UInt16.max)).contains(keyCode),
            let modifiersRawValue,
            modifiersRawValue >= 0
        else { return defaultHotkey }

        let hotkey = GlobalHotkey(
            keyCode: UInt16(keyCode),
            modifiers: NSEvent.ModifierFlags(rawValue: UInt(modifiersRawValue))
        )
        return hotkey.isValid ? hotkey : defaultHotkey
    }

    private func resolvedTemperature(maximum: Double) -> Double? {
        guard temperatureMode == .custom else { return nil }
        return min(Swift.max(Self.normalizedTemperature(temperature), 0), maximum)
    }

    private static func normalizedTemperature(_ value: Double) -> Double {
        guard value.isFinite else { return 1.0 }
        return min(max((value * 100).rounded() / 100, 0), 2)
    }

    private static func normalizedCountdown(_ value: Int) -> Int {
        min(max(value, 0), 10)
    }

    private static func normalizedRawValue<T: RawRepresentable & CaseIterable>(
        _ rawValue: String?,
        valid: T.AllCases,
        defaultValue: T
    ) -> String where T.RawValue == String {
        guard let rawValue, valid.contains(where: { $0.rawValue == rawValue }) else {
            return defaultValue.rawValue
        }
        return rawValue
    }
}
