import SwiftUI

struct AudioSourcePickerView: View {
    @Binding var audioSource: RecordingAudioSource
    @Binding var microphoneDeviceID: String?
    var compact = false

    @State private var microphoneDevices: [RecordingMicrophoneDevice] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if compact {
                audioPicker.pickerStyle(.menu).labelsHidden()
            } else {
                audioPicker.pickerStyle(.segmented)
            }

            if audioSource.includesMicrophone, microphoneDevices.count > 1 {
                Picker("Microphone", selection: Binding(
                    get: { microphoneDeviceID ?? "" },
                    set: { microphoneDeviceID = $0.isEmpty ? nil : $0 }
                )) {
                    Text("System Default").tag("")
                    ForEach(microphoneDevices) { device in
                        Text(device.name).tag(device.id)
                    }
                }
            }
        }
        .onAppear(perform: reloadMicrophones)
        .onChange(of: audioSource) { _ in reloadMicrophones() }
    }

    private var audioPicker: some View {
        Picker("Audio", selection: $audioSource) {
            ForEach(RecordingAudioSource.allCases) { source in
                Text(source.label).tag(source)
            }
        }
    }

    private func reloadMicrophones() {
        microphoneDevices = RecordingMicrophoneDevices.available()
        guard let microphoneDeviceID else { return }
        if !microphoneDevices.contains(where: { $0.id == microphoneDeviceID }) {
            self.microphoneDeviceID = nil
        }
    }
}
