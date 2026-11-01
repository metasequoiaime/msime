import SwiftUI

/// 隐私，从 我的 › 隐私 推入，对应 Android 的 `PrivacyPage`：匿名使用统计开关（原先在关于里）、键盘的本地剪贴板历史和快捷模式、社区里已屏蔽的作者，以及隐私政策。
struct PrivacySettingsView: View {
  @State private var usageReporting = UsageReporting.isEnabled
  @State private var usageReportingFailed = false
  /// 「已屏蔽的作者」只给登录账号：没登录时这一行打开登录面板。
  @State private var signedIn = false
  @State private var signingIn = false

  @MainActor private func refreshSignedIn() async {
    signedIn = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
  }

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "联网功能",
                    info: "水杉本地优先：拼音、词库、自造词和输入统计都在这台设备上处理和保存。AI、语音、账号、云同步和皮肤社区只在你使用时联网。",
                    radius: MetasequoiaTheme.tabCardRadius) {
          DesignToggleRow(
            title: "发送匿名使用统计",
            info: "仅发送匿名使用记录和崩溃摘要，不含输入内容、联系人或账号；关闭后立即清空待发记录。",
            isOn: Binding(get: { usageReporting }, set: { enabled in
              if UsageReporting.setEnabled(enabled) { usageReporting = enabled } else { usageReportingFailed = true }
            }))
          .accessibilityIdentifier("usageReportingToggle")
        }

        DesignGroup(title: "本机数据", radius: MetasequoiaTheme.tabCardRadius) {
          NavigationLink { ClipboardHistorySettingsView() } label: {
            DesignNavRowLabel(title: "剪贴板历史", info: "只保存在本机，可选择保留条数")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("privacyClipboardHistoryLink")
          DesignDivider()
          NavigationLink { LocalModeSettingsView() } label: {
            DesignNavRowLabel(title: "快捷模式", info: "快捷短语、日期时间、表情等输入模式")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("privacyLocalModeLink")
        }

        DesignGroup(title: "社区", radius: MetasequoiaTheme.tabCardRadius) {
          if signedIn {
            NavigationLink { CommunityBlockedAuthorsView() } label: {
              DesignNavRowLabel(title: "已屏蔽的作者", info: "社区里不显示这些作者的作品")
            }
            .buttonStyle(PressFillButtonStyle())
            .accessibilityIdentifier("privacyBlockedAuthorsLink")
          } else {
            Button { signingIn = true } label: {
              DesignNavRowLabel(title: "已屏蔽的作者", subtitle: "登录后可用")
            }
            .buttonStyle(PressFillButtonStyle())
            .accessibilityIdentifier("privacyBlockedAuthorsLink")
          }
        }

        DesignGroup(radius: MetasequoiaTheme.tabCardRadius) {
          Link(destination: URL(string: "https://msime.app/privacy/")!) {
            HStack(spacing: 12) {
              Text("隐私政策").font(.system(size: 17)).foregroundStyle(.primary)
              Spacer(minLength: 8)
              Text("msime.app").font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub)
              DesignExternalLinkIcon()
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 20)
            .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
            .contentShape(Rectangle())
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("privacyPolicyLink")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .designCanvasBackground()
    .navigationTitle("隐私").navigationBarTitleDisplayMode(.inline)
    // 本页被盖住时键盘可能改了同一个偏好，所以每次返回都重新读取。
    .onAppear { usageReporting = UsageReporting.isEnabled }
    .task { await refreshSignedIn() }
    .sheet(isPresented: $signingIn, onDismiss: { Task { await refreshSignedIn() } }) { AccountLoginSheet() }
    .alert("无法保存设置", isPresented: $usageReportingFailed) { Button("好", role: .cancel) {} } message: {
      Text("设置被键盘同时修改了，请稍后再试。")
    }
  }
}
