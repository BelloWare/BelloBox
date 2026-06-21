import SwiftUI

struct RecordingPermissionView: View {
    let options: RecordingOptions
    var onRequestScreenRecording: () -> Void
    var onRequestMicrophone: () -> Void
    var onRequestInputMonitoring: () -> Void
    var onOpenAccessibility: () -> Void
    var onContinueWithoutOptional: () -> Void
    var onCancel: () -> Void

    @State private var permissions: RecordingPermissionState
    private let permissionTimer = Timer.publish(every: 1.0, on: .main, in: .common).autoconnect()

    init(
        permissions: RecordingPermissionState,
        options: RecordingOptions,
        onRequestScreenRecording: @escaping () -> Void,
        onRequestMicrophone: @escaping () -> Void,
        onRequestInputMonitoring: @escaping () -> Void,
        onOpenAccessibility: @escaping () -> Void,
        onContinueWithoutOptional: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.options = options
        self.onRequestScreenRecording = onRequestScreenRecording
        self.onRequestMicrophone = onRequestMicrophone
        self.onRequestInputMonitoring = onRequestInputMonitoring
        self.onOpenAccessibility = onOpenAccessibility
        self.onContinueWithoutOptional = onContinueWithoutOptional
        self.onCancel = onCancel
        _permissions = State(initialValue: permissions)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PopupHeader(
                icon: "record.circle",
                title: "Recording Permissions",
                subtitle: "Review what Bello Box needs",
                onClose: onCancel
            )

            permissionRow(
                title: "Screen Recording",
                detail: "Required to capture video.",
                status: permissions.screenRecording,
                action: onRequestScreenRecording
            )
            permissionRow(
                title: "Microphone",
                detail: "Needed only when microphone audio is enabled.",
                status: permissions.microphone,
                action: onRequestMicrophone
            )
            permissionRow(
                title: "Input Monitoring",
                detail: "Needed for click and keystroke overlays.",
                status: permissions.inputMonitoring,
                action: onRequestInputMonitoring
            )
            permissionRow(
                title: "Accessibility",
                detail: "Needed to detect password fields and protect printable key overlays.",
                status: permissions.accessibility,
                action: onOpenAccessibility
            )

            if let warning = secureFieldRedactionWarning {
                Label(warning, systemImage: "lock.slash")
                    .font(.caption2)
                    .foregroundStyle(.orange)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text("Bello Box hides detected secure fields and suppresses key overlays while typing into them. Microphone audio may still include anything spoken aloud.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack {
                Button("Cancel", action: onCancel)
                    .buttonStyle(SecondaryButtonStyle())
                Spacer()
                Button("Continue without optional items", action: onContinueWithoutOptional)
                    .buttonStyle(PrimaryButtonStyle())
                    .disabled(!permissions.canRecordVideo)
            }
        }
        .padding(18)
        .frame(width: 520, height: 420)
        .popupCard()
        .onReceive(permissionTimer) { _ in
            permissions = RecordingPermissionState.current(options: options)
        }
    }

    private var secureFieldRedactionWarning: String? {
        RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: permissions.accessibility == .granted)
    }

    private func permissionRow(title: String, detail: String, status: PermissionStatus, action: @escaping () -> Void) -> some View {
        HStack(spacing: 12) {
            Image(systemName: status == .granted ? "checkmark.shield.fill" : "exclamationmark.shield.fill")
                .foregroundStyle(status == .granted ? .green : .orange)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.callout.weight(.semibold))
                Text(detail).font(.caption2).foregroundStyle(.secondary)
            }
            Spacer()
            if status != .granted {
                Button("Grant…", action: action)
                    .buttonStyle(SecondaryButtonStyle())
                    .controlSize(.small)
            }
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.primary.opacity(0.05)))
    }
}
