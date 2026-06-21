import AppKit
import SwiftUI

struct HotkeyRecorderView: View {
    @ObservedObject var settings: AppSettings

    var body: some View {
        HotkeyRecorderControl(
            hotkey: settings.globalHotkey,
            defaultHotkey: .default,
            setHotkey: {
                settings.setGlobalHotkey($0)
                settings.globalHotkeyEnabled = true
            },
            resetHotkey: {
                settings.resetGlobalHotkey()
            },
            isEnabled: settings.globalHotkeyEnabled,
            activeRecorderID: settings.activeShortcutRecorderID,
            setActiveRecorderID: {
                settings.activeShortcutRecorderID = $0
            }
        )
    }
}

struct ScreenshotHotkeyRecorderView: View {
    @ObservedObject var settings: AppSettings

    var body: some View {
        HotkeyRecorderControl(
            hotkey: settings.screenshotHotkey,
            defaultHotkey: .defaultScreenshot,
            setHotkey: {
                settings.setScreenshotHotkey($0)
                settings.screenshotHotkeyEnabled = true
            },
            resetHotkey: {
                settings.resetScreenshotHotkey()
            },
            isEnabled: settings.screenshotHotkeyEnabled,
            activeRecorderID: settings.activeShortcutRecorderID,
            setActiveRecorderID: {
                settings.activeShortcutRecorderID = $0
            }
        )
    }
}

struct RecordingHotkeyRecorderView: View {
    @ObservedObject var settings: AppSettings

    var body: some View {
        HotkeyRecorderControl(
            hotkey: settings.recordingHotkey,
            defaultHotkey: .defaultRecording,
            setHotkey: {
                settings.setRecordingHotkey($0)
                settings.recordingHotkeyEnabled = true
            },
            resetHotkey: {
                settings.resetRecordingHotkey()
            },
            isEnabled: settings.recordingHotkeyEnabled,
            activeRecorderID: settings.activeShortcutRecorderID,
            setActiveRecorderID: {
                settings.activeShortcutRecorderID = $0
            }
        )
    }
}

private struct HotkeyRecorderControl: View {
    var hotkey: GlobalHotkey
    var defaultHotkey: GlobalHotkey
    var setHotkey: (GlobalHotkey) -> Void
    var resetHotkey: () -> Void
    var isEnabled: Bool
    var activeRecorderID: UUID?
    var setActiveRecorderID: (UUID?) -> Void = { _ in }

    @State private var recorderID = UUID()
    @State private var isRecording = false
    @State private var ownsActiveRecorder = false
    @State private var localMonitor: Any?
    @State private var message: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Text(isRecording ? "Press shortcut…" : hotkey.displayString)
                    .font(.system(.body, design: .monospaced).weight(.semibold))
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .frame(minWidth: 108)
                    .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.07)))
                    .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.primary.opacity(0.08), lineWidth: 1))

                Button(isRecording ? "Cancel" : "Change") {
                    isRecording ? stopRecording() : startRecording()
                }

                Button("Reset") {
                    resetHotkey()
                    message = nil
                }
                .disabled(hotkey == defaultHotkey)
            }

            if let message {
                Text(message)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .onChange(of: activeRecorderID) { activeID in
            guard isRecording, activeID != recorderID else { return }
            stopRecording(releaseActiveRecorder: false)
        }
        .onChange(of: isEnabled) { enabled in
            if !enabled {
                stopRecording()
            }
        }
        .onDisappear { stopRecording() }
    }

    private func startRecording() {
        stopRecording()
        setActiveRecorderID(recorderID)
        ownsActiveRecorder = true
        isRecording = true
        message = "Use at least one modifier: Control, Option, Shift, or Command. Press Esc to cancel."
        localMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { event in
            capture(event)
            return nil
        }
    }

    private func capture(_ event: NSEvent) {
        if event.keyCode == 53 {
            stopRecording()
            return
        }

        guard let hotkey = GlobalHotkey.from(event: event) else {
            message = "That shortcut is not valid. Add a modifier and a regular key."
            return
        }

        setHotkey(hotkey)
        stopRecording()
    }

    private func stopRecording(releaseActiveRecorder: Bool = true) {
        let wasRecording = isRecording
        if let localMonitor {
            NSEvent.removeMonitor(localMonitor)
        }
        localMonitor = nil
        isRecording = false
        message = nil
        if wasRecording, ownsActiveRecorder, releaseActiveRecorder {
            setActiveRecorderID(nil)
        }
        ownsActiveRecorder = false
    }
}
