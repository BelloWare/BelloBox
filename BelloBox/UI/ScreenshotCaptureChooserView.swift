import SwiftUI

@MainActor
final class ScreenshotCaptureChooserViewModel: ObservableObject {
    @Published var hasScreenRecordingPermission = ScreenCapturePermission.isTrusted
    @Published var isCapturing = false
    @Published var errorMessage: String?

    var onCaptureArea: () -> Void = {}
    var onCaptureWindow: () -> Void = {}
    var onCaptureScreen: () -> Void = {}
    var onCaptureScrolling: () -> Void = {}
    var onClose: () -> Void = {}

    func refreshPermission() {
        hasScreenRecordingPermission = ScreenCapturePermission.isTrusted
    }

    func requestPermission() {
        _ = ScreenCapturePermission.requestPrompt()
        refreshPermission()
        if !hasScreenRecordingPermission {
            errorMessage = "Screen Recording permission is required. You may need to enable it in System Settings and try again."
        }
    }

    func openSettings() {
        ScreenCapturePermission.openSettings()
    }
}

struct ScreenshotCaptureChooserView: View {
    static let preferredSize = CGSize(width: 420, height: 330)

    @ObservedObject var viewModel: ScreenshotCaptureChooserViewModel
    var initialMode: ScreenshotCaptureMode?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            PopupHeader(
                icon: "camera.viewfinder",
                title: "Screenshot",
                subtitle: "Capture and annotate",
                onClose: viewModel.onClose
            )

            if !viewModel.hasScreenRecordingPermission {
                permissionNotice
            }

            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                modeButton(.area, action: viewModel.onCaptureArea)
                modeButton(.window, action: viewModel.onCaptureWindow)
                modeButton(.screen, action: viewModel.onCaptureScreen)
                modeButton(.scrolling, action: viewModel.onCaptureScrolling)
            }
            .disabled(!viewModel.hasScreenRecordingPermission || viewModel.isCapturing)

            if let error = viewModel.errorMessage {
                Label(error, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.orange)
                    .lineLimit(2)
            }

            Spacer(minLength: 0)

            Text("Screenshots stay on this Mac. OCR is available from the screenshot editor and LLM OCR asks before upload.")
                .font(.caption2)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(18)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
        .popupCard()
        .onAppear {
            viewModel.refreshPermission()
            if let initialMode, viewModel.hasScreenRecordingPermission {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                    trigger(initialMode)
                }
            }
        }
    }

    private var permissionNotice: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Screen Recording permission is required for screenshots.", systemImage: "lock.shield")
                .font(.caption)
                .foregroundStyle(.secondary)
            HStack {
                Button("Grant Screen Recording…") { viewModel.requestPermission() }
                    .buttonStyle(PrimaryButtonStyle())
                Button("Open System Settings") { viewModel.openSettings() }
                    .buttonStyle(SecondaryButtonStyle())
            }
        }
        .toolPanel()
    }

    private func modeButton(_ mode: ScreenshotCaptureMode, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: mode.symbol)
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(.white)
                    .frame(width: 34, height: 34)
                    .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(BoxTheme.accentGradient))
                Text(mode.label)
                    .font(.callout.weight(.semibold))
                Spacer()
            }
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.primary.opacity(0.06)))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(.primary.opacity(0.08), lineWidth: 1))
        }
        .buttonStyle(.plain)
        .help(help(for: mode))
    }

    private func help(for mode: ScreenshotCaptureMode) -> String {
        switch mode {
        case .area: return "Drag a region to capture."
        case .window: return "Choose a window to capture."
        case .screen: return "Capture the display under the pointer."
        case .scrolling: return "Capture multiple scrolled frames and stitch them."
        }
    }

    private func trigger(_ mode: ScreenshotCaptureMode) {
        switch mode {
        case .area: viewModel.onCaptureArea()
        case .window: viewModel.onCaptureWindow()
        case .screen: viewModel.onCaptureScreen()
        case .scrolling: viewModel.onCaptureScrolling()
        }
    }
}
