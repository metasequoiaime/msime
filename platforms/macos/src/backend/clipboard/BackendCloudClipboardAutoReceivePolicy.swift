import Foundation

/// 自动接收只处理云端的新条目，不覆盖用户刚复制的内容，也不重复接收同一时间戳的条目。
struct MacCloudClipboardReceivePolicy {
  private(set) var latestRemoteDate = Date.distantPast
  private(set) var localText = ""
  private(set) var receiverOwned = false
  private var needsBaseline = true
  private let formatter = ISO8601DateFormatter()

  mutating func reset() {
    latestRemoteDate = .distantPast
    localText = ""
    receiverOwned = false
    needsBaseline = true
  }

  mutating func begin(text: String) {
    localText = text
    receiverOwned = false
    needsBaseline = !text.isEmpty
  }

  mutating func localChanged(text: String) {
    localText = text
    receiverOwned = false
    needsBaseline = true
  }

  mutating func received(text: String, updatedAt: String) {
    localText = text
    receiverOwned = true
    needsBaseline = false
    markRemote(updatedAt: updatedAt)
  }

  mutating func markRemote(updatedAt: String) {
    if let date = date(updatedAt), date > latestRemoteDate { latestRemoteDate = date }
  }

  mutating func emptyPage(currentText: String) {
    if currentText != localText { localChanged(text: currentText) }
    needsBaseline = false
  }

  private func date(_ raw: String) -> Date? {
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    if let date = formatter.date(from: raw) { return date }
    formatter.formatOptions = [.withInternetDateTime]
    return formatter.date(from: raw)
  }

  mutating func receive(text: String, updatedAt: String, currentText: String) -> Bool {
    if currentText != localText { localChanged(text: currentText) }
    guard let date = date(updatedAt) else { return false }
    if needsBaseline {
      markRemote(updatedAt: updatedAt)
      needsBaseline = false
      return false
    }
    guard date > latestRemoteDate else { return false }
    if currentText == text {
      markRemote(updatedAt: updatedAt)
      return false
    }
    return true
  }
}
