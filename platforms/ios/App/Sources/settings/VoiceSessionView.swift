import SwiftUI
import UIKit

/// 键盘长按空格打开的语音会话页（msime://voice-session），全屏盖在 App 上：打开按住说话的后台会话，告诉用户回原应用说话。会话本身由 `VoiceSessionService` 管，这一页关掉也不影响它。
///
/// 用户这时要做的只有一件事：点状态栏左上角系统给的「◀︎ 返回」。所以页面不放导航栏，左上角留给一个指向它的气泡；中间是和键盘聆听界面同样的强调色麦克风圆球和一句怎么说话；「关闭语音」只是底部的一行次要文字，不和主要的那件事抢。
struct VoiceSessionView: View {
  @ObservedObject private var service = VoiceSessionService.shared
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    VStack(spacing: 0) {
      HStack(alignment: .top) {
        if service.status == .ready { backHint } else { Spacer(minLength: 0) }
        Spacer(minLength: 12)
        Button("完成") { dismiss() }
          .font(.system(size: 17, weight: .semibold))
          .foregroundStyle(MetasequoiaTheme.accent)
          .accessibilityIdentifier("voiceSessionDone")
      }
      .padding(.horizontal, 16)
      .padding(.top, 6)
      Spacer(minLength: 24)
      VStack(spacing: 14) {
        orb
        Text(title)
          .font(.system(size: 22, weight: .semibold))
          .foregroundStyle(.primary)
          .accessibilityIdentifier("voiceSessionTitle")
        Text(detail)
          .font(.system(size: 15))
          .foregroundStyle(MetasequoiaTheme.sub)
          .multilineTextAlignment(.center)
          .fixedSize(horizontal: false, vertical: true)
          .accessibilityIdentifier("voiceSessionDetail")
        actions.padding(.top, 6)
      }
      .padding(.horizontal, 32)
      Spacer(minLength: 24)
      footer
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .designCanvasBackground()
    .task { await service.activate() }
  }

  /// 指向状态栏左上角「◀︎ 返回」的气泡。返回按钮是系统画的，App 碰不到它，只能指给用户看。
  private var backHint: some View {
    HStack(spacing: 6) {
      Image(systemName: "arrow.up.left").font(.system(size: 14, weight: .bold))
      Text("点左上角回到刚才的应用").font(.system(size: 14, weight: .semibold))
    }
    .foregroundStyle(MetasequoiaTheme.accent)
    .padding(.horizontal, 12)
    .padding(.vertical, 8)
    .background(MetasequoiaTheme.accentSoft, in: Capsule())
    .accessibilityElement(children: .combine)
    .accessibilityIdentifier("voiceSessionBackHint")
  }

  /// 和键盘聆听界面的圆球同一个样子：强调色的圆，里面是麦克风。失败时换成次要色和感叹号。
  private var orb: some View {
    ZStack {
      Circle()
        .fill(isFailed ? MetasequoiaTheme.sub.opacity(0.25) : MetasequoiaTheme.accent)
        .frame(width: 88, height: 88)
      if service.status == .starting {
        ProgressView().tint(MetasequoiaTheme.onAccent).controlSize(.large)
      } else {
        Image(systemName: isFailed ? "exclamationmark" : "mic.fill")
          .font(.system(size: 36, weight: .semibold))
          .foregroundStyle(isFailed ? MetasequoiaTheme.sub : MetasequoiaTheme.onAccent)
      }
    }
    .accessibilityHidden(true)
  }

  private var isFailed: Bool {
    if case .failed = service.status { return true }
    return false
  }

  private var title: String {
    switch service.status {
    case .starting: "正在开启语音"
    case .ready: "语音已开启"
    case .failed: "语音没有开启"
    case .inactive: "语音已关闭"
    }
  }

  private var detail: String {
    switch service.status {
    case .starting: "第一次使用时系统会询问麦克风权限。"
    case .ready: "回去以后在键盘上按住空格说话，\n松开上屏，上滑取消。"
    case .failed(let message): message
    case .inactive: "在键盘上长按空格会再次开启。"
    }
  }

  @ViewBuilder private var actions: some View {
    switch service.status {
    case .failed:
      Button { Task { await service.activate() } } label: {
        Text("重试").font(.system(size: 16, weight: .semibold)).frame(minWidth: 120).padding(.vertical, 4)
      }
      .buttonStyle(.borderedProminent)
      .tint(MetasequoiaTheme.accent)
      Button("打开系统设置") {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
      }
      .font(.system(size: 15))
      .foregroundStyle(MetasequoiaTheme.accent)
    case .inactive:
      Button { Task { await service.activate() } } label: {
        Text("重新开启").font(.system(size: 16, weight: .semibold)).frame(minWidth: 120).padding(.vertical, 4)
      }
      .buttonStyle(.borderedProminent)
      .tint(MetasequoiaTheme.accent)
    case .starting, .ready:
      EmptyView()
    }
  }

  private var footer: some View {
    VStack(spacing: 10) {
      Text("\(Int(VoiceSessionChannel.idleLifetime / 60)) 分钟不用会自动关闭。开着时状态栏显示麦克风标记，只有按住空格的那一段才录音识别，其余声音直接丢弃。")
        .designSubtext()
        .multilineTextAlignment(.center)
      if service.status == .ready {
        Button("现在关闭语音") { service.deactivate() }
          .font(.system(size: 13))
          .foregroundStyle(MetasequoiaTheme.sub)
          .underline()
          .accessibilityIdentifier("voiceSessionClose")
      }
    }
    .padding(.horizontal, 32)
    .padding(.bottom, 16)
  }
}
