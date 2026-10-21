import SwiftUI
import UIKit

/// 键盘长按空格打开的语音会话页（msime://voice-session）：打开按住说话的后台会话，告诉用户回到原应用说话。会话本身由 `VoiceSessionService` 管，这一页关掉也不影响它。
struct VoiceSessionView: View {
  @ObservedObject private var service = VoiceSessionService.shared

  var body: some View {
    VStack(spacing: 18) {
      Spacer()
      Image(systemName: symbol)
        .font(.system(size: 44, weight: .medium))
        .foregroundStyle(MetasequoiaTheme.accent)
        .accessibilityHidden(true)
      Text(title).font(.title3.weight(.semibold)).accessibilityIdentifier("voiceSessionTitle")
      Text(detail)
        .font(.subheadline).foregroundStyle(.secondary)
        .multilineTextAlignment(.center)
        .accessibilityIdentifier("voiceSessionDetail")
      actions
      Spacer()
      Text("会话开着时状态栏会显示麦克风标记。只有在键盘上按住空格的那一段时间录音识别，其余时间的声音直接丢弃，不保存也不上传。")
        .font(.footnote).foregroundStyle(.secondary)
        .multilineTextAlignment(.center)
    }
    .padding(24)
    .navigationTitle("语音输入")
    .navigationBarTitleDisplayMode(.inline)
    .task { await service.activate() }
  }

  private var symbol: String {
    switch service.status {
    case .ready: "checkmark.circle.fill"
    case .failed: "exclamationmark.triangle.fill"
    case .starting, .inactive: "mic.fill"
    }
  }

  private var title: String {
    switch service.status {
    case .starting: "正在开启语音…"
    case .ready: "语音已就绪"
    case .failed: "语音没有开启"
    case .inactive: "语音已关闭"
    }
  }

  private var detail: String {
    switch service.status {
    case .starting: "第一次使用时系统会询问麦克风权限。"
    case .ready:
      "点左上角的「◀︎ 返回」回到刚才的应用，在键盘上按住空格说话，松开上屏，上滑取消。\(Int(VoiceSessionChannel.idleLifetime / 60)) 分钟不用会自动关闭，到时再长按空格会回到这里重新开启。"
    case .failed(let message): message
    case .inactive: "在键盘上长按空格会再次开启。"
    }
  }

  @ViewBuilder private var actions: some View {
    switch service.status {
    case .ready:
      Button("关闭语音", role: .destructive) { service.deactivate() }
        .accessibilityIdentifier("voiceSessionClose")
    case .failed:
      Button("重试") { Task { await service.activate() } }
        .buttonStyle(.borderedProminent)
      Button("打开系统设置") {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
      }
    case .inactive:
      Button("重新开启") { Task { await service.activate() } }
        .buttonStyle(.borderedProminent)
    case .starting:
      ProgressView()
    }
  }
}
