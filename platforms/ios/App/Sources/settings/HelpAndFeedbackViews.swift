import SwiftUI
import UIKit

private struct HelpItem: View {
  let term: String
  let detail: String

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      Text(term).font(.callout.weight(.medium))
      Text(detail)
        .font(.footnote)
        .foregroundStyle(.secondary)
        .fixedSize(horizontal: false, vertical: true)
    }
    .padding(.vertical, 2)
  }
}

struct HelpView: View {
  var body: some View {
    Form {
      Section("启用键盘") {
        HelpItem(term: "1. 打开键盘设置", detail: "前往“设置 → 通用 → 键盘 → 键盘”。")
        HelpItem(term: "2. 添加水杉输入法", detail: "选择“添加新键盘”，再选择水杉输入法。")
        HelpItem(term: "3. 切换并开始输入", detail: "在输入框长按地球键，选择水杉输入法。")
        SettingsActionRow(title: "打开系统键盘设置", detail: "直接跳到“设置”里对应的位置",
                          symbol: "gearshape.fill") {
          guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
          UIApplication.shared.open(url)
        }.accessibilityIdentifier("helpOpenKeyboardSettings")
      }
      .designRow()
      Section("打字") {
        HelpItem(term: "选择候选词", detail: "点候选栏里的词上屏。候选多于一行时，点右端的箭头展开整页。")
        HelpItem(term: "换一种输入方案", detail: "全拼、双拼、五笔等在“键盘设置 → 输入设置”里切换。")
        HelpItem(term: "换皮肤与布局", detail: "“键盘设置”里可以改皮肤、键位布局和按键间距。")
      }
      .designRow()
      Section("需要完全访问权限的功能") {
        HelpItem(term: "什么时候需要", detail: "打字统计要保存本机字数、手写首次要下载识别模型，这两项需要在系统键盘设置里开启“允许完全访问”。")
        HelpItem(term: "不开会怎样", detail: "键盘照常打字。默认离线，不开这项不影响输入本身。")
      }
      .designRow()
      Section("遇到问题") {
        HelpItem(term: "键盘里没有水杉", detail: "回到上面的启用步骤确认已添加；添加过仍看不到时，长按地球键翻一下列表。")
        HelpItem(term: "云功能连不上", detail: "云词库、皮肤社区这些要联网并登录账号。在“我的”里确认账号状态。")
        HelpItem(term: "更新后行为变了", detail: "在 App Store 确认已是最新版本，词库随版本更新。")
      }
      .designRow()
      Section("更多") {
        Link(destination: URL(string: "https://msime.app/docs/")!) {
          SettingsRowLabel(title: "完整文档", detail: "msime.app，在浏览器里打开",
                           symbol: "book.fill")
        }
      }
      .designRow()
    }
    .designPage()
    .navigationTitle("使用帮助")
    .navigationBarTitleDisplayMode(.inline)
  }
}

struct FeedbackView: View {
  /// 反馈类型，与 Android 反馈页提供的三种相同。
  private enum Kind: String, CaseIterable {
    case bug = "问题", suggestion = "建议", dictionary = "词库纠错"
  }

  static let qqGroup = "829919142"
  private static let maximumDetail = 500

  @State private var kind = Kind.bug
  @State private var includeDiagnostics = false
  @State private var detail = ""
  @FocusState private var editingDetail: Bool

  private var trimmedDetail: String { detail.trimmingCharacters(in: .whitespacesAndNewlines) }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 28) {
        // 移动端设计把帮助从关于挪到了这里，让用户想找的答案出现在他们原本要写的反馈之前。
        DesignGroup {
          NavigationLink(destination: HelpView()) {
            DesignNavRowLabel(title: "使用帮助", subtitle: "启用键盘、输入方案、常见问题")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("feedbackHelpLink")
        }
        DesignGroup(title: "类型") {
          DesignSelectRow(title: "反馈类型",
                          options: Kind.allCases.map { DesignOption(title: $0.rawValue, value: $0) },
                          selection: $kind, sheetTitle: "反馈类型", identifier: "feedbackKindPicker")
          DesignDivider()
          DesignToggleRow(title: "附带诊断信息", subtitle: "只附带机型、系统和应用版本、当前方案与皮肤，不含输入内容",
                          isOn: $includeDiagnostics)
            .accessibilityIdentifier("feedbackDiagnosticsToggle")
        }
        VStack(alignment: .leading, spacing: 7) {
          DesignGroup(title: "描述") { editor }
          Text("\(detail.count) / \(Self.maximumDetail)")
            .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
            .monospacedDigit()
            .padding(.horizontal, 20)
            .accessibilityLabel("已输入 \(detail.count) 个字，最多 \(Self.maximumDetail) 个")
        }
        actions
        DesignGroup(footer: "提交问题时建议附上：系统版本、输入方案、复现步骤、相关截图，以及诊断日志。") {
          NavigationLink(destination: DiagnosticLogSettingsView()) {
            DesignNavRowLabel(title: "诊断日志", subtitle: "键盘出现问题时，开启后复现一次再分享")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("feedbackDiagnosticLogLink")
        }
        // 与 Windows 和 macOS 反馈页列出的渠道相同。
        DesignGroup(title: "交流") {
          DesignInlineButtonRow(title: "QQ 交流群", subtitle: "群号：\(Self.qqGroup)，日常交流与测试反馈",
                                buttonTitle: "复制群号", doneTitle: "已复制", identifier: "feedbackQQGroup") {
            UIPasteboard.general.string = Self.qqGroup
            ToastCenter.shared.show("已复制群号")
          }
          DesignDivider()
          Link(destination: URL(string: "https://t.me/msimegroup")!) {
            DesignNavRowLabel(title: "Telegram 群组", subtitle: "t.me/msimegroup，面向国际用户和开发者")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("feedbackTelegram")
        }
      }
      .padding(.horizontal, 16).padding(.top, 8).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .scrollDismissesKeyboard(.interactively)
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("帮助与反馈")
    .navigationBarTitleDisplayMode(.inline)
    .toolbar {
      ToolbarItemGroup(placement: .keyboard) {
        Spacer()
        Button("完成") { editingDetail = false }
      }
    }
    .onChange(of: detail) { _, value in
      if value.count > Self.maximumDetail { detail = String(value.prefix(Self.maximumDetail)) }
    }
  }

  /// 150pt 高的描述编辑框，带占位文字。
  private var editor: some View {
    TextEditor(text: $detail)
      .font(.system(size: 17))
      .scrollContentBackground(.hidden)
      .focused($editingDetail)
      .frame(height: 150)
      .padding(.horizontal, 11).padding(.vertical, 8)
      .overlay(alignment: .topLeading) {
        if detail.isEmpty {
          Text("遇到了什么问题？可以写复现步骤、出错的词或期望的结果")
            .font(.system(size: 17)).foregroundStyle(MetasequoiaTheme.sub)
            .padding(.horizontal, 16).padding(.vertical, 16)
            .allowsHitTesting(false)
            .accessibilityHidden(true)
        }
      }
      .accessibilityLabel("描述")
      .accessibilityIdentifier("feedbackDetailEditor")
  }

  private var actions: some View {
    let filled = !trimmedDetail.isEmpty
    return VStack(spacing: 6) {
      Button(action: submit) {
        Text("在 GitHub 提交").font(.system(size: 17, weight: .semibold))
          .foregroundStyle(filled ? MetasequoiaTheme.onAccent : MetasequoiaTheme.sub)
          .frame(maxWidth: .infinity, minHeight: 50)
          .background(filled ? MetasequoiaTheme.accent : MetasequoiaTheme.segBg,
                      in: RoundedRectangle(cornerRadius: 14, style: .continuous))
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .disabled(!filled)
      .accessibilityIdentifier("feedbackSubmitButton")
      Button {
        UIPasteboard.general.string = report()
        ToastCenter.shared.show("已复制报告")
      } label: {
        Text("复制报告").font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.accent)
          .frame(maxWidth: .infinity, minHeight: 44)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityIdentifier("feedbackCopyButton")
      Text("提交会打开 GitHub 并预填这份报告。")
        .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
        .frame(maxWidth: .infinity)
        .multilineTextAlignment(.center)
    }
  }

  /// 机型、系统和应用版本、当前方案与皮肤。不含用户在键盘里输入的任何内容。
  private func diagnostics() -> String {
    let info = Bundle.main.infoDictionary
    let version = info?["CFBundleShortVersionString"] as? String ?? "开发构建"
    let build = info?["CFBundleVersion"] as? String ?? "-"
    let device = UIDevice.current
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    let scheme = InputSchemePreference.current(in: document).scheme.shortLabel
    let skin = KeyboardTheme.resolve(document: document).title
    return "水杉输入法 \(version)（构建 \(build)）\n\(device.systemName) \(device.systemVersion)\n\(Self.machine())\n方案：\(scheme)\n皮肤：\(skin)"
  }

  /// 硬件型号标识，例如 iPhone16,2；`UIDevice.model` 只会给出 iPhone 或 iPad。
  private static func machine() -> String {
    if let simulated = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] { return simulated }
    var system = utsname()
    uname(&system)
    let identifier = withUnsafeBytes(of: &system.machine) { bytes in
      String(decoding: bytes.prefix { $0 != 0 }, as: UTF8.self)
    }
    return identifier.isEmpty ? UIDevice.current.model : identifier
  }

  private func report() -> String {
    var text = "### 类型\n\(kind.rawValue)\n\n### 描述\n\(detail)\n"
    if includeDiagnostics { text += "\n### 环境\n\(diagnostics())\n" }
    return text
  }

  private func submit() {
    guard !trimmedDetail.isEmpty else { return }
    let body = String(report().prefix(4000))
    var components = URLComponents(string: "https://github.com/metasequoiaime/msime/issues/new")
    components?.queryItems = [
      URLQueryItem(name: "title", value: kind.rawValue),
      URLQueryItem(name: "body", value: body)
    ]
    guard let url = components?.url else { return }
    UIApplication.shared.open(url)
  }
}
