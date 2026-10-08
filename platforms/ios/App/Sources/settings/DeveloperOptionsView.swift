import SwiftUI
import UIKit

@_silgen_name("msime_client_diagnostic_bundle")
private func msimeDeveloperDiagnosticBundle(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_load_preferences")
private func msimeDeveloperLoadPreferences(_ directory: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_restore_default_preferences")
private func msimeDeveloperRestoreDefaultPreferences(_ directory: UnsafePointer<UInt8>?, _ length: UInt,
                                                     _ expectedRevision: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeDeveloperStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// 开发者选项，对应 Android 的 `DeveloperPage`，但不含其中的 MCP 上传、「显示调试信息」和「日志级别」：iOS 没有支撑这几项的后端或键盘能力。「调试」一组放键盘诊断日志的开关及其页面；「数据」一组导出诊断包、恢复出厂设置。
struct DeveloperOptionsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var logEnabled = false
  @State private var logSaveFailed = false
  @State private var exporting = false
  @State private var exported: DiagnosticBundleFile?
  @State private var confirmsReset = false
  @State private var resetting = false

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "调试", footer: logSaveFailed ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。" : nil) {
          DesignToggleRow(title: "记录输入日志", subtitle: "仅保存在本机，不会上传",
                          isOn: Binding(get: { logEnabled }, set: saveLog))
            .accessibilityIdentifier("developerDiagnosticLogToggle")
          DesignDivider()
          NavigationLink(destination: DiagnosticLogSettingsView()) {
            DesignNavRowLabel(title: "查看日志")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("diagnosticLogLink")
        }

        DesignGroup(title: "数据") {
          DesignInlineButtonRow(title: "导出诊断包", buttonTitle: exporting ? "导出中…" : "导出",
                                identifier: "diagnosticBundleExport", action: export)
            .disabled(exporting || resetting)
          DesignDivider()
          DesignInlineButtonRow(title: "重置所有设置", subtitle: "恢复为出厂设置，词库不受影响",
                                buttonTitle: resetting ? "重置中…" : "重置", identifier: "resetAllSettings") {
            confirmsReset = true
          }
          .disabled(exporting || resetting)
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("开发者选项").navigationBarTitleDisplayMode(.inline)
    .alert("重置所有设置？", isPresented: $confirmsReset) {
      Button("取消", role: .cancel) {}
      Button("重置", role: .destructive, action: reset)
    } message: {
      Text("所有设置恢复为出厂设置，词库、统计和服务凭据不受影响")
    }
    .sheet(item: $exported) { file in
      DiagnosticBundleShareView(url: file.url) { completed in
        exported = nil
        if completed { ToastCenter.shared.show("诊断包已导出") }
      }
      .onDisappear { try? FileManager.default.removeItem(at: file.url.deletingLastPathComponent()) }
    }
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { reload() }
    }
  }

  /// 与 `DiagnosticLogSettingsView` 写的是同一处：共享文档里的 `diagnostic_log.server`，键盘下次出现时读到。
  private func saveLog(_ value: Bool) {
    logEnabled = value
    logSaveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences {
      var diagnostic = $0["diagnostic_log"] as? [String: Any] ?? [:]
      diagnostic["server"] = value
      $0["diagnostic_log"] = diagnostic
    }
    if logSaveFailed { reload() }
  }

  private func reload() {
    logEnabled = DiagnosticLog.isEnabled(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
  }

  private func export() {
    guard !exporting else { return }
    exporting = true
    Task {
      let url = await Task.detached(priority: .userInitiated) { DeveloperDiagnostics.exportBundle() }.value
      exporting = false
      guard let url else {
        ToastCenter.shared.show("诊断包生成失败，请重试")
        return
      }
      exported = DiagnosticBundleFile(url: url)
    }
  }

  private func reset() {
    guard !resetting else { return }
    resetting = true
    Task {
      let restored = await Task.detached(priority: .userInitiated) { DeveloperDiagnostics.restoreDefaults() }.value
      if restored {
        DeveloperDiagnostics.resetLocalPreferences()
        // 恢复出厂设置会把共享文档里的 `theme` 和 `settings_theme` 一并改回默认值，与「明暗与候选颜色」写这两个键时一样通知窗口重新应用界面样式，不必等 App 回到前台。
        NotificationCenter.default.post(name: AppAppearancePreference.didChange, object: nil)
      }
      resetting = false
      reload()
      ToastCenter.shared.show(restored ? "已重置" : "重置失败，请重试")
    }
  }
}

/// 已写好、等待分享的诊断包；分享面板关闭时删除它所在的目录。
private struct DiagnosticBundleFile: Identifiable {
  let url: URL
  var id: String { url.path }
}

/// 分享诊断包用的系统分享面板。`onFinish` 报告用户是否分享或保存了它。
private struct DiagnosticBundleShareView: UIViewControllerRepresentable {
  let url: URL
  let onFinish: (Bool) -> Void

  func makeUIViewController(context: Context) -> UIActivityViewController {
    let controller = UIActivityViewController(activityItems: [url], applicationActivities: nil)
    controller.completionWithItemsHandler = { _, completed, _, _ in onFinish(completed) }
    return controller
  }

  func updateUIViewController(_ controller: UIActivityViewController, context: Context) { }
}

/// 「数据」一组背后的两个 Rust 调用：`msime_client_diagnostic_bundle` 和 `msime_client_restore_default_preferences`。两者都要读写文件，所以不在主线程上运行。
private enum DeveloperDiagnostics {
  /// Rust 把诊断包里的崩溃记录限制在 50 条，保留最新的；读取更多文件是白费功夫。
  private static let crashRecordLimit = 50
  /// Rust 对单条记录的长度上限（`MAX_CRASH_MESSAGE_BYTES`、`MAX_CRASH_STACK_BYTES`）；超出的记录会被整条丢弃。
  private static let crashMessageBytes = 2 * 1024
  private static let crashStackBytes = 16 * 1024
  /// 崩溃记录文件是一行摘要加一段调用栈；远大于此的文件就不是崩溃记录。
  private static let crashFileBytes = 64 * 1024

  /// 在新建的临时目录里写一个 zip，内含崩溃记录和脱敏后的配置快照，返回它，失败时返回 nil。iOS 不记录性能日志和输入事件，所以不包含这两项。
  static func exportBundle() -> URL? {
    let stateRoot = MetasequoiaInputSessionBridge.sharedStateDirectory
    guard stateRoot.hasPrefix("/") else { return nil }
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("diagnostics-\(UUID().uuidString)", isDirectory: true)
    do {
      try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
                                              attributes: [.posixPermissions: 0o700])
    } catch {
      return nil
    }
    let crashLog = writeCrashLog(into: directory)
    defer { if let crashLog { try? FileManager.default.removeItem(at: crashLog) } }
    let stamp = DateFormatter()
    stamp.locale = Locale(identifier: "en_US_POSIX")
    stamp.dateFormat = "yyyyMMdd-HHmmss"
    let destination = directory.appendingPathComponent("msime-diagnostics-\(stamp.string(from: Date())).zip")
    let request: [String: Any] = [
      "state_root": stateRoot,
      "include": ["crash_logs": true, "performance_logs": false, "input_events": false, "config_snapshot": true],
      "sources": ["crash_logs": crashLog.map { $0.path as Any } ?? NSNull(), "performance_logs": NSNull(), "input_events": NSNull()],
      "destination": destination.path,
    ]
    guard let body = try? JSONSerialization.data(withJSONObject: request),
          let value = call(body, { msimeDeveloperDiagnosticBundle($0, $1) }) as? [String: Any],
          let path = value["path"] as? String,
          FileManager.default.fileExists(atPath: path) else {
      try? FileManager.default.removeItem(at: directory)
      return nil
    }
    return URL(fileURLWithPath: path)
  }

  /// 以刚读到的 revision 把共享文档替换成本版本的默认设置（Rust 保留服务凭据，不动词库和统计），再把新文档复制到 App 和键盘读取的 App Group 镜像里。没有写入任何内容时返回 false，例如键盘在这期间保存过。
  static func restoreDefaults() -> Bool {
    let directory = Data(MetasequoiaInputSessionBridge.sharedStateDirectory.utf8)
    guard directory.first == UInt8(ascii: "/"), directory.count <= 16_384,
          let loaded = call(directory, { msimeDeveloperLoadPreferences($0, $1) }) as? [String: Any],
          let revision = revision(loaded["revision"]) else { return false }
    let restored = directory.withUnsafeBytes { bytes in
      reply(msimeDeveloperRestoreDefaultPreferences(bytes.bindMemory(to: UInt8.self).baseAddress,
                                                    UInt(directory.count), revision))
    }
    guard let snapshot = restored as? [String: Any], let document = snapshot["preferences"] as? [String: Any] else {
      return false
    }
    InputSchemePreference.mirrorRestoredDefaults(document)
    ChineseOutputPreference.mirror(document)
    WubiProfilePreference.mirror(document)
    GlobalThemePreference.mirror(document)
    return true
  }

  /// 「重置所有设置」还要覆盖只存在本设备 App Group、不在 `restoreDefaults` 替换的文档里的设置，与 Android 连同共享设置一起重置本机设置一样：按键反馈、隐私模式、单手模式、工具栏的常用语 / 输入方式按钮和显示方式、键盘几何尺寸的镜像、iPad 和手势开关（滑行输入、滑动输入符号、长按空格语音）、中英模式记忆、五笔开关、键盘每次按键都读的候选与组字开关、候选栏使用主题配色、手写和 App 主题。删掉一个键，读取方就回到它的默认值。在恢复成功之后运行；放在主 actor 上，因为其中一些类型隔离在主 actor。
  @MainActor static func resetLocalPreferences() {
    KeyboardLayoutPreference.resetToDefaults()
    let local: [(UserDefaults, [String])] = [
      (KeyboardLayoutPreference.defaults, [KeyboardLayoutPreference.tabletSplitKey, KeyboardLayoutPreference.tabletFullKeysKey,
                                           KeyboardLayoutPreference.glideTypingKey, KeyboardLayoutPreference.oneHandedKey,
                                           KeyboardLayoutPreference.spaceVoiceKey, KeyboardLayoutPreference.swipeSymbolsKey]),
      (KeyboardFeedbackPreference.defaults, [KeyboardFeedbackPreference.soundKey, KeyboardFeedbackPreference.hapticsKey,
                                             KeyboardFeedbackPreference.strengthKey]),
      (KeyboardPrivacyPreference.defaults, [KeyboardPrivacyPreference.incognitoKey]),
      (TouchToolbarLocalPreference.defaults, TouchToolbarLocalPreference.keys),
      (ImeModeMemoryPreference.defaults, [ImeModeMemoryPreference.enabledKey, ImeModeMemoryPreference.lastChineseKey]),
      (WubiMixedPinyinPreference.defaults, [WubiMixedPinyinPreference.enabledKey]),
      (WubiCodeHintPreference.defaults, [WubiCodeHintPreference.enabledKey]),
      (EnglishSuggestionsPreference.defaults, [EnglishSuggestionsPreference.enabledKey]),
      (InlinePreeditPreference.defaults, [InlinePreeditPreference.styleKey, InlinePreeditPreference.key]),
      (CloudCandidatePreference.defaults, [CloudCandidatePreference.key]),
      (CandidatePageSizePreference.defaults, [CandidatePageSizePreference.key]),
      (CandidatePalette.defaults, [CandidatePalette.followsDesktopKey]),
    ]
    for (defaults, keys) in local {
      for key in keys { defaults.removeObject(forKey: key) }
    }
    HandwritingPreference.reset()
    AppThemePalette.resetToDefault()
  }

  /// App 的 MetricKit 崩溃记录（`UsageReporting.storeCrashDiagnostic`，由 `CrashDiagnostics` 写入）以及键盘留下的记录，写成 `msime_client_diagnostic_bundle` 读取的 `{at, message, stack}` 行。键盘已排队上报的记录已从目录中移走，所以这里只有仍留在设备上的那些。没有记录时返回 nil。
  private static func writeCrashLog(into directory: URL) -> URL? {
    // 目录本身或它的上级是符号链接时不读，与写入方 `UsageReporting.storeCrashDiagnostic` 的检查一致。
    guard let crashes = UsageReporting.directory?.appendingPathComponent("telemetry-crashes", isDirectory: true),
          !SafePath.hasRefusedSymbolicLink(crashes) else { return nil }
    let keys: [URLResourceKey] = [.isRegularFileKey, .isSymbolicLinkKey, .contentModificationDateKey, .fileSizeKey]
    guard let files = try? FileManager.default.contentsOfDirectory(at: crashes, includingPropertiesForKeys: keys,
                                                                   options: [.skipsHiddenFiles]) else { return nil }
    let records = files.compactMap { file -> (date: Date, line: Data)? in
      guard file.pathExtension == "crash",
            let values = try? file.resourceValues(forKeys: Set(keys)),
            values.isRegularFile == true, values.isSymbolicLink != true,
            (values.fileSize ?? 0) <= crashFileBytes,
            // 上面的属性只是粗筛；真正读取时用不跟随符号链接、限定长度的读法，打开的文件被换成链接或长过上限就跳过。
            let data = try? BoundedFileReader.read(from: file, maximumBytes: crashFileBytes) else { return nil }
      let date = values.contentModificationDate ?? Date(timeIntervalSince1970: 0)
      let text = String(decoding: data, as: UTF8.self)
      let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
      let message = prefix(lines.first ?? "", bytes: crashMessageBytes)
      guard !message.trimmingCharacters(in: .whitespaces).isEmpty else { return nil }
      let record: [String: String] = [
        "at": ISO8601DateFormatter().string(from: date),
        "message": message,
        "stack": stack(lines.dropFirst(), bytes: crashStackBytes),
      ]
      guard let line = try? JSONSerialization.data(withJSONObject: record) else { return nil }
      return (date, line)
    }
    .sorted { $0.date < $1.date }
    .suffix(crashRecordLimit)
    guard !records.isEmpty else { return nil }
    var body = Data()
    for record in records {
      body.append(record.line)
      body.append(UInt8(ascii: "\n"))
    }
    let file = directory.appendingPathComponent("crash_logs.ndjson")
    guard FileManager.default.createFile(atPath: file.path, contents: body, attributes: [.posixPermissions: 0o600]) else {
      return nil
    }
    return file
  }

  /// `text` 在 `bytes` 个 UTF-8 字节以内的最长前缀，在字符边界处截断。
  private static func prefix(_ text: String, bytes: Int) -> String {
    guard text.utf8.count > bytes else { return text }
    var result = ""
    var used = 0
    for character in text {
      let size = String(character).utf8.count
      guard used + size <= bytes else { break }
      result.append(character)
      used += size
    }
    return result
  }

  /// 调用栈开头能放进 `bytes` 字节的那些行，在行边界处截断，不会只留下半个栈帧。
  private static func stack<Lines: Sequence>(_ lines: Lines, bytes: Int) -> String where Lines.Element == String {
    var kept: [String] = []
    var used = 0
    for line in lines {
      let size = line.utf8.count + (kept.isEmpty ? 0 : 1)
      guard used + size <= bytes else { break }
      kept.append(line)
      used += size
    }
    return kept.joined(separator: "\n")
  }

  /// 文档的 revision，必须是精确的无符号整数；布尔值或小数都不算。
  private static func revision(_ value: Any?) -> UInt64? {
    guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
          let revision = UInt64(number.stringValue) else { return nil }
    return revision
  }

  private static func call(_ input: Data, _ function: (UnsafePointer<UInt8>?, UInt) -> UnsafeMutablePointer<CChar>?) -> Any? {
    input.withUnsafeBytes { bytes in
      reply(function(bytes.bindMemory(to: UInt8.self).baseAddress, UInt(input.count)))
    }
  }

  /// ABI 信封 `{"ok": true, "value": …}`，出错或没有回复时为 nil。
  private static func reply(_ raw: UnsafeMutablePointer<CChar>?) -> Any? {
    guard let raw else { return nil }
    defer { msimeDeveloperStringFree(raw) }
    guard let reply = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any],
          reply["ok"] as? Bool == true else { return nil }
    return reply["value"]
  }
}
