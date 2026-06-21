import CoreMedia
import XCTest
@testable import BelloBox

final class PrivacyGuardTests: XCTestCase {
    func testSensitiveStateIsRetainedDuringHysteresisWindow() {
        let sensitive = SensitiveInputState.sensitiveKnownFrame(SensitiveFieldInfo(
            reason: .secureTextField,
            frameInScreenPoints: CGRect(x: 10, y: 20, width: 100, height: 24),
            owningAppBundleID: nil,
            confidence: 1
        ))
        let detector = SequencedPasswordDetector(states: [sensitive, .notSensitive, .notSensitive])
        let guardrail = PrivacyGuard(detector: detector, options: .default)

        XCTAssertEqual(guardrail.update(now: CMTime(seconds: 0, preferredTimescale: 600)), sensitive)
        XCTAssertEqual(guardrail.update(now: CMTime(seconds: 0.25, preferredTimescale: 600)), sensitive)
        XCTAssertEqual(guardrail.update(now: CMTime(seconds: 1.0, preferredTimescale: 600)), .notSensitive)
    }

    func testSensitiveMetadataMatchesPhrasesAndStandaloneShortTokens() {
        XCTAssertTrue(PasswordFieldDetector.metadataLooksSensitive("Enter your API key"))
        XCTAssertTrue(PasswordFieldDetector.metadataLooksSensitive("PIN"))
        XCTAssertTrue(PasswordFieldDetector.metadataLooksSensitive("One-time code"))
    }

    func testSensitiveMetadataDoesNotMatchShortTokensInsideNormalWords() {
        XCTAssertFalse(PasswordFieldDetector.metadataLooksSensitive("Shipping address"))
        XCTAssertFalse(PasswordFieldDetector.metadataLooksSensitive("Desktop title"))
        XCTAssertFalse(PasswordFieldDetector.metadataLooksSensitive("Lesson notes"))
    }
}

private final class SequencedPasswordDetector: PasswordFieldDetecting {
    private var states: [SensitiveInputState]

    init(states: [SensitiveInputState]) {
        self.states = states
    }

    func currentSensitiveInputState() -> SensitiveInputState {
        guard !states.isEmpty else { return .notSensitive }
        return states.removeFirst()
    }
}
