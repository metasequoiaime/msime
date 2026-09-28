import SwiftUI

/// The switch and the file of the keyboard's diagnostic log (see DiagnosticLog), the page Linux and macOS keep on their About pages.
///
/// The switch is `diagnostic_log.server` in the shared document, so it syncs with the desktop hosts; the Windows-only `diagnostic_log.tsf` is left as stored. The keyboard picks the change up the next time it appears. iPad has the room to show the end of the log beside the switch; the phone opens it on a page of its own.
struct DiagnosticLogSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @Environment(\.horizontalSizeClass) private var sizeClass
  @State private var enabled = false
  @State private var text = ""
  @State private var size = 0
  @State private var saveFailed = false
  @State private var confirmsClear = false

  private var file: URL? { DiagnosticLog.url(in: MetasequoiaInputSessionBridge.sharedStateDirectory) }

  var body: some View {
    Form {
      Section {
        Toggle(isOn: Binding(get: { enabled }, set: save)) {
          VStack(alignment: .leading, spacing: 2) {
            Text("记录键盘诊断日志")
            Text("键盘出现、收起、设置应用和出错的时间").font(.footnote).foregroundStyle(.secondary)
          }
        }.accessibilityIdentifier("diagnosticLogEnabled")
      } footer: {
        Text(saveFailed
          ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
          : "只记录事件名称，不记录按键、输入的文字、候选、账号或服务响应。日志保存在本机，超过 1 MB 时保留一份旧日志。键盘需要开启“完全访问”才能写入；下次弹出键盘时生效。")
      }
      Section {
        LabeledContent("大小", value: ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file))
        if let file, size > 0 {
          ShareLink(item: file) { Label("分享日志", systemImage: "square.and.arrow.up") }
            .accessibilityIdentifier("diagnosticLogShare")
          if sizeClass != .regular {
            NavigationLink("查看日志") { logText.navigationTitle("诊断日志").navigationBarTitleDisplayMode(.inline) }
              .accessibilityIdentifier("diagnosticLogView")
          }
          SettingsActionRow(title: "清空日志", symbol: "trash.fill", destructive: true) { confirmsClear = true }
            .accessibilityIdentifier("diagnosticLogClear")
        }
      } header: {
        Text("日志")
      } footer: {
        Text("反馈键盘问题时，可以把日志分享给我们。")
      }
      if sizeClass == .regular, size > 0 {
        Section("最近记录") { logText.frame(minHeight: 240) }
      }
    }
    .navigationTitle("诊断日志").navigationBarTitleDisplayMode(.inline)
    .confirmationDialog("清空诊断日志？", isPresented: $confirmsClear, titleVisibility: .visible) {
      Button("清空", role: .destructive, action: clear)
    }
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
  }

  private var logText: some View {
    ScrollView {
      Text(text).font(.caption.monospaced()).textSelection(.enabled)
        .frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 4)
    }.defaultScrollAnchor(.bottom)
  }

  private func save(_ value: Bool) {
    enabled = value
    saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences {
      var diagnostic = $0["diagnostic_log"] as? [String: Any] ?? [:]
      diagnostic["server"] = value
      $0["diagnostic_log"] = diagnostic
    }
    if saveFailed { reload() }
  }

  private func clear() {
    guard let file else { return }
    try? FileManager.default.removeItem(at: file)
    try? FileManager.default.removeItem(at: file.appendingPathExtension("1"))
    reload()
  }

  private func reload() {
    enabled = DiagnosticLog.isEnabled(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
    guard let file, let result = try? DiagnosticLog.readTail(from: file, maximumBytes: 32 * 1024) else { text = ""; size = 0; return }
    size = result.size
    // The end of the log is what a report needs; drawing a whole megabyte in one Text would stall the page.
    text = String(decoding: result.data, as: UTF8.self)
  }
}
