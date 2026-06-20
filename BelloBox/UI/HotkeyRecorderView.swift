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
            }
        )
    }
}

private struct HotkeyRecorderControl: View {
    var hotkey: GlobalHotkey
    var defaultHotkey: GlobalHotkey
    var setHotkey: (GlobalHotkey) -> Void
    var resetHotkey: () -> Void

    @State private var isRecording = false
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
        .onDisappear { stopRecording() }
    }

    private func startRecording() {
        stopRecording()
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

    private func stopRecording() {
        if let localMonitor {
            NSEvent.removeMonitor(localMonitor)
        }
        localMonitor = nil
        isRecording = false
        message = nil
    }
}
