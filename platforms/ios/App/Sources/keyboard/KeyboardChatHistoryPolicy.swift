import Foundation

struct KeyboardChatHistoryPolicy {
  static let maxMessages = 16
  static let maxContentBytes = 64 * 1024

  static func bounded(_ messages: [(role: String, text: String)]) -> [(role: String, text: String)] {
    var result: [(role: String, text: String)] = []
    var bytes = 0
    for message in messages.reversed() {
      let count = message.text.utf8.count
      guard result.count < maxMessages, bytes + count <= maxContentBytes else { break }
      result.insert(message, at: 0)
      bytes += count
    }
    return result
  }
}
