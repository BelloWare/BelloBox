import SwiftUI

@MainActor
final class WindowCapturePickerViewModel: ObservableObject {
    @Published var windows: [CaptureWindow] = []
    @Published var isLoading = false
    @Published var errorMessage: String?

    var onSelect: (CaptureWindow) -> Void = { _ in }
    var onCancel: () -> Void = {}

    private let service: CapturableWindowProviding
    private var loadTask: Task<Void, Never>?

    init(service: CapturableWindowProviding) {
        self.service = service
    }

    deinit {
        loadTask?.cancel()
    }

    func load() {
        loadTask?.cancel()
        isLoading = true
        errorMessage = nil
        loadTask = Task { [weak self, service] in
            do {
                let windows = try await service.capturableWindows()
                guard !Task.isCancelled else { return }
                self?.completeLoad(.success(windows))
            } catch is CancellationError {
                return
            } catch {
                guard !Task.isCancelled else { return }
                self?.completeLoad(.failure(error))
            }
        }
    }

    func cancel() {
        cancelLoad()
        onCancel()
    }

    func cancelLoad() {
        loadTask?.cancel()
        loadTask = nil
        isLoading = false
    }

    private func completeLoad(_ result: Result<[CaptureWindow], Error>) {
        guard loadTask != nil else { return }
        loadTask = nil
        isLoading = false
        switch result {
        case let .success(windows):
            self.windows = windows
            if windows.isEmpty { errorMessage = "No capturable windows found." }
        case let .failure(error):
            errorMessage = error.localizedDescription
        }
    }
}

struct WindowCapturePickerView: View {
    static let preferredSize = CGSize(width: 460, height: 380)

    @StateObject var viewModel: WindowCapturePickerViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PopupHeader(
                icon: "macwindow",
                title: "Choose Window",
                subtitle: "Capture a visible window",
                onClose: viewModel.cancel
            )

            if viewModel.isLoading {
                HStack { Spacer(); ProgressView(); Spacer() }
                    .frame(maxHeight: .infinity)
            } else if let error = viewModel.errorMessage {
                VStack(spacing: 10) {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                    Text(error).foregroundStyle(.secondary)
                    Button("Reload") { viewModel.load() }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(viewModel.windows) { window in
                    Button {
                        viewModel.onSelect(window)
                    } label: {
                        HStack(spacing: 10) {
                            Image(systemName: symbol(for: window))
                                .foregroundStyle(BoxTheme.accent)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(title(for: window))
                                    .lineLimit(1)
                                Text(subtitle(for: window))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                    .padding(.vertical, 4)
                }
                .listStyle(.inset)
            }
        }
        .padding(18)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
        .popupCard()
        .onAppear { viewModel.load() }
    }

    private func symbol(for window: CaptureWindow) -> String {
        window.captureMode == .visibleFrame ? "menubar.rectangle" : "macwindow"
    }

    private func title(for window: CaptureWindow) -> String {
        if let title = window.title, !title.isEmpty {
            return title
        }
        if window.captureMode == .visibleFrame {
            return "System Surface"
        }
        return "Untitled Window"
    }

    private func subtitle(for window: CaptureWindow) -> String {
        let owner = window.ownerName ?? "Unknown app"
        if window.captureMode == .visibleFrame {
            return "\(owner) - visible frame capture"
        }
        return owner
    }
}
