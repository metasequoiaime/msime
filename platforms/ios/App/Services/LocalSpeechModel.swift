import Foundation

/// An installed on-device model as its `msime-model.json` describes it: the same manifest and the same rules `shared/voice/LocalAsr.cpp` applies on the desktops, so a model directory means the same thing on every platform.
struct LocalSpeechModelManifest: Equatable {
  enum Kind: String {
    case onlineTransducer = "online_transducer"
    case offlineSenseVoice = "offline_sense_voice"
    case offlineFunAsrNano = "offline_funasr_nano"
  }

  static let fileName = "msime-model.json"
  /// The manifest is a small catalog entry, not a model payload. Keep a damaged or untrusted file from being read without a bound.
  static let maximumManifestBytes = 256 * 1024
  /// Token vocabularies are model data, but only their first field is needed for native hotwords.
  static let maximumTokensBytes = 8 * 1024 * 1024

  let directory: URL
  let kind: Kind
  let files: [String: String]
  let modelingUnit: String
  /// `native` hands the words to the recognizer; `pinyin` corrects the final text through the shared hotword matcher.
  let hotwords: String

  init(directory: URL) throws {
    let url = directory.appendingPathComponent(Self.fileName)
    let invalidManifest = ServiceFailure(message: "本地语音模型的描述文件已损坏，请删除后重新下载。")
    guard let size = try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize else {
      throw ServiceFailure(message: "所选目录不是已安装的本地语音模型。")
    }
    guard size <= Int64(Self.maximumManifestBytes) else { throw invalidManifest }
    guard let handle = try? FileHandle(forReadingFrom: url) else {
      throw ServiceFailure(message: "所选目录不是已安装的本地语音模型。")
    }
    defer { try? handle.close() }
    guard let data = try? handle.read(upToCount: Self.maximumManifestBytes + 1),
          data.count <= Self.maximumManifestBytes else { throw invalidManifest }
    guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let kindName = object["kind"] as? String, let files = object["files"] as? [String: Any]
    else { throw invalidManifest }
    guard let kind = Kind(rawValue: kindName) else { throw ServiceFailure(message: "暂不支持这种本地语音模型（\(kindName)）。") }
    var names: [String: String] = [:]
    for (role, value) in files {
      guard let name = value as? String else { throw invalidManifest }
      names[role] = name
    }
    self.directory = directory
    self.kind = kind
    self.files = names
    modelingUnit = object["modeling_unit"] as? String ?? ""
    hotwords = object["hotwords"] as? String ?? ""
  }

  /// The absolute path of a file the manifest names, which must exist.
  func file(_ role: String) throws -> String {
    guard let name = files[role] else { throw ServiceFailure(message: "本地语音模型缺少 \(role) 文件，请删除后重新下载。") }
    guard let path = Self.containedPath(name, in: directory), FileManager.default.fileExists(atPath: path.path) else {
      throw ServiceFailure(message: "本地语音模型缺少 \(name)，请删除后重新下载。")
    }
    return path.path
  }

  func optionalFile(_ role: String) throws -> String? { files[role] == nil ? nil : try file(role) }

  func textFile(_ role: String, maximumBytes: Int) throws -> String {
    let data = try BoundedFileReader.read(from: URL(fileURLWithPath: try file(role)), maximumBytes: maximumBytes)
    guard let text = String(data: data, encoding: .utf8) else {
      throw ServiceFailure(message: "本地语音模型的 \(role) 文件不是有效的 UTF-8。")
    }
    return text
  }

  static func isModelDirectory(_ url: URL) -> Bool {
    var directory: ObjCBool = false
    return FileManager.default.fileExists(atPath: url.path, isDirectory: &directory) && directory.boolValue
      && FileManager.default.isReadableFile(atPath: url.appendingPathComponent(fileName).path)
  }

  /// Resolve a manifest member and keep symlinks and traversal from escaping the installed model.
  private static func containedPath(_ name: String, in directory: URL) -> URL? {
    let member = URL(fileURLWithPath: name)
    guard member.pathComponents.first != "/", !member.pathComponents.contains("..") else { return nil }
    let root = directory.resolvingSymlinksInPath().standardizedFileURL
    let candidate = directory.appendingPathComponent(name).resolvingSymlinksInPath().standardizedFileURL
    let rootPath = root.path.hasSuffix("/") ? root.path : root.path + "/"
    guard candidate.path != root.path, candidate.path.hasPrefix(rootPath) else { return nil }
    return candidate
  }
}

/// The text rules of the shared local recognizer, kept byte-for-byte in step with `shared/voice/LocalAsr.cpp` so the same audio reads the same on iOS and the desktops.
enum LocalSpeechText {
  /// hardware / 2, clamped to 1...4.
  static var threadCount: Int32 { Int32(min(max(ProcessInfo.processInfo.activeProcessorCount / 2, 1), 4)) }

  /// SenseVoice detects the language itself and handles Mandarin with English words best that way, so only the languages it would not otherwise guess reliably are pinned.
  static func senseVoiceLanguage(_ tag: String) -> String {
    let value = tag.lowercased()
    if value.hasPrefix("yue") || value == "zh-hk" || value == "zh-mo" { return "yue" }
    if value.hasPrefix("ja") { return "ja" }
    if value.hasPrefix("ko") { return "ko" }
    return "auto"
  }

  /// The first field of every line of `tokens.txt`.
  static func tokenSet(_ contents: String) -> Set<String> {
    var tokens = Set<String>()
    contents.enumerateLines { line, _ in
      tokens.insert(line.split(separator: " ", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? "")
    }
    return tokens
  }

  /// sherpa-onnx drops every hotword of a stream when a single one fails to encode, so a word with a character the model has no token for is left out here rather than taking the others with it. ASCII goes through the BPE model, which can spell anything. At most 200 words, one per line.
  static func transducerHotwords(_ words: [String], tokens: Set<String>) -> String {
    var joined = ""
    var kept = 0
    for word in words {
      if kept == 200 { break }
      var clean = ""
      var usable = true
      var previousSpace = true
      for scalar in word.unicodeScalars {
        if scalar.isASCII {
          let byte = UInt8(scalar.value)
          if isSpace(byte) || scalar == "/" {
            if !previousSpace { clean += " " }
            previousSpace = true
            continue
          }
          if !isAlphanumeric(byte) && scalar != "'" && scalar != "-" {
            usable = false
            break
          }
        } else if !tokens.contains(String(scalar)) {
          usable = false
          break
        }
        clean.unicodeScalars.append(scalar)
        previousSpace = false
      }
      while clean.hasSuffix(" ") { clean.removeLast() }
      guard usable, !clean.isEmpty else { continue }
      joined += clean + "\n"
      kept += 1
    }
    return joined
  }

  /// The prompt shares the decoder's 512-token context with the audio; a long list crowds out the speech. At most 30 words, comma-separated.
  static func funAsrHotwords(_ words: [String]) -> String {
    var kept: [String] = []
    for word in words {
      if kept.count == 30 { break }
      if word.isEmpty || word.contains(",") { continue }
      kept.append(word)
    }
    return kept.joined(separator: ",")
  }

  /// A space between two segments only where both touching ends are ASCII letters or digits.
  static func joinSegments(_ segments: [String]) -> String {
    var joined = ""
    for segment in segments where !segment.isEmpty {
      if let last = joined.unicodeScalars.last, let first = segment.unicodeScalars.first,
         last.isASCII, isAlphanumeric(UInt8(last.value)), first.isASCII, isAlphanumeric(UInt8(first.value)) {
        joined += " "
      }
      joined += segment
    }
    return joined
  }

  private static let cjkPunctuation: Set<Unicode.Scalar> = ["，", "。", "？", "！", "、", "：", "；", "“", "”", "‘", "’", "（", "）", "《", "》", "…"]

  /// Removes spaces next to CJK punctuation, inside initialisms spelled as single capitals ("A I" → "AI"), and leading, repeated or trailing ones.
  static func tidy(_ text: String) -> String {
    let characters = Array(text.unicodeScalars)
    func singleCapital(_ index: Int) -> Bool {
      guard index >= 0, index < characters.count, characters[index].isASCII,
            (65...90).contains(characters[index].value) else { return false }
      let leftOK = index == 0 || characters[index - 1] == " " || !characters[index - 1].isASCII
      let rightOK = index + 1 >= characters.count || characters[index + 1] == " " || !characters[index + 1].isASCII
      return leftOK && rightOK
    }
    var out = String.UnicodeScalarView()
    for (index, character) in characters.enumerated() {
      if character == " " {
        let afterMark = index > 0 && cjkPunctuation.contains(characters[index - 1])
        let beforeMark = index + 1 < characters.count && cjkPunctuation.contains(characters[index + 1])
        let insideInitialism = index > 0 && singleCapital(index - 1) && singleCapital(index + 1)
        let leading = out.isEmpty
        let repeated = out.last == " "
        if afterMark || beforeMark || insideInitialism || leading || repeated { continue }
      }
      out.append(character)
    }
    while out.last == " " { out.removeLast() }
    return String(out)
  }

  private static func isSpace(_ byte: UInt8) -> Bool { byte == 0x20 || (0x09...0x0D).contains(byte) }
  private static func isAlphanumeric(_ byte: UInt8) -> Bool {
    (0x30...0x39).contains(byte) || (0x41...0x5A).contains(byte) || (0x61...0x7A).contains(byte)
  }
}

/// A catalog model with what is installed of it, as `msime_client_voice_local_models` reports it.
struct LocalSpeechModelInfo: Decodable, Identifiable, Equatable {
  let id: String
  let title: String
  let description: String
  let languages: [String]
  let streaming: Bool
  let isDefault: Bool
  let desktopOnly: Bool
  let installed: Bool
  let path: String
  let installedSize: UInt64
  let archiveSize: UInt64
  /// Resident memory while recognizing, in bytes.
  let memory: UInt64
  let licenseSPDX: String
  let licenseSource: String
  let licenseTerms: String
  let licenseNotice: String
  let hotwords: String

  enum CodingKeys: String, CodingKey {
    case id, title, description, languages, streaming, installed, path, memory, hotwords
    case isDefault = "default"
    case desktopOnly = "desktop_only"
    case installedSize = "installed_size"
    case archiveSize = "archive_size"
    case licenseSPDX = "license_spdx"
    case licenseSource = "license_source"
    case licenseTerms = "license_terms"
    case licenseNotice = "license_notice"
  }

  init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)
    id = try values.decode(String.self, forKey: .id)
    title = try values.decodeIfPresent(String.self, forKey: .title) ?? id
    description = try values.decodeIfPresent(String.self, forKey: .description) ?? ""
    languages = try values.decodeIfPresent([String].self, forKey: .languages) ?? []
    streaming = try values.decodeIfPresent(Bool.self, forKey: .streaming) ?? false
    isDefault = try values.decodeIfPresent(Bool.self, forKey: .isDefault) ?? false
    desktopOnly = try values.decodeIfPresent(Bool.self, forKey: .desktopOnly) ?? false
    installed = try values.decodeIfPresent(Bool.self, forKey: .installed) ?? false
    path = try values.decodeIfPresent(String.self, forKey: .path) ?? ""
    installedSize = try values.decodeIfPresent(UInt64.self, forKey: .installedSize) ?? 0
    archiveSize = try values.decodeIfPresent(UInt64.self, forKey: .archiveSize) ?? 0
    memory = try values.decodeIfPresent(UInt64.self, forKey: .memory) ?? 0
    licenseSPDX = try values.decodeIfPresent(String.self, forKey: .licenseSPDX) ?? ""
    licenseSource = try values.decodeIfPresent(String.self, forKey: .licenseSource) ?? ""
    licenseTerms = try values.decodeIfPresent(String.self, forKey: .licenseTerms) ?? ""
    licenseNotice = try values.decodeIfPresent(String.self, forKey: .licenseNotice) ?? ""
    hotwords = try values.decodeIfPresent(String.self, forKey: .hotwords) ?? ""
  }
}

/// Where the phone keeps on-device models and which one `voice_input.asr_model_path` names.
enum LocalSpeechModelLocation {
  /// `Application Support/voice-models` in the app's own container. The keyboard extension never loads a model (its memory limit is far below one), so the models stay out of the App Group.
  static func root(fileManager: FileManager = .default) throws -> URL {
    var root = try fileManager.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
      .appendingPathComponent("voice-models", isDirectory: true)
    try fileManager.createDirectory(at: root, withIntermediateDirectories: true)
    // Hundreds of megabytes that can be downloaded again do not belong in a device backup.
    var values = URLResourceValues()
    values.isExcludedFromBackup = true
    try? root.setResourceValues(values)
    return root
  }

  /// The installed model the stored path names. iOS moves an app's container on update, so a stored path that no longer exists is looked up again by its model id under the current root.
  static func resolve(storedPath: String, root: URL?) -> URL? {
    let trimmed = storedPath.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return nil }
    let stored = URL(fileURLWithPath: trimmed, isDirectory: true)
    guard let root else { return nil }
    let managedRoot = root.resolvingSymlinksInPath().standardizedFileURL
    let resolvedStored = stored.resolvingSymlinksInPath().standardizedFileURL
    if isWithin(resolvedStored, root: managedRoot), LocalSpeechModelManifest.isModelDirectory(resolvedStored) {
      return resolvedStored
    }
    let moved = root.appendingPathComponent(stored.lastPathComponent, isDirectory: true)
    return LocalSpeechModelManifest.isModelDirectory(moved) ? moved.resolvingSymlinksInPath().standardizedFileURL : nil
  }

  /// Whether `storedPath` points at the model `id`, wherever the container was when it was written.
  static func names(_ storedPath: String, model id: String) -> Bool {
    let trimmed = storedPath.trimmingCharacters(in: .whitespacesAndNewlines)
    return !trimmed.isEmpty && URL(fileURLWithPath: trimmed).lastPathComponent == id
  }

  private static func isWithin(_ child: URL, root: URL) -> Bool {
    let rootPath = root.path.hasSuffix("/") ? root.path : root.path + "/"
    return child.path != root.path && child.path.hasPrefix(rootPath)
  }
}
