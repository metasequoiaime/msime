import Foundation

/// 云同步的纯决策，对应 Android 的 `SyncMergePolicy` 和 `CloudSync.message`：每个分类这一轮该上传、下载、合并还是不动，什么时候该跑，PUT 冲突怎么重试，失败怎么告诉用户。不碰磁盘、钥匙串和界面，Swift 包测试直接覆盖；执行的是 iOS 应用的 `CloudSync`。
enum BackendSyncPolicy {
  /// 一个分类这一轮做什么。
  enum Mode: Equatable, Sendable { case none, upload, download, merge }

  /// 首次开启同步时用户的选择；云端为空时不用问，直接上传。
  enum Choice: Equatable, Sendable { case merge, useCloud }

  /// 两次自动同步之间至少隔这么久。
  static let throttle: TimeInterval = 5 * 60
  /// 本机有改动时两次同步之间至少隔这么久，连续改几项设置只上传一次。
  static let dirtyDebounce: TimeInterval = 30
  /// PUT 回 409 `revision_conflict` 时重新拉取、合并后再试的次数；超过就放到下一轮。
  static let maxConflictRetries = 2
  /// 设备本地且涉及隐私、无论如何都不上传也不应用的设置，按键名里的片段识别。
  static let localOnlyFragments = ["incognito", "developer_options", "diagnostic_log", "contribute_audio"]

  /// 首次开启：云端没有任何数据就直接上传，否则要等用户选「合并」或「使用云端」，返回 nil 表示需要问。
  static func firstRun(cloudHasData: Bool, choice: Choice?) -> Mode? {
    guard cloudHasData else { return .upload }
    guard let choice else { return nil }
    return choice == .merge ? .merge : .download
  }

  /// 之后每一轮：两边都没变不动；只有本机变了上传（本机删掉的条目随之从云端消失）；只有云端变了下载；两边都变了合并，同一项以本机为准。
  static func incremental(cloudChanged: Bool, localDirty: Bool) -> Mode {
    if cloudChanged && localDirty { return .merge }
    if cloudChanged { return .download }
    return localDirty ? .upload : .none
  }

  /// 云端版本是不是和上次同步记下的游标不同；从没记过（空串）也算不同。
  static func cloudChanged(cursor: String, revision: Int64) -> Bool {
    cursor.isEmpty || cursor != String(revision)
  }

  /// 这一次回到前台要不要真的跑：距上次尝试满 `throttle`；有分类带着待上传标记时只要满 `dirtyDebounce`。没跑过或时钟往回拨时当作已到期。
  static func due(now: Date, lastAttempt: Date?, anyDirty: Bool) -> Bool {
    guard let lastAttempt, now >= lastAttempt else { return true }
    let elapsed = now.timeIntervalSince(lastAttempt)
    return elapsed >= throttle || anyDirty && elapsed >= dirtyDebounce
  }

  /// 首次选择的提示要不要再弹：用户刚打开开关时总是弹；否则距上次弹满 `throttle` 才再弹，取消之后不会每次回到前台都追着问。
  static func promptDue(now: Date, promptedAt: Date?, force: Bool) -> Bool {
    guard !force, let promptedAt, now >= promptedAt else { return true }
    return now.timeIntervalSince(promptedAt) >= throttle
  }

  static func isLocalOnly(_ key: String) -> Bool {
    localOnlyFragments.contains { key.contains($0) }
  }

  /// 去掉设备本地且涉及隐私的键（隐私模式、开发者选项、诊断日志、语音贡献）。
  static func withoutLocalOnly<Value>(_ settings: [String: Value]) -> [String: Value] {
    settings.filter { !isLocalOnly($0.key) }
  }

  /// 这次失败是不是 CAS 冲突：别的设备在我们读之后先写了。
  static func isConflict(_ error: Error) -> Bool {
    guard let failure = error as? BackendAccountClient.Failure else { return false }
    return failure.status == 409 && failure.code == "revision_conflict"
  }

  /// 整文档 CAS 的写入：按 `mode` 写一次；回 409 `revision_conflict` 时重新拉取云端、改按合并再写，最多重试 `maxConflictRetries` 次，之后把冲突抛给调用方，留到下一轮。其他失败原样抛出。
  ///
  /// - Parameters:
  ///   - put: 以这份云端为底按这个模式写入，返回写入后的结果
  ///   - refetch: 重新读一次云端
  static func resolvingConflicts<Cloud, Value>(_ cloud: Cloud, mode: Mode,
                                               put: (Cloud, Mode) async throws -> Value,
                                               refetch: () async throws -> Cloud) async throws -> Value {
    var cloud = cloud
    var mode = mode
    var attempt = 0
    while true {
      do {
        return try await put(cloud, mode)
      } catch let error where isConflict(error) && attempt < maxConflictRetries {
        attempt += 1
        cloud = try await refetch()
        mode = .merge
      }
    }
  }

  // ---- 常用语 ----

  /// 常用语接口的条数上限。
  static let maxPhrases = 500
  /// 常用语接口每条正文的 UTF-16 单元上限。
  static let maxPhraseUnits = 2000

  /// 云端常用语接口里的一条：`group` 可为空串，`position` 从 0 数。
  struct Phrase: Equatable, Sendable {
    var id: String
    var text: String
    var group: String = ""
    var position: Int = 0
  }

  /// 合并两份常用语：先按 id，`preferred` 里的同 id 项胜出；再按正文去重，保留先出现的；最后重排 position 并截到 `maxPhrases` 条。正文为空或超长的项丢掉。
  ///
  /// - Parameters:
  ///   - preferred: 后写的一方（本机这次的列表），排在前面
  ///   - other: 另一方（云端），只补上 `preferred` 里没有的
  static func mergePhrases(_ preferred: [Phrase], _ other: [Phrase]) -> [Phrase] {
    var seen = Set<String>()
    var merged: [Phrase] = []
    for phrase in preferred + other where usable(phrase) && seen.insert(phrase.id).inserted {
      merged.append(phrase)
    }
    return normalized(merged)
  }

  /// 按正文去重、重排 position 并截断；上传前对任何一份列表都要过这一步。
  static func normalized(_ phrases: [Phrase]) -> [Phrase] {
    var texts = Set<String>()
    var result: [Phrase] = []
    for phrase in phrases where usable(phrase) && texts.insert(phrase.text).inserted {
      guard result.count < maxPhrases else { break }
      result.append(Phrase(id: phrase.id, text: phrase.text, group: phrase.group, position: result.count))
    }
    return result
  }

  /// 只有本机改动时要上传的整份常用语：本机列表在前，再接上云端里正文属于 `unheld` 的那些。
  ///
  /// `unheld` 是上次应用云端结果时本机收不下的正文（本机自己添加的上限比服务端小）。它们从没进过本机列表，所以不会被当成本机删除而从云端抹掉；本机真正删掉的常用语从来不在 `unheld` 里，照常随这次上传从云端消失。
  static func uploadPhrases(_ local: [Phrase], _ cloud: [Phrase], unheld: Set<String>) -> [Phrase] {
    normalized(local + cloud.filter { unheld.contains($0.text) })
  }

  /// 本机要新增的正文和要删掉的本机 id：`target` 是合并或云端的结果，`local` 是本机现有的（id, 正文）。
  ///
  /// 本机 id 由 client-core 生成，和云端 id 不通用，所以按正文对齐：`target` 里有、本机没有的正文要新增；本机有、`target` 里没有的要删掉（只有「使用云端」和下载才会走到删除）。
  static func localPlan(_ local: [(id: String, text: String)], target: [Phrase]) -> (add: [String], remove: [String]) {
    let wanted = Set(target.map(\.text))
    let present = Set(local.map(\.text))
    return (target.map(\.text).filter { !present.contains($0) }, local.filter { !wanted.contains($0.text) }.map(\.id))
  }

  /// 本机列表换成云端格式；同一正文在云端有分组时沿用云端的分组。
  static func asCloudPhrases(_ local: [(id: String, text: String)], cloud: [Phrase]) -> [Phrase] {
    var groups: [String: String] = [:]
    for phrase in cloud where groups[phrase.text] == nil { groups[phrase.text] = phrase.group }
    return local.enumerated().map { index, phrase in
      Phrase(id: phrase.id, text: phrase.text, group: groups[phrase.text] ?? "", position: index)
    }
  }

  private static func usable(_ phrase: Phrase) -> Bool {
    !phrase.id.isEmpty && !phrase.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      && phrase.text.utf16.count <= maxPhraseUnits && !phrase.text.contains("\0")
  }

  static let networkMessage = "网络不可用，稍后自动重试"

  /// 一个分类失败时展示给用户的原因。nil 表示这一轮作废（任务取消，或跑的过程中退出登录、换了账号），不写进状态。不是网络或云端的失败（本机读写出错）用 `fallback`。
  static func failureMessage(_ error: Error, fallback: String) -> String? {
    if error is CancellationError { return nil }
    if let error = error as? URLError { return error.code == .cancelled ? nil : networkMessage }
    if let failure = error as? BackendAccountClient.Failure { return message(status: failure.status, code: failure.code) }
    return fallback
  }

  /// 按 msime-cloud 返回的状态码和错误码说清楚是哪一种失败，措辞与 Android 的 `CloudSync.message` 相同；不认识的带上状态码和错误码，方便用户截图反馈。
  ///
  /// 状态码 0 是 iOS 这边拿到了响应却读不出来（不是 HTTP、超出大小、解析失败）；Android 把同一类情况记成 500 `invalid_response`，这里同样按云端繁忙说，只是不写一个并不存在的状态码。
  static func message(status: Int, code: String?) -> String {
    if status == 401 { return "登录已失效，请重新登录后再同步" }
    if status == 503 && (code == "provider_disabled" || code == "service_disabled") { return "云同步暂时不可用" }
    switch code {
    case "invalid_dictionary_snapshot", "invalid_dictionary_entry":
      return "词库里有云端暂不支持的词条，词库没有同步（\(code ?? "")）"
    case "dictionary_limit": return "云端词库最多保存 10 万条，词库没有同步"
    case "snapshot_too_large": return "词库太大，无法同步到云端"
    case "invalid_preference_field", "invalid_preferences": return "有设置项云端不认识，设置没有同步（\(code ?? "")）"
    case "invalid_phrases": return "常用语超出云端限制（最多 500 条，每条最多 2000 字），常用语没有同步"
    case "account_banned": return "账号已被停用，无法同步"
    case "rate_limit_exceeded": return "同步太频繁，稍后自动重试"
    case "snapshot_restore_busy", "engine_unavailable", "auth_unavailable", "dictionary_timeout":
      return "云端暂时繁忙，稍后自动重试"
    default: break
    }
    if status == 0 { return "云端暂时繁忙，稍后自动重试" }
    if status == 429 || status >= 500 { return "云端暂时繁忙，稍后自动重试（HTTP \(status)）" }
    return "同步失败（HTTP \(status)\(code.map { " " + $0 } ?? "")）"
  }

  /// 「我的」页同步行的一行状态，措辞与 Android 的 `AccountFragment.syncLine` 和 `CloudSync.statusLine` 相同：关闭、最近一次失败的原因、云端常用语本机收不下的提示、尚未同步，或「已同步 · 最近 …」。
  static func statusLine(enabled: Bool, error: String?, unheldPhrases: Int, lastSynced: Date?, now: Date) -> String {
    guard enabled else { return "云同步已关闭" }
    if let error, !error.isEmpty { return error }
    if unheldPhrases > 0 { return "云端有 \(unheldPhrases) 条常用语超出本机上限，未下载但已保留" }
    guard let lastSynced else { return "云同步已开启 · 尚未同步" }
    return "已同步 · 最近 " + ago(now: now, then: lastSynced)
  }

  /// 「已同步 · 最近 …」里的相对时间，与 Android 的 `RelativeTimePolicy.minutesAgo` 相同。
  static func ago(now: Date, then: Date) -> String {
    let minutes = Int(max(0, now.timeIntervalSince(then)) / 60)
    if minutes < 1 { return "刚刚" }
    if minutes < 60 { return "\(minutes) 分钟前" }
    let hours = minutes / 60
    if hours < 24 { return "\(hours) 小时前" }
    return "\(hours / 24) 天前"
  }
}
