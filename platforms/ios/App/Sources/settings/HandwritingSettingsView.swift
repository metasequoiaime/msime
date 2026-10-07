import SwiftUI

/// 手写输入: showing and selecting the handwriting scheme, and what its recognizer does with ink. The notice and the ML Kit terms used to sit in 输入设置; the scheme switch there stays as well, since that page lists every scheme.
struct HandwritingSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var enabledSchemes = InputSchemePreference.enabledSchemes
  @State private var current = InputSchemePreference.scheme
  @State private var saveFailed = false

  private var enabled: Bool { enabledSchemes.contains(.handwriting) }

  var body: some View {
    Form {
      Section {
        Toggle("在键盘中显示手写", isOn: Binding(get: { enabled }, set: setEnabled))
          .disabled(enabled && enabledSchemes.count == 1)
          .accessibilityIdentifier("handwritingEnabledToggle")
        Button {
          saveFailed = !InputSchemePreference.select(.handwriting)
          reload()
        } label: {
          HStack {
            Text("设为当前方案").foregroundStyle(enabled ? Color.primary : Color.secondary)
            Spacer()
            if current == .handwriting {
              Image(systemName: "checkmark").foregroundStyle(MetasequoiaTheme.accent).accessibilityHidden(true)
            }
          }.contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .accessibilityIdentifier("handwritingSelectButton")
        .accessibilityValue(current == .handwriting ? "已选择" : "未选择")
      } footer: {
        Text(saveFailed
          ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
          : "开启后，手写出现在键盘的方案切换里。至少保留一种方案。")
      }
      Section {
        Text("首次在键盘中下载中文模型，需要完全访问权限。下载后可离线识别，笔迹和识别结果不会上传。Google ML Kit 会发送性能及使用统计。")
          .font(.footnote).foregroundStyle(.secondary)
        Link("手写 SDK 隐私说明", destination: URL(string: "https://developers.google.com/ml-kit/terms")!)
      } header: {
        SettingsGroupHeader(title: "识别模型")
      }
    }
    .navigationTitle("手写输入").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
  }

  /// 只改启用列表，选中的方案以文档里的为准：页面上的 `current` 来自 App Group 镜像，可能落后于文档，写回去会把用户在别处选的方案改掉。
  private func setEnabled(_ on: Bool) {
    saveFailed = !InputSchemePreference.setEnabled(.handwriting, on)
    reload()
  }

  private func reload() {
    InputSchemePreference.mirror(MetasequoiaInputSessionBridge.loadSharedPreferences())
    enabledSchemes = InputSchemePreference.enabledSchemes
    current = InputSchemePreference.scheme
  }
}
