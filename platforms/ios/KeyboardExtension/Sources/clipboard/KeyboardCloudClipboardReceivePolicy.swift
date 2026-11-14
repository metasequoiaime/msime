import Foundation

/// 自动接收云剪贴板的状态机：只接受比上次看到的新条目，不覆盖用户刚复制的内容。
struct KeyboardCloudClipboardReceivePolicy {
  private(set) var latestRemoteDate = Date.distantPast
  private(set) var localChangeCount: Int?
  private var needsBaseline = true

  mutating func begin(changeCount: Int, hasItems: Bool) {
    latestRemoteDate = .distantPast
    localChangeCount = changeCount
    needsBaseline = hasItems
  }

  mutating func localChanged(changeCount: Int) {
    localChangeCount = changeCount
    needsBaseline = true
  }

  mutating func received(changeCount: Int) {
    localChangeCount = changeCount
    needsBaseline = false
  }

  mutating func markRemote(_ item: BackendAccountClient.ClipboardItem) {
    if let date = CloudClipboardTimePolicy.date(item.updated_at), date > latestRemoteDate {
      latestRemoteDate = date
    }
  }

  mutating func receive(_ item: BackendAccountClient.ClipboardItem?, changeCount: Int) -> Bool {
    if localChangeCount != changeCount { localChanged(changeCount: changeCount) }
    if needsBaseline {
      // 第一页只建立水位；之后的新远端复制仍然可达，本机复制不再永久阻止接收。
      if let item {
        guard CloudClipboardTimePolicy.date(item.updated_at) != nil else { return false }
        markRemote(item)
      }
      needsBaseline = false
      return false
    }
    guard let item, let date = CloudClipboardTimePolicy.date(item.updated_at),
          date > latestRemoteDate,
          localChangeCount == changeCount else {
      return false
    }
    return true
  }
}
