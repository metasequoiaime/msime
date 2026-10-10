import Foundation
import CoreFoundation

private typealias MSIMEClipboardByte = UInt8

@_silgen_name("msime_client_mobile_clipboard_history")
private func msimeClientMobileClipboardHistory(
  _ request: UnsafePointer<MSIMEClipboardByte>?, _ length: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientClipboardStringFree(_ value: UnsafeMutablePointer<CChar>?)

struct ClipboardHistoryItem: Equatable, Identifiable {
  var id: String { text }
  var text: String
  var date = Date()
  var pinned = false

  /// Windows' clipboard search: a case-insensitive substring match that keeps the stored order, with an empty query showing everything.
  static func matching(_ items: [Self], query: String) -> [Self] {
    guard !query.isEmpty else { return items }
    return items.filter { $0.text.range(of: query, options: .caseInsensitive) != nil }
  }
}

struct ClipboardHistoryStore {
  static let limit = 50
  let root: URL
  let file: URL

  init(directory: URL? = nil) {
    root = directory ?? FileManager.default.containerURL(
      forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier)
      ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    file = root.appendingPathComponent("MSIME/clipboard_history.json")
  }

  func load() throws -> [ClipboardHistoryItem] {
    let value = try call(["operation": "load"])
    guard let rows = value["entries"] as? [[String: Any]], rows.count <= Self.limit else {
      throw Failure.invalidFile
    }
    return try rows.map { row in
      guard let text = row["text"] as? String,
            let rawTimestamp = row["timestampMs"] as? NSNumber,
            let timestamp = Self.strictTimestampMilliseconds(rawTimestamp),
            let pinned = row["pinned"] as? Bool,
            !text.contains("\0")
      else { throw Failure.invalidFile }
      return ClipboardHistoryItem(
        text: text, date: Date(timeIntervalSince1970: Double(timestamp) / 1_000),
        pinned: pinned)
    }
  }

  /// The shared store writes a non-negative JSON `u64`; reject NSNumber's boolean and lossy conversions.
  static func strictTimestampMilliseconds(_ value: NSNumber) -> UInt64? {
    SharedNumber.strictUInt64(value)
  }

  func add(_ text: String) throws {
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
      throw Failure.empty
    }
    let value = try call(["operation": "capture", "text": text])
    guard value["captured"] as? Bool == true else {
      if value["reason"] as? String == "full" { throw Failure.full }
      if value["reason"] as? String == "invalid" { throw Failure.invalidText }
      throw Failure.invalidFile
    }
  }

  func setPinned(_ pinned: Bool, text: String) throws {
    let value = try call(["operation": "set_pinned", "text": text, "pinned": pinned])
    guard value["updated"] as? Bool == true else { throw Failure.stale }
  }

  func remove(text: String) throws {
    let value = try call(["operation": "remove", "text": text])
    guard value["removed"] as? Bool == true else { throw Failure.stale }
  }

  func clear() throws {
    let value = try call(["operation": "clear"])
    guard value["cleared"] as? Bool == true else { throw Failure.invalidFile }
  }

  private func call(_ action: [String: Any]) throws -> [String: Any] {
    let request = try JSONSerialization.data(withJSONObject: [
      "directory": root.path,
      "action": action,
    ])
    let pointer = request.withUnsafeBytes { bytes in
      msimeClientMobileClipboardHistory(
        bytes.bindMemory(to: MSIMEClipboardByte.self).baseAddress, UInt(request.count))
    }
    guard let pointer else { throw Failure.invalidFile }
    let response = String(cString: pointer)
    msimeClientClipboardStringFree(pointer)
    guard let data = response.data(using: .utf8),
          let envelope = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          envelope["ok"] as? Bool == true,
          let value = envelope["value"] as? [String: Any]
    else { throw Failure.invalidFile }
    protectSharedState()
    return value
  }

  private func protectSharedState() {
    let directory = file.deletingLastPathComponent()
    if FileManager.default.fileExists(atPath: directory.path) {
      var protectedDirectory = directory
      var values = URLResourceValues()
      values.isExcludedFromBackup = true
      try? protectedDirectory.setResourceValues(values)
    }
    if FileManager.default.fileExists(atPath: file.path) {
      try? FileManager.default.setAttributes(
        [.protectionKey: FileProtectionType.complete], ofItemAtPath: file.path)
    }
  }

  enum Failure: Error, LocalizedError {
    case empty, invalidText, full, invalidFile, stale
    var errorDescription: String? {
      switch self {
      case .empty: "剪贴板中没有可保存的文本，或尚未允许粘贴。"
      case .invalidText: "剪贴板文本过长或包含不能保存的字符，请缩短或修改后重试。"
      case .full: "50 条历史均已固定，请先取消固定或删除一条。"
      case .invalidFile: "历史记录无法读取；原文件已保留。"
      case .stale: "记录已在其他窗口中更改，请重试。"
      }
    }
  }
}
