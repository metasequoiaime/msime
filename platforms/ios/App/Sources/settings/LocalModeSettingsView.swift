import SwiftUI

/// Which local input modes the keyboard offers.
///
/// Like the punctuation page, these live only in the shared preference document, under `local_modes`, so each write merges one field into that object and leaves the rest as stored. The keyboard opens a mode from its candidate bar's name or its tools panel rather than a typed capital, and lists only the modes left on here; it picks a change up the next time it appears.
struct LocalModeSettingsView: View {
  private static let modes = [
    ("quick_phrase", "快捷短语", "输入编码，列出词库里的快捷短语"),
    ("date_time", "日期与时间", "输入 rq、sj 等，插入当前日期或时间"),
    ("unicode", "Unicode 码点", "输入十六进制码点，插入对应字符"),
    ("emoji", "表情", "输入关键词查找 emoji"),
    ("kaomoji", "颜文字", "输入关键词查找颜文字"),
    ("super_jianpin", "超级简拼", "按每个字的首字母查词"),
    ("temporary_english", "英文补全", "临时输入英文单词，不离开中文键盘"),
    ("temporary_japanese", "临时日语", "临时用罗马音输入日语"),
  ]

  @Environment(\.scenePhase) private var scenePhase
  @State private var enabled: [String: Bool] = [:]
  @State private var saveFailed = false

  var body: some View {
    Form {
      Section {
        ForEach(Self.modes, id: \.0) { key, title, detail in
          Toggle(isOn: stored(key)) {
            VStack(alignment: .leading, spacing: 2) {
              Text(title)
              Text(detail).font(.footnote).foregroundStyle(.secondary)
            }
          }.accessibilityIdentifier("localMode.\(key)")
        }
      } footer: {
        if saveFailed {
          Text("设置没有保存，键盘可能正在写入同一份设置，请再试一次。")
        } else {
          Text("中文键盘在没有输入内容时，点候选栏上的名称或工具面板里的“本地输入”打开这些模式。关掉的模式不再出现在列表里。五笔、日语、韩语、粤拼、注音和越南语方案不提供本地输入。")
        }
      }
    }
    .navigationTitle("本地输入模式").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
  }

  /// A binding that merges one mode into `local_modes`; a refused write puts the stored values back.
  private func stored(_ key: String) -> Binding<Bool> {
    Binding(get: { enabled[key] ?? true }, set: { value in
      enabled[key] = value
      saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences {
        var modes = $0["local_modes"] as? [String: Any] ?? [:]
        modes[key] = value
        $0["local_modes"] = modes
      }
      if saveFailed { reload() }
    })
  }

  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    let stored = preferences["local_modes"] as? [String: Any] ?? [:]
    for (key, _, _) in Self.modes { enabled[key] = stored[key] as? Bool ?? true }
  }
}
