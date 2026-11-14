import Foundation

/// 变化通知只采样一次；落盘失败保留原文，不在重试时改读最后一条。
@MainActor final class ClipboardHistoryCaptureBuffer {
  static let capacity = 128
  // 与共享 Rust 的移动端字节上限一致，避免重试队列持有系统中任意大的文本。
  static let maximumTextBytes = 100_000
  private struct Entry { let changeCount: Int; let text: String }
  private var lastChangeCount: Int?
  private var pending: [Entry] = []
  var hasPending: Bool { !pending.isEmpty }

  func start(changeCount: Int) {
    pending.removeAll()
    lastChangeCount = changeCount
  }

  func stop() {
    pending.removeAll()
    lastChangeCount = nil
  }

  func capture(changeCount: Int, allowed: Bool, read: () -> String?, save: (Int, String) throws -> Void) {
    guard let previous = lastChangeCount else { return }
    lastChangeCount = changeCount
    guard allowed else { pending.removeAll(); return }
    if previous != changeCount, let text = read(), text.utf8.count <= Self.maximumTextBytes,
       !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
      if pending.count == Self.capacity { pending.removeFirst() }
      pending.append(Entry(changeCount: changeCount, text: text))
    }
    while let entry = pending.first {
      do { try save(entry.changeCount, entry.text) }
      catch { return }
      pending.removeFirst()
    }
  }
}
