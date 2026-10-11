import XCTest
@testable import BelloBox

final class LaunchWindowTests: XCTestCase {
    func testOnlyE2EFixturesWaitBeforeTheLaunchWindow() {
        // 0.0.77 waited 0.3 s before showing Home on every launch.
        XCTAssertNil(AppDelegate.launchWindowDelay(environment: [:]))
        XCTAssertNil(AppDelegate.launchWindowDelay(environment: ["HOME": "/Users/x", "XCTestConfigurationFilePath": "/tmp/x"]))
        XCTAssertEqual(AppDelegate.launchWindowDelay(environment: ["BELLOBOX_E2E_QR_TEXT": "hello"]), 0.3)
        XCTAssertEqual(AppDelegate.launchWindowDelay(environment: ["BELLOBOX_E2E_LAUNCHER_TEXT": "hello"]), 0.3)
    }
}
