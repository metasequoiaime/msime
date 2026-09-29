import SwiftUI

/// 开发者选项: tools for diagnosing the keyboard rather than for typing. The diagnostic log moved here from 关于.
struct DeveloperOptionsView: View {
  var body: some View {
    Form {
      Section {
        NavigationLink(destination: DiagnosticLogSettingsView()) {
          SettingsNavLabel(title: "诊断日志", symbol: "list.bullet.rectangle")
        }.accessibilityIdentifier("diagnosticLogLink")
      } footer: {
        Text("记录键盘的运行事件，反馈问题时可以一起分享。")
      }
    }
    .navigationTitle("开发者选项").navigationBarTitleDisplayMode(.inline)
  }
}
