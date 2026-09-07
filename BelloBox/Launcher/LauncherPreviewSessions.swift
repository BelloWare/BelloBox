import AppKit
import Combine
import Foundation

/// The interactive state behind a focused row. One session per command lives
/// for the palette session (it survives focusing other rows and searching) and
/// is discarded with the selection. Sessions are created when a row is
/// focused, never persist anything, never touch the clipboard or the network
/// on their own, and hand their draft to the full tool on Enter.
enum LauncherInteractivePreview {
    /// Developer tools share the workbench model, so Enter opens the very
    /// same instance with every edit and option intact.
    case workbench(UtilityWorkbenchModel)
    case clock(WorldClockViewModel)
    case qr(LauncherQRPreview)
    case textTools(TextToolsPopupViewModel)
    case ai(LauncherAIPreview)
    case capture(LauncherCapturePreview)
    case recording(LauncherRecordingPreview)
    case videoToGIF(LauncherGIFPreview)
    case appStatus(LauncherAppStatusPreview)

    /// The model's change publisher, so the palette re-renders the row.
    var objectWillChange: AnyPublisher<Void, Never> {
        switch self {
        case .workbench(let model): return model.objectWillChange.map { _ in () }.eraseToAnyPublisher()
        case .clock(let model):
            return model.objectWillChange.map { _ in () }
                .merge(with: model.copilot.objectWillChange.map { _ in () }).eraseToAnyPublisher()
        case .qr(let model): return model.objectWillChange.map { _ in () }.eraseToAnyPublisher()
        case .textTools(let model): return model.objectWillChange.map { _ in () }.eraseToAnyPublisher()
        case .ai(let model): return model.objectWillChange.map { _ in () }.eraseToAnyPublisher()
        case .capture(let model): return model.objectWillChange.map { _ in () }.eraseToAnyPublisher()
        case .recording(let model): return model.objectWillChange.map { _ in () }.eraseToAnyPublisher()
        case .videoToGIF(let model): return model.objectWillChange.map { _ in () }.eraseToAnyPublisher()
        case .appStatus(let model): return model.objectWillChange.map { _ in () }.eraseToAnyPublisher()
        }
    }

    /// Stops any work the session started (a preview calculation, a request).
    @MainActor func cancel() {
        switch self {
        case .workbench(let model): model.cancel()
        case .clock(let model): model.cancelAI()
        case .qr, .textTools, .ai, .capture, .recording, .videoToGIF, .appStatus: break
        }
    }

    /// What assistive technology hears for the focused row: the session's
    /// current state, never the original selection's summary.
    @MainActor var accessibilitySummary: String {
        switch self {
        case .workbench(let model):
            let state = model.error.map { "Not recognized: \($0)" } ?? (model.busy ? "Working" : model.result?.status ?? "Ready")
            return "\(model.command.title). \(state). Interactive controls below."
        case .clock(let model): return model.accessibilitySummary
        case .qr(let model):
            let code = model.code
            let state = code.image == nil ? (code.isEmpty ? "no text yet" : code.capacityMessage)
                : model.isEnlarged ? "enlarged, scannable" : model.isDense ? "dense, enlarge to scan" : "scannable"
            return "QR code, \(code.byteCount) bytes, \(state). Editable text, copy and save below."
        case .textTools(let model):
            return "Text Tools, \(model.category.rawValue), \(model.input.count) characters. Interactive controls below."
        case .ai(let model):
            return "Ask AI, \(model.providerSummary). " + (model.trimmedDraft.isEmpty ? "No prompt yet." : "Prompt: \(model.promptSummary).") + " Nothing is sent until you choose an action or Send."
        case .capture(let model):
            return "Screenshot, " + (model.hasScreenRecordingPermission ? "ready" : "Screen Recording permission needed") + ". Capture modes below."
        case .recording(let model):
            return "Screen Recording, " + (model.canRecordVideo ? model.summary : "Screen Recording permission needed") + ". Options and Start below."
        case .videoToGIF(let model): return "Video to GIF, \(model.summary). Choose a movie below."
        case .appStatus(let model):
            return (model.accessibilityTrusted ? "Selection access on" : "Selection access needed") + ", "
                + (model.screenRecordingTrusted ? "Screen Recording on" : "Screen Recording needed") + ", " + model.aiSummary + "."
        }
    }

    var workbench: UtilityWorkbenchModel? { if case .workbench(let model) = self { return model }; return nil }
    var clock: WorldClockViewModel? { if case .clock(let model) = self { return model }; return nil }
    var qr: LauncherQRPreview? { if case .qr(let model) = self { return model }; return nil }
    var textTools: TextToolsPopupViewModel? { if case .textTools(let model) = self { return model }; return nil }
    var ai: LauncherAIPreview? { if case .ai(let model) = self { return model }; return nil }
    var capture: LauncherCapturePreview? { if case .capture(let model) = self { return model }; return nil }
    var recording: LauncherRecordingPreview? { if case .recording(let model) = self { return model }; return nil }
    var videoToGIF: LauncherGIFPreview? { if case .videoToGIF(let model) = self { return model }; return nil }
    var appStatus: LauncherAppStatusPreview? { if case .appStatus(let model) = self { return model }; return nil }
}

/// The QR row: the popup's own view model (text, image, copy, save) plus the
/// row's display state. The compact card cannot show a dense code with two
/// pixels per module on a standard display, so the row can enlarge it; the
/// palette grows and shrinks with that choice.
@MainActor
final class LauncherQRPreview: ObservableObject {
    let code: QRCodePopupViewModel
    @Published var isEnlarged = false { didSet { if isEnlarged != oldValue { reportHeightIfChanged() } } }
    /// Fires once after the row's height changed (enlarged, or the enlarged
    /// code's density changed with the text), so the host can resize.
    var onStateChange: () -> Void = {}
    static let compactCardPoints: CGFloat = 128
    static let cardInset: CGFloat = 6
    static let largeCardRange: ClosedRange<CGFloat> = 192...384
    private var observers: [AnyCancellable] = []
    /// Metrics for the text they were computed from. Set from the text
    /// publisher before the change lands, so height answers are never stale.
    private var metrics: (text: String, modules: Int?)
    private var renders: [CGFloat: NSImage?] = [:]
    private var reportedExtraHeight: CGFloat = 0

    init(text: String) {
        code = QRCodePopupViewModel(text: text)
        metrics = (text, QRCodeGenerator.moduleCount(for: text))
        observers = [
            code.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() },
            code.$text.dropFirst().sink { [weak self] text in self?.textWillChange(to: text) }
        ]
    }

    private func textWillChange(to text: String) {
        guard text != metrics.text else { return }
        // Old rasters are dropped, not kept per size for every payload.
        renders = [:]
        metrics = (text, QRCodeGenerator.moduleCount(for: text))
        reportHeightIfChanged()
    }
    private func reportHeightIfChanged() {
        let height = extraHeight
        guard height != reportedExtraHeight else { return }
        reportedExtraHeight = height
        onStateChange()
    }

    var text: String { code.text }
    var fitsPreviewLimit: Bool { code.text.utf8.count <= LauncherPreview.parsingByteLimit }
    /// Modules per side (with quiet zone), or nil when the text is not encodable.
    var moduleCount: Int? { metrics.modules }
    /// Whether the compact card would give a module less than two pixels on a
    /// standard-resolution display, where scanners start to fail.
    var isDense: Bool {
        guard let moduleCount else { return false }
        return CGFloat(moduleCount) * 2 > Self.compactCardPoints - 2 * Self.cardInset
    }
    /// The enlarged card: two pixels per module plus the inset, in steps of 8 points.
    var largeCardPoints: CGFloat {
        guard let moduleCount else { return Self.largeCardRange.lowerBound }
        let needed = CGFloat(moduleCount) * 2 + 2 * Self.cardInset + 8
        return min(max((needed / 8).rounded(.up) * 8, Self.largeCardRange.lowerBound), Self.largeCardRange.upperBound)
    }
    var cardPoints: CGFloat { isEnlarged ? largeCardPoints : Self.compactCardPoints }
    /// How much taller the row is while the code is enlarged.
    var extraHeight: CGFloat { isEnlarged ? largeCardPoints - Self.compactCardPoints : 0 }
    /// The code rendered for the card at two pixels per point, so Retina
    /// displays get exact pixels and standard displays a clean 2:1 reduction.
    /// Only the current text's rasters are kept.
    var cardImage: NSImage? {
        let side = (cardPoints - 2 * Self.cardInset) * 2
        if let render = renders[side] { return render }
        let image = QRCodeGenerator.image(for: metrics.text, pixelSize: side)
        renders[side] = image
        return image
    }
    var cachedRenderCount: Int { renders.count }
    var exportPixelSize: Int { Int(QRCodeGenerator.exportPixelSize(for: metrics.text)) }
}

/// The Ask AI row: a prompt draft and the quick actions. Nothing is sent from
/// the palette. Enter (Open) carries the draft into Ask AI without sending;
/// only Send, Return inside the prompt field, or a quick action starts a
/// request, and the answer streams in the popup.
@MainActor
final class LauncherAIPreview: ObservableObject {
    @Published var draft = ""
    let quickActions = QuickAction.library
    let characterCount: Int
    private let settings: AppSettings
    private var observer: AnyCancellable?

    init(text: String, settings: AppSettings) {
        characterCount = text.count
        self.settings = settings
        observer = settings.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() }
    }

    var isConfigured: Bool { settings.isConfigured }
    var providerSummary: String {
        guard settings.isConfigured else { return "No AI provider yet" }
        let model = settings.currentConfig.model.trimmingCharacters(in: .whitespacesAndNewlines)
        return settings.providerKind.displayName + (model.isEmpty ? "" : " · " + model)
    }
    var hasSelection: Bool { characterCount > 0 }
    var trimmedDraft: String { draft.trimmingCharacters(in: .whitespacesAndNewlines) }
    /// A short form of the prompt for spoken summaries.
    var promptSummary: String {
        let prefix = String(trimmedDraft.prefix(60))
        return prefix.count < trimmedDraft.count ? prefix + "…" : prefix
    }
    /// A custom question needs a provider, not a selection.
    var canSend: Bool { isConfigured && !trimmedDraft.isEmpty }
    /// Quick transformations work on the selected text.
    var canRunQuickActions: Bool { isConfigured && hasSelection }

    /// What Enter (Open) carries: the draft, kept even without a provider or
    /// a selection, and never a request.
    var openHandoff: AIHandoff? {
        guard !trimmedDraft.isEmpty else { return nil }
        return AIHandoff(instruction: trimmedDraft, replacesSelection: true, run: false)
    }
    /// The explicit Send (button or Return inside the prompt field).
    var sendHandoff: AIHandoff? {
        guard canSend else { return nil }
        return AIHandoff(instruction: trimmedDraft, replacesSelection: true, run: true)
    }
    func handoff(for action: QuickAction) -> AIHandoff? {
        guard canRunQuickActions else { return nil }
        return AIHandoff(instruction: action.instruction, replacesSelection: action.replacesSelection, run: true)
    }
}

/// Screenshot and Scrolling Screenshot: the capture modes the native flow
/// offers, plus the Screen Recording permission the capture needs.
@MainActor
final class LauncherCapturePreview: ObservableObject {
    @Published private(set) var hasScreenRecordingPermission: Bool
    let defaultMode: ScreenshotCaptureMode
    let includesCursor: Bool
    let modes: [ScreenshotCaptureMode]

    init(command: LauncherCommand, settings: AppSettings, permission: () -> Bool = { ScreenCapturePermission.isTrusted }) {
        hasScreenRecordingPermission = permission()
        includesCursor = settings.screenshotIncludeCursor
        switch settings.screenshotDefaultMode {
        case .area: defaultMode = command == .scrollCapture ? .scrolling : .area
        case .window: defaultMode = command == .scrollCapture ? .scrolling : .window
        case .screen: defaultMode = command == .scrollCapture ? .scrolling : .screen
        case .scrolling: defaultMode = .scrolling
        }
        modes = command == .scrollCapture ? [.scrolling] : ScreenshotCaptureMode.allCases
    }

    func refreshPermission() {
        let trusted = ScreenCapturePermission.isTrusted
        if trusted != hasScreenRecordingPermission { hasScreenRecordingPermission = trusted }
    }

    /// Explicit: shows the system prompt, then Screen Recording settings.
    func requestPermission() {
        _ = ScreenCapturePermission.requestPrompt()
        refreshPermission()
        if !hasScreenRecordingPermission { ScreenCapturePermission.openSettings() }
    }
}

/// Screen Recording: a session copy of the recording defaults. Edits here are
/// never saved; Start hands them to the capture overlay, which starts from
/// them exactly like the floating toolbar does.
@MainActor
final class LauncherRecordingPreview: ObservableObject {
    @Published var options: RecordingOptions
    @Published private(set) var permissions: RecordingPermissionState
    private let permissionProvider: (RecordingOptions) -> RecordingPermissionState

    init(options: RecordingOptions, permissions: @escaping (RecordingOptions) -> RecordingPermissionState = { RecordingPermissionState.current(options: $0) }) {
        self.options = options
        self.permissionProvider = permissions
        self.permissions = permissions(options)
    }

    var canRecordVideo: Bool { permissions.canRecordVideo }
    func refreshPermissions() {
        let state = permissionProvider(options)
        if state != permissions { permissions = state }
    }
    func requestScreenRecording() {
        _ = ScreenCapturePermission.requestPrompt()
        refreshPermissions()
        if !permissions.canRecordVideo { ScreenCapturePermission.openSettings() }
    }
    var summary: String {
        var parts = [options.outputFormat == .gif ? "GIF" : "Movie", options.quality.label.lowercased() + " quality"]
        parts.append(options.audioSource == .none ? "no audio" : options.audioSource.label.lowercased())
        parts.append(options.countdownSeconds == 0 ? "no countdown" : "\(options.countdownSeconds) s countdown")
        return parts.joined(separator: " · ")
    }
    /// What the chosen format delivers, honestly: a silent movie has no audio track.
    var deliverable: String {
        if options.outputFormat == .gif {
            return "Silent GIF · \(options.gif.framesPerSecond) fps · up to \(options.gif.maxWidth) px · the movie is kept too"
        }
        return options.audioSource == .none ? "Silent movie (.mov)" : "Movie with \(options.audioSource.label.lowercased()) (.mov)"
    }
    /// The same secure-field note the recording flow shows: hiding needs Accessibility.
    var privacyNote: String {
        RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: permissions.accessibility == .granted) == nil
            ? "secure fields are hidden while you type in them"
            : "secure-field hiding needs Accessibility"
    }
}

/// Video to GIF: the converter's frame rate, size and loop options as a draft.
@MainActor
final class LauncherGIFPreview: ObservableObject {
    @Published var options: GIFExportOptions
    init(options: GIFExportOptions) { self.options = options }

    var summary: String {
        "\(options.framesPerSecond) fps · up to \(options.maxWidth) px · " + (options.loops ? "loops" : "plays once")
            + " · ≤ \(Int(GIFExportOptions.maxDuration)) s"
    }
}

/// Settings and Home: live status that both windows show, refreshed while the
/// row is focused, plus the destinations Enter or a button can open.
@MainActor
final class LauncherAppStatusPreview: ObservableObject {
    @Published private(set) var accessibilityTrusted: Bool
    @Published private(set) var screenRecordingTrusted: Bool
    private let settings: AppSettings
    private let accessibilityProvider: () -> Bool
    private let screenRecordingProvider: () -> Bool
    private var observer: AnyCancellable?

    init(settings: AppSettings, accessibility: @escaping () -> Bool = { AccessibilityService.isTrusted },
         screenRecording: @escaping () -> Bool = { ScreenCapturePermission.isTrusted }) {
        self.settings = settings
        accessibilityProvider = accessibility
        screenRecordingProvider = screenRecording
        accessibilityTrusted = accessibility()
        screenRecordingTrusted = screenRecording()
        observer = settings.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() }
    }

    func refresh() {
        let accessibility = accessibilityProvider(), screen = screenRecordingProvider()
        if accessibility != accessibilityTrusted { accessibilityTrusted = accessibility }
        if screen != screenRecordingTrusted { screenRecordingTrusted = screen }
    }

    var aiSummary: String {
        settings.isConfigured ? "AI: " + settings.providerKind.displayName : "AI is optional · no provider yet"
    }
    var shortcutSummary: String {
        settings.globalHotkeyEnabled ? "Palette shortcut " + settings.globalHotkey.displayString : "Palette shortcut off"
    }
    var launchAtLogin: Bool { settings.launchAtLoginEnabled }
    var floatingHint: Bool { settings.floatingButtonEnabled }
    var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }
    func openAccessibilitySettings() {
        AccessibilityService.requestPermissionPrompt()
        AccessibilityService.openAccessibilitySettings()
    }
    func openScreenRecordingSettings() {
        _ = ScreenCapturePermission.requestPrompt()
        ScreenCapturePermission.openSettings()
    }
}
