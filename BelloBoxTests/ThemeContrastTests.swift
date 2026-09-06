import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

@MainActor
final class ThemeContrastTests: XCTestCase {
    func testSemanticTextRemainsReadableOnEverySurfaceInBothThemes() throws {
        let inks: [(String, Color)] = [
            ("accent", BoxTheme.accent), ("success", BoxTheme.success),
            ("warning", BoxTheme.warning), ("danger", BoxTheme.danger),
            ("teal", BoxTheme.teal), ("cyan", BoxTheme.cyan),
            ("purple", BoxTheme.purple), ("pink", BoxTheme.pink)
        ]
        let surfaces = [BoxTheme.background, BoxTheme.surface, BoxTheme.well]
        // Reuse the same color instances across appearance changes to catch
        // colors accidentally resolved once at startup.
        for mode in [NSAppearance.Name.aqua, .darkAqua, .aqua] {
            let appearance = try XCTUnwrap(NSAppearance(named: mode))
            for (name, ink) in inks {
                for surface in surfaces {
                    let foreground = try resolved(ink, appearance)
                    let background = try resolved(surface, appearance)
                    XCTAssertGreaterThanOrEqual(contrast(foreground, background), 4.5, "\(name) in \(mode)")
                    // Status badges and selected rows put the ink on a tinted surface.
                    let tinted = zip(foreground, background).map { $0 * 0.12 + $1 * 0.88 }
                    XCTAssertGreaterThanOrEqual(contrast(foreground, tinted), 4.5, "\(name) badge in \(mode)")
                }
            }
        }
    }

    func testFilledControlsKeepWhiteLabelsReadableInBothThemes() throws {
        for mode in [NSAppearance.Name.aqua, .darkAqua] {
            let appearance = try XCTUnwrap(NSAppearance(named: mode))
            for fill in [BoxTheme.accentFill, BoxTheme.accentDeep] {
                XCTAssertGreaterThanOrEqual(contrast([1, 1, 1], try resolved(fill, appearance)), 4.5)
            }
        }
    }

    func testSurfacesAndAccentActuallyChangeWithAppearance() throws {
        let light = try XCTUnwrap(NSAppearance(named: .aqua))
        let dark = try XCTUnwrap(NSAppearance(named: .darkAqua))
        for surface in [BoxTheme.background, BoxTheme.surface, BoxTheme.well] {
            XCTAssertGreaterThan(luminance(try resolved(surface, light)), 0.8)
            XCTAssertLessThan(luminance(try resolved(surface, dark)), 0.05)
        }
        XCTAssertGreaterThan(luminance(try resolved(BoxTheme.accent, dark)), luminance(try resolved(BoxTheme.accent, light)))
    }

    private func resolved(_ color: Color, _ appearance: NSAppearance) throws -> [Double] {
        var result: NSColor?
        appearance.performAsCurrentDrawingAppearance { result = NSColor(color).usingColorSpace(.sRGB) }
        let c = try XCTUnwrap(result)
        return [Double(c.redComponent), Double(c.greenComponent), Double(c.blueComponent)]
    }
    private func luminance(_ c: [Double]) -> Double {
        let linear = c.map { $0 <= 0.04045 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4) }
        return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
    }
    private func contrast(_ a: [Double], _ b: [Double]) -> Double {
        let x = luminance(a), y = luminance(b)
        return (max(x, y) + 0.05) / (min(x, y) + 0.05)
    }
}
