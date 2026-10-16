import SwiftUI

struct KeyboardVoiceView: View {
  let entry: VoiceTextHandoff?
  let insert: () throws -> Void
  let close: () -> Void
  /// Opens the app's recording screen; nil where there is no app to open, as in the app's own preview.
  var record: (() -> Void)? = nil
  @State private var error: String?
  @State private var errorID = UUID()

  var body: some View {
    VStack(spacing: 4) {
      HStack {
        Label("语音结果", systemImage: "waveform").font(.headline)
          .dynamicTypeSize(...DynamicTypeSize.xxxLarge).accessibilityAddTraits(.isHeader)
        Spacer()
        Button("关闭", action: close).accessibilityIdentifier("keyboardServiceClose")
      }
      ScrollViewReader { proxy in
        ScrollView {
          VStack(alignment: .leading, spacing: 8) {
            if let error {
              Text(error).font(.footnote).foregroundStyle(.red)
                .id("status").accessibilityIdentifier("keyboardVoiceStatus")
            }
            if let entry {
              Text(entry.text).frame(maxWidth: .infinity, alignment: .leading)
              Text("\(entry.expiresAt.formatted(date: .omitted, time: .shortened)) 前可用；点击插入后清除待插入结果。")
                .font(.caption).foregroundStyle(.secondary)
            } else {
              Text("请在水杉 App 的“语音设置”中录音识别，点击“发送到键盘”，再返回这里插入。")
              Text("iOS 键盘不能直接录音。结果只保留最新一条，10 分钟内有效。")
                .font(.footnote).foregroundStyle(.secondary)
            }
          }
        }
        .disablingScrollEdgeEffects()
        .onChange(of: errorID) { proxy.scrollTo("status", anchor: .top) }
      }
      if entry != nil {
        Button("插入语音结果") {
          do { try insert(); close() } catch { self.error = error.localizedDescription; errorID = UUID() }
        }.frame(minHeight: 44).accessibilityIdentifier("keyboardVoiceInsert")
      }
      if let record {
        Button(entry == nil ? "去水杉 App 录音" : "重新录音", action: record)
          .frame(minHeight: 44).accessibilityIdentifier("keyboardVoiceRecord")
      }
    }
    .padding(.horizontal, 12)
    .buttonStyle(KeyboardPanelButtonStyle())
    .background(Color(uiColor: .secondarySystemBackground))
  }
}
