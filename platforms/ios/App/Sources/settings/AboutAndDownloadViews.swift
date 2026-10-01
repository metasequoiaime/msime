import SwiftUI
import UIKit

private enum DesktopPlatform: String, CaseIterable, Identifiable {
  case macOS, windows = "Windows", linux = "Linux"
  var id: String { rawValue }
  var symbol: String {
    switch self { case .macOS: "laptopcomputer"; case .windows: "pc"; case .linux: "terminal" }
  }
  var repository: String {
    // Desktop installers are published from the shared client repository. The
    // platform selector changes the guidance, not the ownership of the release.
    "msime"
  }
  var releaseURL: URL { URL(string: "https://github.com/metasequoiaime/\(repository)/releases")! }
  var steps: [String] {
    switch self {
    case .macOS:
      ["在 Mac 上打开下载页，选择适合你的 macOS 安装包。", "按照发布页说明完成安装。", "在系统设置的键盘输入法中添加水杉输入法，再切换使用。"]
    case .windows:
      ["在 Windows 电脑上打开下载页，选择与你的系统架构匹配的安装包。", "运行安装程序，按发布页说明完成安装。", "使用 Win + 空格切换到水杉输入法。"]
    case .linux:
      ["在 Linux 电脑上打开发布页，查看适用发行版与依赖要求。", "按照项目安装说明配置 IBus 和水杉输入法。", "在系统输入源中添加水杉输入法，按说明重新登录后使用。"]
    }
  }
}

struct DesktopDownloadView: View {
  @State private var platform = DesktopPlatform.macOS
  @State private var copied = false

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 24) {
        VStack(alignment: .leading, spacing: 10) {
          Image(systemName: "desktopcomputer").font(.system(size: 38)).foregroundStyle(MetasequoiaTheme.accent)
          Text("在电脑上，也用水杉").font(.title2.bold())
          Text("选择你的电脑系统，获取官方安装包与使用指南。").foregroundStyle(.secondary)
        }.padding(.top, 12)
        Picker("电脑系统", selection: $platform) {
          ForEach(DesktopPlatform.allCases) { Text($0.rawValue).tag($0) }
        }.pickerStyle(.segmented).accessibilityIdentifier("desktopPlatformPicker")
        VStack(alignment: .leading, spacing: 20) {
          Label(platform.rawValue + " 安装指南", systemImage: platform.symbol).font(.headline)
          ForEach(Array(platform.steps.enumerated()), id: \.offset) { index, step in
            HStack(alignment: .top, spacing: 12) {
              Text(String(index + 1)).font(.subheadline.bold()).foregroundStyle(MetasequoiaTheme.accent)
                .frame(width: 28, height: 28).background(MetasequoiaTheme.accentSoft, in: Circle())
              Text(step).font(.subheadline).fixedSize(horizontal: false, vertical: true)
            }
          }
          Link(destination: platform.releaseURL) {
            Label("打开官方发布页", systemImage: "arrow.down.circle")
              .frame(maxWidth: .infinity).padding(.vertical, 6)
          }.buttonStyle(.borderedProminent).accessibilityIdentifier("desktopReleaseLink")
          Button {
            UIPasteboard.general.url = platform.releaseURL
            copied = true
          } label: {
            Label(copied ? "已复制下载链接" : "复制链接，在电脑上打开", systemImage: copied ? "checkmark" : "doc.on.doc")
              .frame(maxWidth: .infinity)
          }.accessibilityIdentifier("copyDesktopDownloadLink")
        }.padding(20).background(Color(uiColor: .secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 20))
        Text("电脑安装包需要在对应系统上安装。具体系统要求、版本说明和安装步骤以官方发布页为准。").font(.footnote).foregroundStyle(.secondary)
        Link("官网与完整下载指南", destination: URL(string: "https://msime.app/download/")!)
        Link("阅读用户文档", destination: URL(string: "https://msime.app/docs/")!)
      }.padding(20)
    }.background(Color(uiColor: .systemGroupedBackground))
      .navigationTitle("电脑版下载").navigationBarTitleDisplayMode(.inline)
      .onChange(of: platform) { _ in copied = false }
  }
}

struct AboutView: View {
  @State private var usageReporting = UsageReporting.isEnabled
  @State private var usageReportingFailed = false
  private var version: String {
    let info = Bundle.main.infoDictionary ?? [:]
    return "\(info["CFBundleShortVersionString"] as? String ?? "—") (\(info["CFBundleVersion"] as? String ?? "—"))"
  }

  var body: some View {
    List {
      Section {
        VStack(spacing: 12) {
          Image("MSIMELogo").resizable().scaledToFit().frame(width: 72, height: 72)
            .accessibilityHidden(true)
          Text("水杉输入法").font(.title2.bold())
          Text("让输入更自然").foregroundStyle(.secondary)
          Text("版本 " + version).font(.footnote).foregroundStyle(.secondary).accessibilityIdentifier("aboutAppVersion")
        }.frame(maxWidth: .infinity).padding(.vertical, 20)
      }
      Section {
        NavigationLink(destination: DesktopDownloadView()) {
          SettingsRowLabel(title: "电脑版下载", detail: "macOS、Windows、Linux 的安装包与指南",
                           symbol: "desktopcomputer")
        }
      } header: {
        Text("关于水杉")
      } footer: {
        Text("水杉是一款开源输入法，支持多种输入方案和个性化皮肤。手机与电脑共用输入引擎，各平台提供原生输入体验。")
      }
      Section("帮助与开源") {
        NavigationLink(destination: HelpView()) {
          SettingsRowLabel(title: "使用帮助", detail: "启用键盘、输入方案、常见问题",
                           symbol: "questionmark.circle.fill")
        }.accessibilityIdentifier("helpLink")
        NavigationLink(destination: FeedbackView()) {
          SettingsRowLabel(title: "反馈问题与建议", detail: "在应用内写，附带版本与设备信息",
                           symbol: "bubble.left.and.bubble.right.fill")
        }.accessibilityIdentifier("feedbackLink")
        Link(destination: URL(string: "https://msime.app/")!) {
          SettingsRowLabel(title: "官方网站", detail: "msime.app", symbol: "globe")
        }
        Link(destination: URL(string: "https://github.com/metasequoiaime/msime")!) {
          SettingsRowLabel(title: "开源代码与许可证", detail: "GitHub", symbol: "curlybraces")
        }
      }
      Section {
        Toggle("发送匿名使用统计", isOn: Binding(get: { usageReporting }, set: { enabled in
          if UsageReporting.setEnabled(enabled) { usageReporting = enabled } else { usageReportingFailed = true }
        })).accessibilityIdentifier("usageReportingToggle")
      } header: {
        Text("使用统计")
      } footer: {
        Text("开启时，键盘每次显示结束后记一次使用，每天记一次活跃，崩溃后附上崩溃位置（程序模块名与偏移，不含文件路径），连同版本号、平台和一个本机随机生成的安装编号发送给水杉。安装编号与设备、账号无关，不发送输入内容、联系人或任何个人信息。关闭后不再记录或发送，未发送的记录立即删除。")
      }
      Section {
        Link(destination: URL(string: "https://msime.app/privacy/")!) {
          SettingsRowLabel(title: "隐私说明", detail: "msime.app", symbol: "hand.raised.fill")
        }
      } header: {
        Text("隐私")
      } footer: {
        Text("键盘默认离线。仅在你使用 AI 或语音时，将本次文字或录音发送到所配置的服务。账号、云同步和皮肤社区按你启用的功能联网；开启「发送匿名使用统计」时，本应用和允许完全访问的键盘会发送上面的使用统计。手写首次联网下载模型，之后在设备上识别；Google ML Kit 会发送性能及使用统计，不会上传笔迹或识别结果。")
      }
    }.navigationTitle("关于水杉").navigationBarTitleDisplayMode(.inline)
      .onAppear { usageReporting = UsageReporting.isEnabled }
      .alert("无法保存设置", isPresented: $usageReportingFailed) { Button("好", role: .cancel) {} } message: {
        Text("设置被键盘同时修改了，请稍后再试。")
      }
  }
}
