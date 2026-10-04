import Foundation
import CoreFoundation
import Darwin

// The Tauri host reaches the same App Group queue as the native keyboard
// without exposing paths to WebView input. The payload is one already-bounded
// dictionary action; responses never include diagnostics from the Engine.
enum TauriPersonalDictionaryBridge {
  static let maximumRequestBytes = 1_200_000

  static func request(_ data: Data, store: PersonalDictionaryStore = PersonalDictionaryStore()) throws -> [String: Any] {
    guard data.count <= maximumRequestBytes,
          let action = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          let operation = action["operation"] as? String else {
      throw Failure.invalid
    }
    switch operation {
    case "list":
      let offset = try boundedInteger(action["offset"], range: 0...1_000_000)
      _ = try boundedInteger(action["limit"], range: 1...1_000)
      // The kind and code prefix go to the keyboard with the page, which answers them from the user's whole store.
      let kind = try (action["kind"] as? String).map {
        guard let kind = PersonalWordKind(bridgeName: $0) else { throw Failure.invalid }
        return kind
      }
      let query = action["query"] as? String ?? ""
      guard query.utf8.count <= PersonalDictionaryStore.maximumQueryBytes else { throw Failure.invalid }
      try store.requestPage(offset: offset, kind: kind, query: query)
      return response(try store.read())
    case "edit":
      let requestID = try requestID(action)
      // The previous entry is a stored row that only names what to edit or delete; it may carry a quick phrase code with a digit that new input may no longer use, so it skips the new-entry validation.
      let previous = try storedWord(action["previous"])
      let replacement = try word(action["replacement"])
      guard previous != nil || replacement != nil else { throw Failure.invalid }
      _ = try store.enqueue(previous: previous, replacement: replacement, requestID: requestID)
      return ["queued": true, "pending_count": try store.read().pendingCount]
    case "import":
      // A file in one of the shared formats. The Engine route would need the maintenance lock the keyboard holds while typing, so the words are parsed here and queued like any other import.
      let requestID = try requestID(action)
      guard let kind = action["kind"] as? String, let format = action["format"] as? String,
            let text = action["text"] as? String else { throw Failure.invalid }
      let parsed = try PersonalDictionaryBridge.importEntries(kind: kind, format: format, text: text)
      try store.enqueueImport(parsed.entries, requestID: requestID)
      var result = parsed.report
      result["queued"] = true
      result["pending_count"] = try store.read().pendingCount
      return result
    case "import_personal":
      let requestID = try requestID(action)
      guard let text = action["text"] as? String,
            let data = text.data(using: .utf8), data.count <= PersonalDictionaryImport.maximumBytes else {
        throw Failure.invalid
      }
      let file = try PersonalDictionaryImport.decode(data)
      try store.enqueueImport(file.entries, requestID: requestID)
      return ["queued": true, "pending_count": try store.read().pendingCount]
    case "export":
      return try export(action, state: store.read())
    case "retry":
      let requestID = try requestID(action)
      try store.retry(requestID)
      return ["pending_count": try store.read().pendingCount]
    case "dismiss_failure":
      let requestID = try requestID(action)
      try store.dismissFailure(requestID)
      return ["pending_count": try store.read().pendingCount]
    default:
      throw Failure.invalid
    }
  }

  private static func response(_ state: PersonalDictionaryState) -> [String: Any] {
    let failures = state.requests.filter { $0.status == .failed }.map { request in
      ["request_id": request.id,
       "label": (request.replacement ?? request.previous)?.value ?? "词条",
       "error": request.error ?? "同步失败"]
    }
    return [
      "entries": state.entries.map(\.bridgeValue),
      "has_more": state.hasMore,
      "pending_count": state.pendingCount,
      "failed_requests": failures,
      "snapshot_error": state.snapshotError ?? NSNull(),
      "page_offset": state.pageOffset,
      "requested_page_offset": state.requestedPageOffset,
      "page_kind": state.pageKind?.bridgeName ?? NSNull(),
      "page_query": state.pageQuery,
    ]
  }

  private static func export(_ action: [String: Any], state: PersonalDictionaryState) throws -> [String: Any] {
    guard let rawKind = action["kind"] as? String,
          let kind = PersonalWordKind(rawValue: rawKind == "quick_phrase" ? "quickPhrase" : rawKind),
          let format = action["format"] as? String,
          format == "standard" || format == "windows" else { throw Failure.invalid }
    let offset = try boundedInteger(action["offset"], range: 0...1_000_000)
    let limit = try boundedInteger(action["limit"], range: 1...1_000)
    // The export holds the user's own words; a code search may have left bundled rows on the page.
    let matching = state.entries.filter { $0.kind == kind && !$0.isBundled }
    let page = matching.dropFirst(min(offset, matching.count)).prefix(limit)
    var text = page.map { word in
      format == "windows"
        ? "\(word.key)\t\(word.value)\t\(word.weight)"
        : "\(word.value)\t\(word.key)\t\(word.weight)"
    }.joined(separator: "\n")
    if !text.isEmpty { text += "\n" }
    return ["text": text, "has_more": matching.count > offset + limit]
  }

  private static func word(_ value: Any?) throws -> PersonalWord? {
    guard let word = try storedWord(value) else { return nil }
    // A bundled row goes back as listed; the keyboard's Engine refuses anything but a new weight for it.
    return word.isBundled ? word : try word.validated()
  }

  private static func storedWord(_ value: Any?) throws -> PersonalWord? {
    if value == nil || value is NSNull { return nil }
    guard let fields = value as? [String: Any] else { throw Failure.invalid }
    return try PersonalWord(bridgeValue: fields)
  }

  private static func requestID(_ action: [String: Any]) throws -> String {
    guard let value = action["request_id"] as? String, !value.isEmpty, value.utf8.count <= 120,
          value.utf8.allSatisfy({
            ($0 >= 48 && $0 <= 57) || ($0 >= 65 && $0 <= 90) ||
              ($0 >= 97 && $0 <= 122) || $0 == 45 || $0 == 95
          })
    else { throw Failure.invalid }
    return value
  }

  private static func boundedInteger(_ value: Any?, range: ClosedRange<Int>) throws -> Int {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID() else { throw Failure.invalid }
    let integer = number.intValue
    guard NSNumber(value: integer) == number, range.contains(integer) else { throw Failure.invalid }
    return integer
  }

  enum Failure: Error { case invalid }
}

private func tauriPersonalDictionaryError(_ error: Error) -> String {
  switch error {
  case PersonalDictionaryStore.StoreError.busy: return "dictionary_busy"
  case PersonalDictionaryStore.StoreError.tooManyRequests: return "dictionary_too_many"
  case PersonalDictionaryStore.StoreError.conflict: return "dictionary_conflict"
  case PersonalDictionaryStore.StoreError.invalidState, PersonalDictionaryStore.StoreError.unavailable,
       DictionaryFileImportFailure.unavailable:
    return "dictionary_unavailable"
  default: return "dictionary_import_rejected"
  }
}

private func tauriPersonalDictionaryResponse(_ document: [String: Any]) -> UnsafeMutablePointer<CChar>? {
  guard JSONSerialization.isValidJSONObject(document),
        let data = try? JSONSerialization.data(withJSONObject: document),
        data.count <= 8 * 1024 * 1024,
        let text = String(data: data, encoding: .utf8) else { return nil }
  return text.withCString { strdup($0) }
}

@_cdecl("msime_ios_personal_dictionary_request")
func msimeIOSPersonalDictionaryRequest(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  guard let request, length <= UInt(TauriPersonalDictionaryBridge.maximumRequestBytes) else { return nil }
  do {
    let value = try TauriPersonalDictionaryBridge.request(Data(bytes: request, count: Int(length)))
    return tauriPersonalDictionaryResponse(["ok": true, "value": value])
  } catch {
    return tauriPersonalDictionaryResponse(["ok": false, "error": tauriPersonalDictionaryError(error)])
  }
}

@_cdecl("msime_ios_personal_dictionary_string_free")
func msimeIOSPersonalDictionaryStringFree(_ value: UnsafeMutablePointer<CChar>?) {
  free(value)
}
