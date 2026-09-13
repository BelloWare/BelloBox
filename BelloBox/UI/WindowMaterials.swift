import AppKit
import SwiftUI

enum WindowSurfaceStyle: String, CaseIterable, Identifiable {
    case glass, solid
    var id: String { rawValue }
    var title: String { self == .glass ? "Glass" : "Solid" }
    var symbol: String { self == .glass ? "square.on.square" : "square.fill" }
    var detail: String { self == .glass ? "See your desktop through windows and cards." : "Opaque backgrounds with the same colors." }
}

private struct WindowSurfaceStyleKey: EnvironmentKey {
    static let defaultValue = WindowSurfaceStyle.glass
}
private struct HasWindowMaterialKey: EnvironmentKey {
    static let defaultValue = false
}
extension EnvironmentValues {
    var windowSurfaceStyle: WindowSurfaceStyle {
        get { self[WindowSurfaceStyleKey.self] }
        set { self[WindowSurfaceStyleKey.self] = newValue }
    }
    var hasWindowMaterial: Bool {
        get { self[HasWindowMaterialKey.self] }
        set { self[HasWindowMaterialKey.self] = newValue }
    }
}

/// Keep the content tree mounted when settings change. Each hosting window gets
/// its own preference scope; nested popup cards share their ancestor's effect.
private struct WindowSurfacePreferences: ViewModifier {
    @ObservedObject var settings: AppSettings
    func body(content: Content) -> some View {
        content.environment(\.windowSurfaceStyle, settings.windowSurfaceStyle)
    }
}

enum WindowMaterialRole { case workspace, popup }

struct WindowMaterialPolicy {
    let style: WindowSurfaceStyle
    let reduceTransparency: Bool
    let increasedContrast: Bool
    var usesGlass: Bool { style == .glass && !reduceTransparency && !increasedContrast }
}

/// One system-managed backdrop per window region. Translucent cards and fields
/// share this blur. Media and QR pixels keep their own opaque backing.
struct WorkspaceBackground: View {
    var role: WindowMaterialRole = .workspace
    /// An explicit policy also lets isolated hosts validate accessibility states
    /// without changing the person's system preferences.
    var policyOverride: WindowMaterialPolicy? = nil
    @Environment(\.windowSurfaceStyle) private var style
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.colorSchemeContrast) private var contrast
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let policy = policyOverride ?? WindowMaterialPolicy(style: style, reduceTransparency: reduceTransparency, increasedContrast: contrast == .increased)
        ZStack {
            if policy.usesGlass {
                NativeWindowMaterial(role: role)
                // Keep the native material visible. A heavy tint here stacks
                // with every card and turns a glass window into an opaque one.
                BoxTheme.background.opacity(scheme == .dark ? 0.18 : 0.035)
            } else {
                BoxTheme.background
            }
            LinearGradient(colors: [BoxTheme.brand.opacity(policy.usesGlass ? 0.012 : 0.025), .clear], startPoint: .topLeading, endPoint: .bottomTrailing)
        }.allowsHitTesting(false).accessibilityHidden(true)
    }
}

struct NativeWindowMaterial: NSViewRepresentable {
    var role: WindowMaterialRole
    func makeNSView(context: Context) -> WindowMaterialView { WindowMaterialView(role: role) }
    func updateNSView(_ view: WindowMaterialView, context: Context) { view.configure(role: role) }
}

final class WindowMaterialView: NSVisualEffectView {
    init(role: WindowMaterialRole) {
        super.init(frame: .zero)
        configure(role: role)
        setAccessibilityElement(false)
    }
    required init?(coder: NSCoder) { nil }
    func configure(role: WindowMaterialRole) {
        let desired: NSVisualEffectView.Material = .hudWindow
        if material != desired { material = desired }
        if blendingMode != .behindWindow { blendingMode = .behindWindow }
        let activity: NSVisualEffectView.State = role == .popup ? .active : .followsWindowActiveState
        if state != activity { state = activity }
    }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var acceptsFirstResponder: Bool { false }
}

private struct WorkspaceSurface: ViewModifier {
    let role: WindowMaterialRole
    let newWindow: Bool
    @Environment(\.hasWindowMaterial) private var hasMaterial
    func body(content: Content) -> some View {
        content.environment(\.hasWindowMaterial, true)
            .foregroundStyle(BoxTheme.primaryText)
            .background {
                // Extend only the backdrop into a native title bar. Content
                // keeps the safe-area inset and standard window controls.
                if newWindow || !hasMaterial { WorkspaceBackground(role: role).ignoresSafeArea() }
            }
    }
}

/// A lightweight overlay on the window's one backdrop, never another effect.
struct ChromeSurface: View {
    @Environment(\.windowSurfaceStyle) private var style
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.colorSchemeContrast) private var contrast
    var body: some View {
        let glass = WindowMaterialPolicy(style: style, reduceTransparency: reduceTransparency, increasedContrast: contrast == .increased).usesGlass
        BoxTheme.surface.opacity(glass ? 0.08 : 1)
            .allowsHitTesting(false).accessibilityHidden(true)
    }
}

enum ToolSurfaceRole {
    case card, control, input, output

    var color: Color { self == .card || self == .control ? BoxTheme.surface : BoxTheme.well }
    var glassOpacity: Double {
        switch self {
        case .card: return 0.14
        case .control: return 0.20
        case .input: return 0.30
        case .output: return 0.28
        }
    }
}

/// Colored glass over the window's shared backdrop, without a second blur.
/// Foreground opacity stays at 100%; only the backing is translucent.
struct ToolSurface: View {
    var role: ToolSurfaceRole = .card
    @Environment(\.windowSurfaceStyle) private var style
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.colorSchemeContrast) private var contrast
    var body: some View {
        let glass = WindowMaterialPolicy(style: style, reduceTransparency: reduceTransparency, increasedContrast: contrast == .increased).usesGlass
        role.color.opacity(glass ? role.glassOpacity : 1)
            .allowsHitTesting(false).accessibilityHidden(true)
    }
}

extension View {
    func windowSurfacePreferences(_ settings: AppSettings) -> some View { modifier(WindowSurfacePreferences(settings: settings)) }
    func workspaceBackground(role: WindowMaterialRole = .workspace, newWindow: Bool = false) -> some View {
        modifier(WorkspaceSurface(role: role, newWindow: newWindow))
    }
    func toolSurface(_ role: ToolSurfaceRole = .card, cornerRadius: CGFloat = 8) -> some View {
        background { ToolSurface(role: role).clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)) }
    }
}

struct WindowSurfaceChoices: View {
    @Binding var selection: WindowSurfaceStyle
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.colorSchemeContrast) private var contrast
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            ToolSectionHeading(title: "Window background")
            HStack(spacing: 10) {
                ForEach(WindowSurfaceStyle.allCases) { style in
                    Button { selection = style } label: {
                        HStack(spacing: 10) {
                            Image(systemName: style.symbol).font(.system(size: 18)).foregroundStyle(BoxTheme.accent)
                            VStack(alignment: .leading, spacing: 4) {
                                Text(style.title).font(.system(size: 12, weight: .semibold))
                                Text(style.detail).font(.system(size: 11)).foregroundStyle(BoxTheme.secondaryText).fixedSize(horizontal: false, vertical: true)
                            }
                            Spacer(minLength: 4)
                            Image(systemName: selection == style ? "checkmark.circle.fill" : "circle")
                                .foregroundStyle(selection == style ? BoxTheme.accent : BoxTheme.secondaryText)
                        }.padding(12).frame(maxWidth: .infinity, minHeight: 68, alignment: .leading)
                            .toolSurface(.control, cornerRadius: 10)
                            .background(selection == style ? BoxTheme.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 10))
                            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(selection == style ? BoxTheme.accent : BoxTheme.separator))
                            .contentShape(RoundedRectangle(cornerRadius: 10))
                    }.buttonStyle(.plain)
                        .accessibilityLabel("\(style.title) window background")
                        .accessibilityValue(selection == style ? "Selected" : "Not selected")
                        .accessibilityIdentifier("windowBackground_\(style.rawValue)")
                }
            }
            Text(reduceTransparency || contrast == .increased
                 ? "macOS accessibility settings are using solid backgrounds. Your choice is kept."
                 : "Glass flows through windows, cards, and controls. QR codes and image pixels keep their original colors.")
                .font(.system(size: 11)).foregroundStyle(BoxTheme.secondaryText).fixedSize(horizontal: false, vertical: true)
        }
    }
}
