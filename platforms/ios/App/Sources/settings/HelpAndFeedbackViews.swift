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
      Section("打字") {
        HelpItem(term: "选择候选词", detail: "点候选栏里的词上屏。候选多于一行时，点右端的箭头展开整页。")
        HelpItem(term: "换一种输入方案", detail: "全拼、双拼、五笔等在“键盘设置 → 输入设置”里切换。")
        HelpItem(term: "换皮肤与布局", detail: "“键盘设置”里可以改皮肤、键位布局和按键间距。")
      }
      Section("需要完全访问权限的功能") {
        HelpItem(term: "什么时候需要", detail: "打字统计要保存本机字数、手写首次要下载识别模型，这两项需要在系统键盘设置里开启“允许完全访问”。")
        HelpItem(term: "不开会怎样", detail: "键盘照常打字。默认离线，不开这项不影响输入本身。")
      }
      Section("遇到问题") {
        HelpItem(term: "键盘里没有水杉", detail: "回到上面的启用步骤确认已添加；添加过仍看不到时，长按地球键翻一下列表。")
        HelpItem(term: "云功能连不上", detail: "云词库、皮肤社区这些要联网并登录账号。在“我的”里确认账号状态。")
        HelpItem(term: "更新后行为变了", detail: "在 App Store 确认已是最新版本，词库随版本更新。")
      }
      Section("更多") {
        Link(destination: URL(string: "https://msime.app/docs/")!) {
          SettingsRowLabel(title: "完整文档", detail: "msime.app，在浏览器里打开",
                           symbol: "book.fill")
        }
      }
    }
    .navigationTitle("使用帮助")
    .navigationBarTitleDisplayMode(.inline)
  }
}

struct FeedbackView: View {
  @State private var kind = "功能异常"
  @State private var detail = ""
  @State private var copied = false
  @State private var copiedGroup = false

  static let qqGroup = "829919142"

  private let kinds = ["功能异常", "候选词不对", "功能建议", "其他"]

  private var diagnostics: String {
    let info = Bundle.main.infoDictionary
    let version = info?["CFBundleShortVersionString"] as? String ?? "开发构建"
    let build = info?["CFBundleVersion"] as? String ?? "-"
    let device = UIDevice.current
    return "水杉输入法 \(version)（构建 \(build)）\n\(device.systemName) \(device.systemVersion)\n\(device.model)"
  }

  private var report: String {
    "### 类型\n\(kind)\n\n### 描述\n\(detail)\n\n### 环境\n\(diagnostics)\n"
  }

  var body: some View {
    Form {
      // 帮助 moved here from 关于 in the mobile design, so the answers people look for come before the report they would otherwise write.
      Section {
        NavigationLink(destination: HelpView()) {
          SettingsRowLabel(title: "使用帮助", detail: "启用键盘、输入方案、常见问题",
                           symbol: "questionmark.circle.fill")
        }.accessibilityIdentifier("feedbackHelpLink")
      }
      Section("类型") {
        Picker("类型", selection: $kind) {
          ForEach(kinds, id: \.self) { Text($0).tag($0) }
        }
        .pickerStyle(.menu)
        .accessibilityIdentifier("feedbackKindPicker")
      }
      Section {
        TextEditor(text: $detail)
          .frame(minHeight: 120)
          .accessibilityIdentifier("feedbackDetailEditor")
      } header: {
        Text("描述")
      } footer: {
        Text("发生了什么？如果和打字有关，写出你输入的编码和期望的结果最有用。")
      }
      Section("会一起附上的信息") {
        Text(diagnostics)
          .font(.footnote.monospaced())
          .foregroundStyle(.secondary)
          .accessibilityIdentifier("feedbackDiagnostics")
      }
      Section {
        SettingsActionRow(title: copied ? "已复制报告" : "复制报告", detail: "完整内容，贴到任何地方",
                          symbol: copied ? "checkmark.circle.fill" : "doc.on.doc.fill") {
          UIPasteboard.general.string = report
          copied = true
        }.accessibilityIdentifier("feedbackCopyButton")
        SettingsActionRow(title: "在 GitHub 提交", detail: "打开浏览器并预填这份报告",
                          symbol: "arrow.up.forward.square.fill") { submit() }
          .accessibilityIdentifier("feedbackSubmitButton")
      } footer: {
        Text("提交会打开 GitHub 并预填这份报告。网址长度有限，过长的描述会被截断，完整内容请用“复制报告”。")
      }
      Section {
        NavigationLink(destination: DiagnosticLogSettingsView()) {
          SettingsRowLabel(title: "诊断日志", detail: "键盘出现问题时，开启后复现一次再分享",
                           symbol: "list.bullet.rectangle.fill")
        }.accessibilityIdentifier("feedbackDiagnosticLogLink")
      } footer: {
        Text("提交问题时建议附上：系统版本、输入方案、复现步骤、相关截图，以及诊断日志。")
      }
      // The same channels the Windows and macOS feedback pages list.
      Section("交流") {
        SettingsActionRow(title: copiedGroup ? "已复制群号" : "QQ 交流群", detail: "群号：\(Self.qqGroup)，日常交流与测试反馈",
                          symbol: copiedGroup ? "checkmark.circle.fill" : "person.3.fill") {
          UIPasteboard.general.string = Self.qqGroup
          copiedGroup = true
        }.accessibilityIdentifier("feedbackQQGroup")
        Link(destination: URL(string: "https://t.me/msimegroup")!) {
          SettingsRowLabel(title: "Telegram 群组", detail: "t.me/msimegroup，面向国际用户和开发者",
                           symbol: "paperplane.fill")
        }.accessibilityIdentifier("feedbackTelegram")
      }
    }
    .navigationTitle("帮助与反馈")
    .navigationBarTitleDisplayMode(.inline)
  }

  private func submit() {
    let body = String(report.prefix(4000))
    var components = URLComponents(string: "https://github.com/metasequoiaime/msime/issues/new")
    components?.queryItems = [
      URLQueryItem(name: "title", value: kind),
      URLQueryItem(name: "body", value: body)
    ]
    guard let url = components?.url else { return }
    UIApplication.shared.open(url)
  }
}
