import AppKit
import AVFoundation
import Foundation

struct TimedOverlayEvent: Equatable {
    let id: UUID
    let time: CMTime
    let kind: OverlayEventKind
    let expiresAt: CMTime
}

enum OverlayEventKind: Equatable {
    case click(ClickOverlayEvent)
    case keystroke(KeystrokeOverlayEvent)
    case secureTypingHidden

    var isEmpty: Bool {
        switch self {
        case let .keystroke(event):
            return event.displayLabel.isEmpty
        case .click, .secureTypingHidden:
            return false
        }
    }
}

struct ClickOverlayEvent: Equatable {
    enum Button: String, Equatable {
        case left
        case right
        case middle
        case other
    }

    let button: Button
    let clickCount: Int
    let locationInScreenPoints: CGPoint
}

struct KeystrokeOverlayEvent: Equatable {
    let displayLabel: String
    let isShortcut: Bool
    let isPrintable: Bool
    let modifiers: NSEvent.ModifierFlags
}
