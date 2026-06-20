import ApplicationServices
import AppKit
import Foundation

protocol PasswordFieldDetecting {
    func currentSensitiveInputState() -> SensitiveInputState
}

final class PasswordFieldDetector: PasswordFieldDetecting {
    func currentSensitiveInputState() -> SensitiveInputState {
        guard AccessibilityService.isTrusted else {
            return .detectorUnavailable(reason: "Accessibility permission not granted")
        }

        guard let element = focusedElement() else { return .notSensitive }

        if stringAttribute(kAXSubroleAttribute, element: element) == kAXSecureTextFieldSubrole as String {
            return .sensitiveKnownFrame(info(for: element, reason: .secureTextField, confidence: 1.0))
        }

        if looksSensitive(element: element) {
            return .sensitiveKnownFrame(info(for: element, reason: .passwordLikeLabel, confidence: 0.75))
        }

        return .notSensitive
    }

    private func focusedElement() -> AXUIElement? {
        let systemWide = AXUIElementCreateSystemWide()
        var focused: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(systemWide, kAXFocusedUIElementAttribute as CFString, &focused)
        guard result == .success,
              let element = AccessibilityService.axElement(from: focused)
        else { return nil }
        return element
    }

    private func info(for element: AXUIElement, reason: SensitiveInputReason, confidence: Double) -> SensitiveFieldInfo {
        SensitiveFieldInfo(
            reason: reason,
            frameInScreenPoints: frame(element: element),
            owningAppBundleID: nil,
            confidence: confidence
        )
    }

    private func looksSensitive(element: AXUIElement) -> Bool {
        let role = stringAttribute(kAXRoleAttribute, element: element)?.lowercased() ?? ""
        guard role.contains("textfield") || role.contains("text") || role.contains("search") else { return false }

        let metadata = [
            stringAttribute(kAXTitleAttribute, element: element),
            stringAttribute(kAXDescriptionAttribute, element: element),
            stringAttribute(kAXHelpAttribute, element: element),
            stringAttribute("AXPlaceholderValue", element: element)
        ]
        .compactMap { $0?.lowercased() }
        .joined(separator: " ")

        guard !metadata.isEmpty else { return false }
        return Self.sensitiveKeywords.contains { metadata.contains($0) }
    }

    private func stringAttribute(_ attribute: String, element: AXUIElement) -> String? {
        var value: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(element, attribute as CFString, &value)
        guard result == .success else { return nil }
        return value as? String
    }

    private func frame(element: AXUIElement) -> CGRect? {
        var positionValue: CFTypeRef?
        var sizeValue: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, kAXPositionAttribute as CFString, &positionValue) == .success,
              AXUIElementCopyAttributeValue(element, kAXSizeAttribute as CFString, &sizeValue) == .success,
              let positionValue,
              let sizeValue
        else { return nil }

        guard let positionAX = AccessibilityService.axValue(from: positionValue),
              let sizeAX = AccessibilityService.axValue(from: sizeValue)
        else { return nil }
        var point = CGPoint.zero
        var size = CGSize.zero
        guard AXValueGetValue(positionAX, .cgPoint, &point),
              AXValueGetValue(sizeAX, .cgSize, &size)
        else { return nil }
        return AccessibilityService.cocoaRect(fromAXRect: CGRect(origin: point, size: size))
    }

    private static let sensitiveKeywords = [
        "password",
        "passcode",
        "pin",
        "2fa",
        "otp",
        "one-time code",
        "verification code",
        "secret",
        "token",
        "api key",
        "private key",
        "recovery key",
        "seed phrase",
        "mnemonic",
        "social security",
        "ssn",
        "credit card",
        "card number",
        "cvv",
        "security code"
    ]
}
