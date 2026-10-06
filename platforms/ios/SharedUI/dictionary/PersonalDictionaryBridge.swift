import Foundation

private typealias PersonalDictionaryByte = UInt8
@_silgen_name("msime_client_dictionary_validate")
private func msimeClientDictionaryValidate(_ request: UnsafePointer<PersonalDictionaryByte>?,
                                            _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_dictionary_hans_entries")
private func msimeClientDictionaryHansEntries(_ text: UnsafePointer<PersonalDictionaryByte>?, _ textLength: UInt,
                                              _ resources: UnsafePointer<PersonalDictionaryByte>?,
                                              _ resourcesLength: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_dictionary_import_entries")
private func msimeClientDictionaryImportEntries(_ request: UnsafePointer<PersonalDictionaryByte>?, _ requestLength: UInt,
                                                _ resources: UnsafePointer<PersonalDictionaryByte>?,
                                                _ resourcesLength: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientPersonalDictionaryStringFree(_ value: UnsafeMutablePointer<CChar>?)

enum PersonalDictionaryBridge {
  static func validateEntry(_ entry: [String: Any]) throws -> [String: Any] {
    // The native side answers with one internal reason for every rejection, so the guidance has to
    // come from here -- this is the layer that still knows which kind of code the user was typing.
    // Callers pass `PersonalWord.bridgeValue`, which spells the quick phrase kind the shared layer's way (`quick_phrase`), not as the Swift raw value.
    let kind = (entry["kind"] as? String).flatMap(PersonalWordKind.init(bridgeName:))
    var request = entry
    if request["kind"] as? String == "quickPhrase" { request["kind"] = "quick_phrase" }
    guard JSONSerialization.isValidJSONObject(request),
          let data = try? JSONSerialization.data(withJSONObject: request) else {
      throw PersonalDictionaryBridgeFailure.invalid(kind)
    }
    let pointer = data.withUnsafeBytes { bytes in
      msimeClientDictionaryValidate(bytes.bindMemory(to: PersonalDictionaryByte.self).baseAddress,
                                    UInt(data.count))
    }
    guard let pointer else { throw PersonalDictionaryBridgeFailure.invalid(kind) }
    let text = String(cString: pointer)
    msimeClientPersonalDictionaryStringFree(pointer)
    guard let response = text.data(using: .utf8),
          let envelope = try? JSONSerialization.jsonObject(with: response) as? [String: Any],
          envelope["ok"] as? Bool == true,
          let value = envelope["value"] as? [String: Any] else {
      throw PersonalDictionaryBridgeFailure.invalid(kind)
    }
    return value
  }
}

extension PersonalDictionaryBridge {
  /// Plain Chinese words, one per line, as the pinyin entries the shared `hans` import format produces. Nothing is written: the caller shows them for confirmation and queues them like any other import.
  static func hansEntries(_ text: String, resources: URL? = packagedResources) throws -> [PersonalWord] {
    guard let resources else { throw HansImportFailure.dictionaryUnavailable }
    let textData = Data(text.utf8)
    let resourceData = Data(resources.path.utf8)
    let pointer = textData.withUnsafeBytes { textBytes in
      resourceData.withUnsafeBytes { resourceBytes in
        msimeClientDictionaryHansEntries(
          textBytes.bindMemory(to: PersonalDictionaryByte.self).baseAddress, UInt(textData.count),
          resourceBytes.bindMemory(to: PersonalDictionaryByte.self).baseAddress, UInt(resourceData.count))
      }
    }
    guard let pointer else { throw HansImportFailure.dictionaryUnavailable }
    let response = String(cString: pointer)
    msimeClientPersonalDictionaryStringFree(pointer)
    guard let data = response.data(using: .utf8),
          let envelope = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
      throw HansImportFailure.dictionaryUnavailable
    }
    guard envelope["ok"] as? Bool == true else {
      throw envelope["error"] as? String == "invalid dictionary import"
        ? HansImportFailure.invalidText : HansImportFailure.dictionaryUnavailable
    }
    guard let value = envelope["value"] as? [String: Any],
          let rows = value["entries"] as? [[String: Any]] else { throw HansImportFailure.dictionaryUnavailable }
    return try rows.map(PersonalWord.init(bridgeValue:))
  }

  /// A dictionary file in one of the shared import formats (`standard`, `windows`, `rime`, `hans`) as the words the queue accepts, with the import report the settings page shows: `applied`, `failed`, `truncated`, `swapped` and `first_failures`. The shared parser counts the rows the Engine would refuse, queues a repeated word once and stops at the queue's 128 words, so a real file imports here the way it does on a desktop. Nothing is written.
  static func importEntries(kind: String, format: String, text: String,
                            resources: URL? = packagedResources) throws -> (entries: [PersonalWord], report: [String: Any]) {
    guard let resources else { throw DictionaryFileImportFailure.unavailable }
    let request = try JSONSerialization.data(withJSONObject: ["kind": kind, "format": format, "text": text])
    let resourceData = Data(resources.path.utf8)
    let pointer = request.withUnsafeBytes { requestBytes in
      resourceData.withUnsafeBytes { resourceBytes in
        msimeClientDictionaryImportEntries(
          requestBytes.bindMemory(to: PersonalDictionaryByte.self).baseAddress, UInt(request.count),
          resourceBytes.bindMemory(to: PersonalDictionaryByte.self).baseAddress, UInt(resourceData.count))
      }
    }
    guard let pointer else { throw DictionaryFileImportFailure.unavailable }
    let response = String(cString: pointer)
    msimeClientPersonalDictionaryStringFree(pointer)
    guard let data = response.data(using: .utf8),
          let envelope = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
      throw DictionaryFileImportFailure.unavailable
    }
    guard envelope["ok"] as? Bool == true else {
      // A reading the packaged dictionary cannot give is the device's problem, not the file's.
      throw envelope["error"] as? String == "dictionary pinyin unavailable"
        ? DictionaryFileImportFailure.unavailable : DictionaryFileImportFailure.rejected
    }
    guard var report = envelope["value"] as? [String: Any],
          let rows = report.removeValue(forKey: "entries") as? [[String: Any]] else {
      throw DictionaryFileImportFailure.unavailable
    }
    return (try rows.map(PersonalWord.init(bridgeValue:)), report)
  }

  /// The verified Engine resources this process can read. The keyboard test host carries them itself; the settings app reads them from the keyboard extension it embeds, which is where the packaged dictionary ships.
  static var packagedResources: URL? {
    let fm = FileManager.default
    let own = Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true)
    let plugins = (Bundle.main.builtInPlugInsURL).flatMap {
      try? fm.contentsOfDirectory(at: $0, includingPropertiesForKeys: nil)
    }?.filter { $0.pathExtension == "appex" }
      .map { $0.appendingPathComponent("EngineResources", isDirectory: true) } ?? []
    return ([own].compactMap { $0 } + plugins).first {
      fm.isReadableFile(atPath: $0.appendingPathComponent("msime-pinyin.db").path)
    }
  }
}

enum HansImportFailure: LocalizedError, Equatable {
  case invalidText, dictionaryUnavailable

  var errorDescription: String? {
    switch self {
    case .invalidText: "每行填写一个只含汉字的词语，最多 1000 行；以 # 开头的行会被跳过。"
    case .dictionaryUnavailable: "无法为这些词语注音：有的字不在词典中，或键盘词典暂时无法读取。"
    }
  }
}

/// Why a dictionary file could not be queued: nothing in it was usable, or the packaged dictionary could not be read.
enum DictionaryFileImportFailure: Error, Equatable {
  case rejected, unavailable
}

private enum PersonalDictionaryBridgeFailure: LocalizedError {
  case invalid(PersonalWordKind?)

  /// Says what to type instead. "格式无效" alone leaves the user to guess which of the code, the
  /// word or the separators the engine objected to.
  var errorDescription: String? {
    switch self {
    case .invalid(.pinyin): "请填写完整拼音，用空格或英文单引号分隔音节，例如 ni hao。"
    case .invalid(.wubi), .invalid(.wubi98): "五笔编码使用 1–4 个字母。"
    case .invalid(.quickPhrase): "快捷短语编码只能包含英文字母，长度 1 到 32。"
    case .invalid(.english): "英文编码只能包含字母、连字符和撇号。"
    case .invalid(nil): "个人词条格式无效。"
    }
  }
}
