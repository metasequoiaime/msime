import Foundation
import Darwin

enum CommunityResourceKind: String, Codable, CaseIterable, Identifiable, Sendable {
  case dictionary, reply
  var id: String { rawValue }
  var title: String { self == .dictionary ? "词库" : "回复模板" }
  var icon: String { self == .dictionary ? "character.book.closed.fill" : "text.bubble.fill" }
}
struct CommunityWord: Codable, Sendable {
  var kind: String
  var code: String
  var word: String
  var weight: Int64
  init(_ value: PersonalWord) {
    kind = value.kind == .quickPhrase ? "quick" : value.kind.rawValue
    code = value.key; word = value.value; weight = value.weight
  }
  func localWord() throws -> PersonalWord {
    guard let kind = PersonalWordKind(rawValue: kind == "quick" ? "quickPhrase" : kind) else {
      throw PersonalDictionaryStore.StoreError.invalidState
    }
    return try PersonalWord(kind: kind, key: code, value: word, weight: weight).validated()
  }
}
struct CommunityResourceContent: Codable, Sendable {
  var entries: [CommunityWord]?
  var prompt: String?
}
struct CommunityResource: Codable, Identifiable, Sendable {
  var id: String
  var kind: CommunityResourceKind
  var name: String
  var description: String
  var author: String
  var content: CommunityResourceContent
  var revision: Int
  var saves: Int
  var saved: Bool
  var owned: Bool
  var rating_count: Int
  var rating_average: Double
  var my_rating: Int
  /// "approved", "pending" or "removed" on the user's own works when the request asked for fields=moderation.
  var moderation: String? = nil
  /// 作品最后修改的时间（RFC 3339），用于社区列表行上的本周更新标注。资源 API 不保证返回这个字段，缺失时解码为 nil，标注也就不显示，与 Android 一致。
  var updated_at: String? = nil
  /// Post-moderation: only a removal is shown to the author, never a pending state or a reason.
  var removed: Bool { owned && moderation == "removed" }
}

// Only explicit downloads are shared with the keyboard; never credentials or source messages.
enum CommunityLibrary {
  private static let maximumBytes = 4_000_000
  private static let maximumItems = 50
  private static let processLock = NSLock()

  private static func rejectSymlinkAncestors(_ path: URL) throws {
    guard !SafePath.hasRefusedSymbolicLink(path) else { throw PersonalDictionaryStore.StoreError.unavailable }
  }

  private static func resolvedDirectory(_ directory: URL?) -> URL? {
    directory ?? FileManager.default.containerURL(
      forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier)
  }

  private static func file(in directory: URL) -> URL {
    directory.appendingPathComponent("CommunityLibrary.json")
  }

  private static func withLock<T>(in directory: URL?, operation: (URL) throws -> T) throws -> T {
    guard let directory = resolvedDirectory(directory) else { throw PersonalDictionaryStore.StoreError.unavailable }
    processLock.lock()
    defer { processLock.unlock() }
    try rejectSymlinkAncestors(directory)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let descriptor = open(directory.appendingPathComponent("community.lock").path,
                          O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw PersonalDictionaryStore.StoreError.unavailable }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else { throw PersonalDictionaryStore.StoreError.busy }
    defer { flock(descriptor, LOCK_UN) }
    return try operation(directory)
  }

  private static func readUnlocked(in directory: URL) throws -> [CommunityResource] {
    let file = file(in: directory)
    try rejectSymlinkAncestors(file)
    guard FileManager.default.fileExists(atPath: file.path) else { return [] }
    guard let size = try file.resourceValues(forKeys: [.fileSizeKey]).fileSize, size <= maximumBytes else {
      throw PersonalDictionaryStore.StoreError.invalidState
    }
    let data: Data
    do {
      data = try BoundedFileReader.read(from: file, maximumBytes: maximumBytes)
    } catch {
      throw PersonalDictionaryStore.StoreError.invalidState
    }
    let items = try JSONDecoder().decode([CommunityResource].self, from: data)
    guard items.count <= maximumItems,
          Set(items.map(\.id)).count == items.count,
          items.allSatisfy(valid) else {
      throw PersonalDictionaryStore.StoreError.invalidState
    }
    return items
  }

  static func read(in directory: URL? = nil) throws -> [CommunityResource] {
    try withLock(in: directory) { try readUnlocked(in: $0) }
  }

  static func save(_ item: CommunityResource, in directory: URL? = nil) throws {
    guard valid(item) else { throw PersonalDictionaryStore.StoreError.invalidState }
    try withLock(in: directory) { directory in
      var items = try readUnlocked(in: directory)
      items.removeAll { $0.id == item.id }
      guard items.count < maximumItems else { throw PersonalDictionaryStore.StoreError.tooManyRequests }
      items.append(item)
      try write(items, in: directory)
    }
  }

  static func remove(_ id: String, in directory: URL? = nil) throws {
    try withLock(in: directory) { directory in
      try write(readUnlocked(in: directory).filter { $0.id != id }, in: directory)
    }
  }

  private static func write(_ items: [CommunityResource], in directory: URL) throws {
    let file = file(in: directory)
    let data = try JSONEncoder().encode(items)
    guard data.count <= maximumBytes else { throw PersonalDictionaryStore.StoreError.tooManyRequests }
    try rejectSymlinkAncestors(file)
    try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    try data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
  }
  static func replies(in directory: URL? = nil) -> [CommunityResource] {
    ((try? read(in: directory)) ?? []).filter { $0.kind == .reply }
  }
  static var replies: [CommunityResource] { replies() }

  private static func valid(_ item: CommunityResource) -> Bool {
    guard CommunityValidation.validID(item.id), item.revision > 0,
          CommunityValidation.validText(item.name, minimum: 1, maximum: 32, multiline: false, trimmed: true),
          CommunityValidation.validText(item.description, minimum: 0, maximum: 280, multiline: true),
          CommunityValidation.validText(item.author, minimum: 1, maximum: 128, multiline: false, trimmed: true),
          item.saves >= 0, item.saves <= CommunityValidation.maximumJavaScriptInteger,
          CommunityValidation.validRating(count: item.rating_count, average: item.rating_average, mine: item.my_rating)
    else { return false }
    switch item.kind {
    case .reply:
      return (item.content.entries ?? []).isEmpty
        && item.content.prompt.map { CommunityValidation.validText($0, minimum: 1, maximum: 2_000, multiline: true) } == true
    case .dictionary:
      guard item.content.prompt == nil, let entries = item.content.entries,
            (1...128).contains(entries.count) else { return false }
      var seen = Set<String>()
      return entries.allSatisfy { entry in
        CommunityValidation.validText(entry.code, minimum: 1, maximum: 256, multiline: false)
          && CommunityValidation.validText(entry.word, minimum: 1, maximum: 1_024, multiline: false)
          && entry.weight >= 0
          && seen.insert("\(entry.kind)|\(entry.code)|\(entry.word)").inserted
      }
    }
  }

}

/// 本机排队导入过的社区词库 id，社区列表和词库页的「已添加」胶囊据此显示，与 Android 按本机命名词库的 `resourceId` 判断一致。iOS 的个人词库不记词条来自哪份社区词库，所以在排队导入成功时另记一笔；服务器上的收藏（`saved`）只表示关注更新，不代表本机装过。只有设置 App 读写它，所以存在 App 自己的 defaults 里。
enum CommunityDictionaryImports {
  static let key = "community.dictionary.imported"
  /// 只留最近的这么多条，社区词库再多也不会让这份记录无限长大。
  static let limit = 500

  static func ids(in defaults: UserDefaults = .standard) -> Set<String> {
    Set(defaults.stringArray(forKey: key) ?? [])
  }

  static func record(_ id: String, in defaults: UserDefaults = .standard) {
    var stored = (defaults.stringArray(forKey: key) ?? []).filter { $0 != id }
    stored.append(id)
    defaults.set(Array(stored.suffix(limit)), forKey: key)
  }
}
