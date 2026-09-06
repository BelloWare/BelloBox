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

    /// The design system follows the orange toolbox icon: the accent, brand,
    /// and fills are warm oranges (red over green over blue) in both themes,
    /// surfaces lean cream rather than blue, and warning stays golden so it
    /// cannot be mistaken for the accent.
    func testBrandColorsFollowTheOrangeIconInBothThemes() throws {
        for mode in [NSAppearance.Name.aqua, .darkAqua] {
            let appearance = try XCTUnwrap(NSAppearance(named: mode))
            for (name, color) in [("accent", BoxTheme.accent), ("brand", BoxTheme.brand),
                                  ("accentFill", BoxTheme.accentFill), ("accentDeep", BoxTheme.accentDeep)] {
                let c = try resolved(color, appearance)
                XCTAssertGreaterThan(c[0], c[1] + 0.2, "\(name) in \(mode) should be orange, not neutral")
                XCTAssertGreaterThan(c[1], c[2], "\(name) in \(mode) should be orange rather than red or pink")
                XCTAssertGreaterThan(hue(c), 12, "\(name) in \(mode) is too red")
                XCTAssertLessThan(hue(c), 36, "\(name) in \(mode) is too yellow")
            }
            let warning = try resolved(BoxTheme.warning, appearance)
            let accent = try resolved(BoxTheme.accent, appearance)
            XCTAssertGreaterThan(hue(warning) - hue(accent), 12, "warning must stay distinct from the accent in \(mode)")
            for (name, surface) in [("background", BoxTheme.background), ("well", BoxTheme.well), ("border", BoxTheme.border)] {
                let c = try resolved(surface, appearance)
                XCTAssertGreaterThanOrEqual(c[0], c[1], "\(name) in \(mode) should be warm")
                XCTAssertGreaterThanOrEqual(c[1], c[2], "\(name) in \(mode) should be warm")
            }
        }
    }

    private func hue(_ c: [Double]) -> Double {
        let maxC = c.max()!, minC = c.min()!
        guard maxC > minC else { return 0 }
        let delta = maxC - minC
        var h: Double
        if maxC == c[0] { h = (c[1] - c[2]) / delta }
        else if maxC == c[1] { h = 2 + (c[2] - c[0]) / delta }
        else { h = 4 + (c[0] - c[1]) / delta }
        h *= 60
        return h < 0 ? h + 360 : h
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
