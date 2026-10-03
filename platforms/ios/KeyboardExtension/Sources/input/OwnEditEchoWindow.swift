import Foundation

/// 认出 `textWillChange` 里键盘自己那次改动的回声。
///
/// UIKit 也会为键盘自己经 `textDocumentProxy` 做的改动回调 `textWillChange`/`textDidChange`，而控制器把这对回调当作宿主改了文档、要结束组字的信号。回调是跨进程送回来的，晚于改动本身，所以不能用「改动期间置位」的标志去认。
///
/// 以前用一个计数器：每次改动加一，每次回调减一。在 iOS 27 模拟器的 SwiftUI 文本框里逐项实测过，回调既不是每次改动一对，也不能按调用次数相加：`insertText`、`deleteBackward`、单独的 `setMarkedText`、清空标记都不回调；`setMarkedText` 接 `unmarkText`（行内预编辑的上屏）和 `adjustTextPosition` 各回调一对，约 5–35ms 后到；同一轮里先 `adjustTextPosition` 再 `setMarkedText` 却一对也没有，同一轮里的几次改动最多合成一对。于是每次上屏加的一都等不来回调，计数一路涨上去，此后用户点到别处、宿主清空输入框都被当成回声吞掉，组字留在原地（实测：一次上屏后计数停在 1 不再归零）。反过来，回调比计数多的宿主上，多出来的那一次会把下一键刚开始的组字结束掉。
///
/// 所以不再数回调，改为按时间认：自己最近一次改动之后 `duration` 以内到达的回调都是回声，过了这段时间到的一律当作外部改动。窗口每次改动都重新开始，不因收到一次回调而关闭——同一轮的改动合成一对、相邻两轮的回调先后到达，都由它覆盖；残留的期待最多存活 `duration`，不会一直吞掉真正的外部改动。
struct OwnEditEchoWindow {
  /// 实测回声最晚约 35ms 到；取十几倍的余量，主线程偶尔卡一下也认得出。真正的外部改动只有紧跟在一次按键之后的这半秒内才会被当成回声，代价是组字没有在那一刻结束、仍留在候选栏里，而不是把用户正在打的字结束掉。
  static let duration: TimeInterval = 0.5

  private var lastOwnEdit: TimeInterval?

  /// 键盘刚经 `textDocumentProxy` 改了文档（插入、删除、标记、取消标记或移动光标）。
  mutating func recordOwnEdit(at now: TimeInterval) {
    lastOwnEdit = now
  }

  /// 这时到达的 `textWillChange` 是否是自己改动的回声。
  func isEcho(at now: TimeInterval) -> Bool {
    guard let lastOwnEdit else { return false }
    return now >= lastOwnEdit && now - lastOwnEdit <= Self.duration
  }

  /// 新的编辑会话不欠任何回调。
  mutating func reset() {
    lastOwnEdit = nil
  }
}
