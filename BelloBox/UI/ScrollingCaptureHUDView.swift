import SwiftUI

@MainActor
final class ScrollingCaptureHUDViewModel: ObservableObject {
    @Published var coordinator: ScrollCaptureCoordinator
    @Published var isBusy = false
    @Published var errorMessage: String?
    @Published var isAutoScrolling = false

    var onFinished: (ScreenshotDocument) -> Void = { _ in }
    var onCancel: () -> Void = {}

    private var autoScrollTask: Task<Void, Never>?

    init(coordinator: ScrollCaptureCoordinator) {
        self.coordinator = coordinator
    }

    func captureNext() {
        guard !isBusy else { return }
        isBusy = true
        errorMessage = nil
        Task {
            do {
                try await coordinator.captureNextFrame()
            } catch {
                errorMessage = error.localizedDescription
            }
            isBusy = false
        }
    }

    func done() {
        guard !isBusy else { return }
        stopAutoScroll()
        isBusy = true
        errorMessage = nil
        do {
            let document = try coordinator.finish()
            onFinished(document)
        } catch {
            errorMessage = error.localizedDescription
            isBusy = false
        }
    }

    func toggleAutoScroll() {
        if isAutoScrolling {
            stopAutoScroll()
        } else {
            startAutoScroll()
        }
    }

    func cancel() {
        stopAutoScroll()
        onCancel()
    }

    private func startAutoScroll() {
        guard autoScrollTask == nil else { return }
        isAutoScrolling = true
        errorMessage = "Auto-scroll is best-effort. Stop it and use Capture Next if the target app does not move."
        autoScrollTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, self.isAutoScrolling else { break }
                if self.coordinator.session.frames.count >= self.coordinator.settingsMaxFrames {
                    self.errorMessage = "Auto-scroll stopped at the configured frame limit."
                    break
                }

                self.coordinator.postAutoScrollEvent()
                try? await Task.sleep(nanoseconds: 450_000_000)

                guard !Task.isCancelled, self.isAutoScrolling else { break }
                self.isBusy = true
                do {
                    try await self.coordinator.captureNextFrame()
                    self.isBusy = false
                    if self.coordinator.warning?.contains("mostly unchanged") == true {
                        self.errorMessage = "Auto-scroll stopped because the new frame looked unchanged."
                        break
                    }
                } catch {
                    self.isBusy = false
                    self.errorMessage = error.localizedDescription
                    break
                }

                try? await Task.sleep(nanoseconds: 150_000_000)
            }
            self?.isBusy = false
            self?.isAutoScrolling = false
            self?.autoScrollTask = nil
        }
    }

    private func stopAutoScroll() {
        autoScrollTask?.cancel()
        autoScrollTask = nil
        isAutoScrolling = false
        isBusy = false
    }

    deinit {
        autoScrollTask?.cancel()
    }
}

struct ScrollingCaptureHUDView: View {
    static let preferredSize = CGSize(width: 430, height: 170)

    @ObservedObject var viewModel: ScrollingCaptureHUDViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PopupHeader(
                icon: "arrow.down.doc",
                title: "Scrolling Capture",
                subtitle: "\(viewModel.coordinator.session.frames.count) frames captured",
                onClose: viewModel.onCancel
            )

            Text("Scroll the target content, then capture the next frame. Press Done when finished.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            HStack(spacing: 8) {
                Button {
                    viewModel.captureNext()
                } label: {
                    if viewModel.isBusy { ProgressView().controlSize(.small) } else { Text("Capture Next") }
                }
                .buttonStyle(PrimaryButtonStyle())
                .disabled(viewModel.isBusy)

                Button(viewModel.isAutoScrolling ? "Stop Auto" : "Auto-scroll") {
                    viewModel.toggleAutoScroll()
                }
                .buttonStyle(SecondaryButtonStyle())

                Button("Done & Stitch") { viewModel.done() }
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(viewModel.coordinator.session.frames.isEmpty || viewModel.isBusy)

                Button("Cancel") { viewModel.cancel() }
                    .buttonStyle(SecondaryButtonStyle())
            }

            if let warning = viewModel.coordinator.warning {
                Label(warning, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption2)
                    .foregroundStyle(.orange)
            }
            if let error = viewModel.errorMessage {
                Label(error, systemImage: "xmark.octagon.fill")
                    .font(.caption2)
                    .foregroundStyle(.red)
            }
        }
        .padding(16)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
        .popupCard()
    }
}
