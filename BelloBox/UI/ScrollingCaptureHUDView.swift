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
    private var autoScrollToken: UUID?
    private var captureTask: Task<Void, Never>?
    private var captureToken: UUID?
    private var finishTask: Task<Void, Never>?
    private var finishToken: UUID?
    private var isCancelled = false

    init(coordinator: ScrollCaptureCoordinator) {
        self.coordinator = coordinator
    }

    func captureNext() {
        guard !isBusy, !isAutoScrolling, captureTask == nil, finishTask == nil else { return }
        isBusy = true
        errorMessage = nil
        let token = UUID()
        captureToken = token
        captureTask = Task { [weak self] in
            guard let self else { return }
            defer {
                if self.captureToken == token {
                    self.captureToken = nil
                    self.captureTask = nil
                }
            }
            do {
                try await coordinator.captureNextFrame()
                guard !Task.isCancelled, captureToken == token else { return }
            } catch {
                guard !Task.isCancelled, captureToken == token else { return }
                errorMessage = error.localizedDescription
            }
            guard captureToken == token else { return }
            isBusy = false
        }
    }

    func done() {
        guard !isBusy, finishTask == nil else { return }
        stopAutoScroll(allowTaskCleanup: false)
        isBusy = true
        errorMessage = nil
        let token = UUID()
        finishToken = token
        finishTask = Task { [weak self] in
            guard let self else { return }
            defer {
                if self.finishToken == token {
                    self.finishToken = nil
                    self.finishTask = nil
                }
            }
            do {
                let document = try await coordinator.finish()
                guard !Task.isCancelled, finishToken == token else { return }
                onFinished(document)
                isBusy = false
            } catch {
                guard !Task.isCancelled, finishToken == token else { return }
                errorMessage = error.localizedDescription
                isBusy = false
            }
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
        guard !isCancelled else { return }
        isCancelled = true
        stopAutoScroll(allowTaskCleanup: false)
        captureTask?.cancel()
        captureTask = nil
        captureToken = nil
        finishTask?.cancel()
        finishTask = nil
        finishToken = nil
        autoScrollToken = nil
        isBusy = false
        onCancel()
    }

    private func startAutoScroll() {
        guard !isBusy, autoScrollTask == nil, finishTask == nil else { return }
        isAutoScrolling = true
        errorMessage = "Auto-scroll is best-effort. Stop it and use Capture Next if the target app does not move."
        let token = UUID()
        autoScrollToken = token
        autoScrollTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, self.isAutoScrolling, self.autoScrollToken == token else { break }
                if self.coordinator.session.frames.count >= self.coordinator.settingsMaxFrames {
                    self.errorMessage = "Auto-scroll stopped at the configured frame limit."
                    break
                }

                self.coordinator.postAutoScrollEvent()
                try? await Task.sleep(nanoseconds: 450_000_000)

                guard !Task.isCancelled, self.isAutoScrolling, self.autoScrollToken == token else { break }
                self.isBusy = true
                do {
                    try await self.coordinator.captureNextFrame()
                    guard !Task.isCancelled, self.isAutoScrolling, self.autoScrollToken == token else { break }
                    self.isBusy = false
                    if self.coordinator.warning?.contains("mostly unchanged") == true {
                        self.errorMessage = "Auto-scroll stopped because the new frame looked unchanged."
                        break
                    }
                } catch {
                    guard !Task.isCancelled, self.isAutoScrolling, self.autoScrollToken == token else { break }
                    self.isBusy = false
                    self.errorMessage = error.localizedDescription
                    break
                }

                try? await Task.sleep(nanoseconds: 150_000_000)
            }
            guard let self, self.autoScrollToken == token else { return }
            self.isBusy = false
            self.isAutoScrolling = false
            self.autoScrollTask = nil
            self.autoScrollToken = nil
        }
    }

    private func stopAutoScroll(allowTaskCleanup: Bool = true) {
        if !allowTaskCleanup {
            autoScrollToken = nil
        }
        autoScrollTask?.cancel()
        autoScrollTask = nil
        isAutoScrolling = false
    }

    deinit {
        autoScrollTask?.cancel()
        autoScrollToken = nil
        captureTask?.cancel()
        captureToken = nil
        finishTask?.cancel()
        finishToken = nil
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
                onClose: viewModel.cancel
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
                .disabled(viewModel.isBusy || viewModel.isAutoScrolling)

                Button(viewModel.isAutoScrolling ? "Stop Auto" : "Auto-scroll") {
                    viewModel.toggleAutoScroll()
                }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(viewModel.isBusy && !viewModel.isAutoScrolling)

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
