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
  /// Post-moderation: only a removal is shown to the author, never a pending state or a reason.
  var removed: Bool { owned && moderation == "removed" }
}

// Only explicit downloads are shared with the keyboard; never credentials or source messages.
enum CommunityLibrary {
  private static let maximumBytes = 4_000_000
  private static let maximumItems = 50
  private static let maximumJavaScriptInteger = 9_007_199_254_740_991

  private static func rejectSymlinkAncestors(_ path: URL) throws {
    var current = path.standardizedFileURL
    while true {
      if current.path == "/" || current.path == "/var" || current.path == "/tmp" { break }
      var status = stat()
      if lstat(current.path, &status) == 0 {
        guard status.st_mode & S_IFMT != S_IFLNK else { throw PersonalDictionaryStore.StoreError.unavailable }
      } else if errno != ENOENT {
        throw PersonalDictionaryStore.StoreError.unavailable
      }
      let parent = current.deletingLastPathComponent()
      if parent == current { break }
      current = parent
    }
  }

  private static func file(in directory: URL? = nil) -> URL? {
    (directory ?? FileManager.default.containerURL(
      forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier))?
      .appendingPathComponent("CommunityLibrary.json")
  }
  static func read(in directory: URL? = nil) throws -> [CommunityResource] {
    guard let file = file(in: directory) else { throw PersonalDictionaryStore.StoreError.unavailable }
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
  static func save(_ item: CommunityResource, in directory: URL? = nil) throws {
    guard valid(item) else { throw PersonalDictionaryStore.StoreError.invalidState }
    var items = try read(in: directory)
    items.removeAll { $0.id == item.id }
    guard items.count < maximumItems else { throw PersonalDictionaryStore.StoreError.tooManyRequests }
    items.append(item)
    try write(items, in: directory)
  }
  static func remove(_ id: String, in directory: URL? = nil) throws {
    try write(read(in: directory).filter { $0.id != id }, in: directory)
  }
  private static func write(_ items: [CommunityResource], in directory: URL?) throws {
    guard let file = file(in: directory) else { throw PersonalDictionaryStore.StoreError.unavailable }
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
    guard let id = UUID(uuidString: item.id),
          id != UUID(uuidString: "00000000-0000-0000-0000-000000000000")!, item.revision > 0,
          validText(item.name, minimum: 1, maximum: 32, multiline: false, trimmed: true),
          validText(item.description, minimum: 0, maximum: 280, multiline: true),
          validText(item.author, minimum: 1, maximum: 128, multiline: false, trimmed: true),
          item.saves >= 0, item.saves <= maximumJavaScriptInteger,
          validRating(count: item.rating_count, average: item.rating_average, mine: item.my_rating)
    else { return false }
    switch item.kind {
    case .reply:
      return (item.content.entries ?? []).isEmpty
        && item.content.prompt.map { validText($0, minimum: 1, maximum: 2_000, multiline: true) } == true
    case .dictionary:
      guard item.content.prompt == nil, let entries = item.content.entries,
            (1...128).contains(entries.count) else { return false }
      var seen = Set<String>()
      return entries.allSatisfy { entry in
        validText(entry.code, minimum: 1, maximum: 256, multiline: false)
          && validText(entry.word, minimum: 1, maximum: 1_024, multiline: false)
          && entry.weight >= 0
          && seen.insert("\(entry.kind)|\(entry.code)|\(entry.word)").inserted
      }
    }
  }

  private static func validRating(count: Int, average: Double, mine: Int) -> Bool {
    count >= 0 && count <= maximumJavaScriptInteger && (0...5).contains(mine)
      && average.isFinite && (0...5).contains(average)
      && (count != 0 || average == 0)
  }

  private static func validText(_ value: String, minimum: Int, maximum: Int,
                                multiline: Bool, trimmed: Bool = false) -> Bool {
    let scalars = value.unicodeScalars
    guard (minimum...maximum).contains(scalars.count),
          (!trimmed || value == value.trimmingCharacters(in: .whitespacesAndNewlines)) else {
      return false
    }
    return !scalars.contains { scalar in
      CharacterSet.controlCharacters.contains(scalar)
        && !(multiline && (scalar == "\n" || scalar == "\t"))
    }
  }
}
