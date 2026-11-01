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
    ("chaizi", "拆字", "输入部件拼音找不会读的字，如 huohuohuo 得 焱"),
    // 「/ 指令」：中文标点下没有输入时按 /，或在本地输入里选「指令」。与桌面的 `local_modes.command` 一样默认关闭；fy 翻译要宿主转交翻译请求，iOS 键盘不转交，所以说明里不列它。
    ("command", "指令", "输入 / 再输入指令字母：rq 日期、sj 时间、xq 星期、nl 农历，以及「插件」页启用的指令表"),
  ]

  /// 默认关闭的模式，与 client-core 的 `LocalModePreferences` 相同；其余默认打开。
  private static let offByDefault: Set<String> = ["command"]

  private static func defaultValue(_ key: String) -> Bool { !offByDefault.contains(key) }

  @State private var enabled: [String: Bool] = [:]
  @State private var saveFailed = false

  var body: some View {
    Form {
      Section {
        ForEach(Self.modes, id: \.0) { key, title, detail in
          Toggle(isOn: stored(key)) {
            SettingsOptionLabel(title: title, info: detail)
          }.accessibilityIdentifier("localMode.\(key)")
        }
      } footer: {
        if saveFailed {
          Text(SettingsSaveFailure.message)
        } else {
          Text("中文键盘在没有输入内容时，点候选栏上的名称或工具面板里的“本地输入”打开这些模式。关掉的模式不再出现在列表里。五笔、日语、韩语、粤拼、注音、越南语、藏文和笔画方案不提供本地输入。")
        }
      }
      .designRow()
    }
    .designPage()
    .navigationTitle("快捷模式").navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive(reload)
  }

  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    let stored = preferences["local_modes"] as? [String: Any] ?? [:]
    for (key, _, _) in Self.modes { enabled[key] = stored[key] as? Bool ?? Self.defaultValue(key) }
  }

  /// 每次只合并一个快捷模式；写入失败时读回已保存的值。
  private func stored(_ key: String) -> Binding<Bool> {
    Binding(get: { enabled[key] ?? Self.defaultValue(key) }, set: { value in
      enabled[key] = value
      saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences {
        var modes = $0["local_modes"] as? [String: Any] ?? [:]
        modes[key] = value
        $0["local_modes"] = modes
      }
      if saveFailed { reload() }
    })
  }
}
