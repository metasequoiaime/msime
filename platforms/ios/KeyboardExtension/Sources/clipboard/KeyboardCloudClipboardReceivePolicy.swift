import Foundation

/// 自动接收云剪贴板的状态机：只接受比上次看到的新条目，不覆盖用户刚复制的内容。
struct KeyboardCloudClipboardReceivePolicy {
  private(set) var latestRemoteDate = Date.distantPast
  private(set) var localChangeCount: Int?
  private(set) var localHasStrings = false
  private var localOwnedByReceiver = false

  mutating func begin(changeCount: Int, hasStrings: Bool) {
    localChangeCount = changeCount
    localHasStrings = hasStrings
    localOwnedByReceiver = false
  }

  mutating func localChanged(changeCount: Int, hasStrings: Bool) {
    localChangeCount = changeCount
    localHasStrings = hasStrings
    localOwnedByReceiver = false
  }

  mutating func received(changeCount: Int) {
    localChangeCount = changeCount
    localHasStrings = true
    localOwnedByReceiver = true
  }

  mutating func markRemote(_ item: BackendAccountClient.ClipboardItem) {
    if let date = CloudClipboardTimePolicy.date(item.updated_at), date > latestRemoteDate {
      latestRemoteDate = date
    }
  }

  mutating func receive(_ item: BackendAccountClient.ClipboardItem,
                        changeCount: Int,
                        hasStrings: Bool) -> Bool {
    guard let date = CloudClipboardTimePolicy.date(item.updated_at),
          date > latestRemoteDate,
          let expected = localChangeCount,
          expected == changeCount,
          (!hasStrings || localOwnedByReceiver),
          (!localHasStrings || localOwnedByReceiver) else {
      return false
    }
    return true
  }
}
