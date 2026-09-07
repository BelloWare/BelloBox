import Foundation

/// Extra state that travels with a command when it leaves the palette. Every
/// field is what the focused row's interactive preview was showing when Enter
/// (or one of the preview's own buttons) fired, so the full tool continues
/// from the same draft instead of starting over. Nothing here is persisted.
struct LauncherCommandContext: Equatable {
    /// What the World Clock preview was showing when Enter was pressed: the
    /// instant, the chosen reference, and the ephemeral copilot conversation.
    var worldClock: WorldClockHandoff?
    /// The text edited in the QR preview, when it differs from the selection.
    var qrText: String?
    /// The Text Tools category and option chosen in the preview.
    var textTools: TextToolsHandoff?
    /// An instruction the user explicitly chose or typed for Ask AI.
    var ai: AIHandoff?
    /// The capture mode the user chose for Screenshot or Scrolling Screenshot.
    var capture: CaptureHandoff?
    /// Recording options adjusted in the preview; the capture overlay starts from them.
    var recording: RecordingOptions?
    /// GIF options adjusted in the preview, and whether to open the file chooser at once.
    var videoToGIF: VideoToGIFHandoff?
    /// The Settings page to open.
    var settings: SettingsCategory?
    /// The Home category or action to open.
    var home: HomeHandoff?
}

/// The Text Tools preview's choices, applied to the popup's own view model.
struct TextToolsHandoff: Equatable {
    var category: TextToolsPopupViewModel.Category
    var caseStyle: CaseConverter.Style
    var encodeMethod: TextEncoder.Method
    var decodeFormat: TextDecoder.Format
    var lineOp: LineTool.Operation
}

/// An Ask AI request the user explicitly started from the palette. The popup
/// opens with the instruction filled in and, when `run` is set, sends it at
/// once. Focusing the row never creates one of these.
struct AIHandoff: Equatable {
    var instruction: String
    /// Whether the answer is meant to replace the selection (quick actions
    /// decide; typed instructions default to yes, like the popup).
    var replacesSelection: Bool
    var run: Bool
}

struct CaptureHandoff: Equatable {
    var mode: ScreenshotCaptureMode
}

struct VideoToGIFHandoff: Equatable {
    var options: GIFExportOptions
    var chooseFile: Bool
}

enum HomeHandoff: Equatable {
    case category(HomeCategory)
    case setupGuide
    case checkForUpdates
}
