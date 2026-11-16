import AppKit

/// 在主线程采样与异步落盘之间保留有界正文；保存重试不再依赖系统仍持有旧内容。
@MainActor final class MacClipboardSamplingBuffer {
  struct Entry {
    let token: UInt64
    let text: String
  }
  static let capacity = 128
  private let capture: MacClipboardCapture
  private var enabled = false
  private var nextToken: UInt64 = 0
  private var entries: [Entry] = []

  init(capture: MacClipboardCapture) { self.capture = capture }
  var first: Entry? { entries.first }

  func setEnabled(_ value: Bool) {
    enabled = value
    if !value {
      entries.removeAll(keepingCapacity: false)
      _ = capture.sample(enabled: false)
    }
  }

  func sample() {
    guard enabled, let sample = capture.sample(enabled: true) else { return }
    // 入队即确认采样身份；落盘失败由队列保留正文，避免重复读当前剪贴板。
    capture.acknowledge(sample)
    nextToken &+= 1
    if entries.count == Self.capacity { entries.removeFirst() }
    entries.append(Entry(token: nextToken, text: sample.text))
  }

  func saved(_ entry: Entry) {
    // 保存期间可能达到上限并移除旧头部，不能误删下一条。
    if entries.first?.token == entry.token { entries.removeFirst() }
  }
}
