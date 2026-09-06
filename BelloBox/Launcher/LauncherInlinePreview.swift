import SwiftUI

struct LauncherInlinePreview: View {
    let preview: LauncherPreview?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            if let preview {
                HStack(spacing: 6) {
                    Image(systemName: preview.isWarning ? "exclamationmark.circle" : "bolt.fill")
                        .foregroundStyle(preview.isWarning ? BoxTheme.warning : BoxTheme.accent)
                    Text(preview.title).fontWeight(.medium)
                    Spacer(minLength: 4)
                    Text(preview.subtitle).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
                }.font(.system(size: 10)).lineLimit(1)
                content(preview.content)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .transition(.opacity)
            } else {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Preparing preview…").font(.system(size: 11)).foregroundStyle(.secondary)
                }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
            }
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: LauncherModel.previewHeight)
        .animation(reduceMotion ? nil : .easeOut(duration: 0.14), value: preview != nil)
    }

    @ViewBuilder private func content(_ content: LauncherPreview.Content) -> some View {
        switch content {
        case .clocks(let clocks):
            HStack(spacing: 8) {
                ForEach(Array(clocks.enumerated()), id: \.offset) { _, clock in
                    VStack(alignment: .leading, spacing: 5) {
                        HStack {
                            Text(clock.name).font(.system(size: 11, weight: .medium)).lineLimit(1)
                            Spacer(minLength: 2)
                            Image(systemName: clock.quality.symbol).foregroundStyle(clock.quality.color)
                        }
                        Text(clock.time).font(.system(size: 23, weight: .medium, design: .rounded)).monospacedDigit()
                            .lineLimit(1).minimumScaleFactor(0.8)
                        Text(clock.date).font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(1)
                        Text(clock.zone).font(.system(size: 9, design: .monospaced)).foregroundStyle(.secondary)
                    }.padding(10).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                        .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 9))
                }
            }
        case .code(let text):
            Text(text).font(.system(size: 11, design: .monospaced)).lineSpacing(2).lineLimit(6)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(9).background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 8))
        case .fields(let fields):
            VStack(alignment: .leading, spacing: 6) {
                ForEach(Array(fields.enumerated()), id: \.offset) { _, field in
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text(field.label).foregroundStyle(.secondary).frame(width: 76, alignment: .leading)
                        Text(field.value).font(.system(size: 11, design: .monospaced)).frame(maxWidth: .infinity, alignment: .leading)
                    }.font(.system(size: 11)).lineLimit(1).truncationMode(.middle)
                }
            }.padding(10).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 8))
        case .statistics(let fields):
            HStack(spacing: 8) {
                ForEach(Array(fields.enumerated()), id: \.offset) { _, field in
                    VStack(alignment: .leading, spacing: 10) {
                        Text(field.label).font(.system(size: 10)).foregroundStyle(.secondary)
                        Text(field.value).font(.system(size: 23, weight: .medium, design: .rounded)).monospacedDigit()
                            .lineLimit(1).minimumScaleFactor(0.8)
                    }.padding(12).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                        .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 9))
                }
            }
        case .notice(let text):
            Text(text).font(.system(size: 12)).foregroundStyle(.secondary).lineLimit(4)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                .padding(12).background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 9))
        }
    }
}

extension LauncherPreview {
    var accessibilitySummary: String {
        let detail: String
        switch content {
        case .clocks(let clocks): detail = clocks.map { "\($0.name): \($0.time), \($0.date), \($0.zone)" }.joined(separator: "; ")
        case .code(let text), .notice(let text): detail = text
        case .fields(let fields), .statistics(let fields): detail = fields.map { "\($0.label): \($0.value)" }.joined(separator: "; ")
        }
        return "\(title). \(subtitle). \(detail)"
    }
}
