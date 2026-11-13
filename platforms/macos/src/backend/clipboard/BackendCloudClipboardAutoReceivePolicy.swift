import Foundation

/// 自动接收只处理云端的新条目，不覆盖用户刚复制的内容，也不重复接收同一时间戳的条目。
struct MacCloudClipboardReceivePolicy {
  private(set) var latestRemoteDate = Date.distantPast
  private(set) var localText = ""
  private(set) var receiverOwned = false
  private let formatter = ISO8601DateFormatter()

  mutating func reset() {
    latestRemoteDate = .distantPast
    localText = ""
    receiverOwned = false
  }

  mutating func begin(text: String) {
    localText = text
    receiverOwned = false
  }

  mutating func localChanged(text: String) {
    localText = text
    receiverOwned = false
  }

  mutating func received(text: String) {
    localText = text
    receiverOwned = true
  }

  mutating func receive(text: String, updatedAt: String, currentText: String) -> Bool {
    guard let date = formatter.date(from: updatedAt), date > latestRemoteDate else { return false }
    latestRemoteDate = date
    guard currentText.isEmpty || receiverOwned, currentText != text else { return false }
    return true
  }
}
