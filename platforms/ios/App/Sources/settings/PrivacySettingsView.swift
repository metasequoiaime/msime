import SwiftUI

/// 隐私，从 我的 › 隐私 推入，对应 Android 的 `PrivacyPage`：匿名使用统计开关（原先在关于里）、键盘的本地剪贴板历史和快捷模式，以及隐私政策。
struct PrivacySettingsView: View {
  @State private var usageReporting = UsageReporting.isEnabled
  @State private var usageReportingFailed = false

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        Text("水杉本地优先：拼音、词库、自造词和输入统计都在这台设备上处理和保存。AI、语音、账号、云同步和皮肤社区只在你使用时联网。")
          .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
          .fixedSize(horizontal: false, vertical: true)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal, 20)

        DesignGroup(title: "联网功能", radius: MetasequoiaTheme.tabCardRadius) {
          DesignToggleRow(
            title: "发送匿名使用统计",
            subtitle: "开启时，键盘每次显示结束后记一次使用，每天记一次活跃，崩溃后附上崩溃位置（程序模块名与偏移，不含文件路径），连同版本号、平台和一个本机随机生成的安装编号发送给水杉。安装编号与设备、账号无关，不发送输入内容、联系人或任何个人信息。关闭后不再记录或发送，未发送的记录立即删除。",
            isOn: Binding(get: { usageReporting }, set: { enabled in
              if UsageReporting.setEnabled(enabled) { usageReporting = enabled } else { usageReportingFailed = true }
            }))
          .accessibilityIdentifier("usageReportingToggle")
        }

        DesignGroup(title: "本机数据", radius: MetasequoiaTheme.tabCardRadius) {
          NavigationLink { ClipboardHistorySettingsView() } label: {
            DesignNavRowLabel(title: "剪贴板历史", subtitle: "只保存在本机，最多 50 条")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("privacyClipboardHistoryLink")
          DesignDivider()
          NavigationLink { LocalModeSettingsView() } label: {
            DesignNavRowLabel(title: "快捷模式", subtitle: "快捷短语、日期时间、表情等输入模式")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("privacyLocalModeLink")
        }

        DesignGroup(radius: MetasequoiaTheme.tabCardRadius) {
          Link(destination: URL(string: "https://msime.app/privacy/")!) {
            HStack(spacing: 12) {
              Text("隐私政策").font(.system(size: 17)).foregroundStyle(.primary)
              Spacer(minLength: 8)
              Text("msime.app").font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub)
              Image(systemName: "arrow.up.right").font(.system(size: 13, weight: .semibold))
                .foregroundStyle(MetasequoiaTheme.sub.opacity(0.55)).accessibilityHidden(true)
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
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("隐私").navigationBarTitleDisplayMode(.inline)
    // 本页被盖住时键盘可能改了同一个偏好，所以每次返回都重新读取。
    .onAppear { usageReporting = UsageReporting.isEnabled }
    .alert("无法保存设置", isPresented: $usageReportingFailed) { Button("好", role: .cancel) {} } message: {
      Text("设置被键盘同时修改了，请稍后再试。")
    }
  }
}
