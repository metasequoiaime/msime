import Foundation

private typealias LocalSpeechByte = UInt8
private typealias LocalSpeechProgress = @convention(c) (UnsafePointer<LocalSpeechByte>?, UInt, UnsafeMutableRawPointer?) -> Void

@_silgen_name("msime_client_voice_local_models")
private func msimeClientVoiceLocalModels(_ request: UnsafePointer<LocalSpeechByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_voice_local_model_install")
private func msimeClientVoiceLocalModelInstall(_ request: UnsafePointer<LocalSpeechByte>?, _ length: UInt,
                                               _ progress: LocalSpeechProgress?, _ context: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_voice_local_model_cancel")
private func msimeClientVoiceLocalModelCancel(_ request: UnsafePointer<LocalSpeechByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_voice_local_model_remove")
private func msimeClientVoiceLocalModelRemove(_ request: UnsafePointer<LocalSpeechByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_voice_hotwords")
private func msimeClientVoiceHotwords(_ request: UnsafePointer<LocalSpeechByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_voice_hotword_correct")
private func msimeClientVoiceHotwordCorrect(_ request: UnsafePointer<LocalSpeechByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_prepare_host")
private func msimeClientVoicePrepareHost(_ request: UnsafePointer<LocalSpeechByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientVoiceStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// A word from the user's dictionary, as the shared hotword matcher takes it back.
struct LocalSpeechHotword: Codable, Equatable, Sendable {
  let text: String
  let pinyin: String
}

/// Install progress as the host reports it: `stage` is download, verify, extract or done.
struct LocalSpeechInstallProgress: Decodable, Equatable, Sendable {
  let id: String
  let stage: String
  let downloaded: UInt64
  let total: UInt64
}

/// The on-device model catalog, downloads and dictionary hotwords, all through the shared host-api so every platform installs and verifies models the same way. Every call blocks; run them off the main thread.
enum LocalSpeechModelStore {
  struct Catalog: Decodable, Equatable {
    let models: [LocalSpeechModelInfo]
    let `default`: String
  }

  static func isValidMirror(_ value: String) -> Bool {
    if value.isEmpty { return true }
    guard let url = CustomServiceConfiguration.validatedEndpoint(value, maximumCharacters: 2048),
          url.query == nil else { return false }
    return true
  }

  static func catalog(root: URL) throws -> Catalog {
    let value = try call(msimeClientVoiceLocalModels, ["root": root.path])
    let data = try JSONSerialization.data(withJSONObject: value)
    return try JSONDecoder().decode(Catalog.self, from: data)
  }

  /// Downloads, verifies and installs `id` under `root`, returning the model directory. `progress` is called on the calling thread.
  static func install(root: URL, id: String, mirror: String,
                      progress: @escaping (LocalSpeechInstallProgress) -> Void) throws -> URL {
    let request = try JSONSerialization.data(withJSONObject: ["root": root.path, "id": id, "mirror": mirror])
    let box = Unmanaged.passRetained(ProgressBox(progress))
    defer { box.release() }
    let callback: LocalSpeechProgress = { bytes, length, context in
      guard let bytes, let context else { return }
      let data = Data(bytes: bytes, count: Int(length))
      guard let update = try? JSONDecoder().decode(LocalSpeechInstallProgress.self, from: data) else { return }
      Unmanaged<ProgressBox>.fromOpaque(context).takeUnretainedValue().report(update)
    }
    let pointer = request.withUnsafeBytes { bytes in
      msimeClientVoiceLocalModelInstall(bytes.bindMemory(to: LocalSpeechByte.self).baseAddress, UInt(request.count),
                                        callback, box.toOpaque())
    }
    let value = try decode(pointer)
    guard let path = (value as? [String: Any])?["path"] as? String else { throw failure("local_model_unknown") }
    return URL(fileURLWithPath: path, isDirectory: true)
  }

  /// Stops a running install of `id`, which then fails with `local_model_cancelled`.
  static func cancel(id: String) {
    _ = try? call(msimeClientVoiceLocalModelCancel, ["id": id])
  }

  static func remove(root: URL, id: String) throws {
    _ = try call(msimeClientVoiceLocalModelRemove, ["root": root.path, "id": id])
  }

  /// The heaviest words of the user's own pinyin dictionary, highest weight first. `resources` is the packaged engine resources and `stateRoot` the shared App Group state the keyboard writes. A dictionary the keyboard is busy maintaining gives no words rather than an error: recognition works without them.
  static func hotwords(resources: URL, stateRoot: URL, limit: Int = 200) -> [LocalSpeechHotword] {
    guard let options = try? call(msimeClientVoicePrepareHost, ["resources": resources.path, "state_root": stateRoot.path]),
          let value = try? call(msimeClientVoiceHotwords, ["options": options, "limit": limit]) as? [String: Any],
          let rows = value["hotwords"] as? [[String: Any]]
    else { return [] }
    return rows.compactMap { row in
      guard let text = row["text"] as? String, !text.isEmpty else { return nil }
      return LocalSpeechHotword(text: text, pinyin: row["pinyin"] as? String ?? "")
    }
  }

  /// Rewrites near-miss spellings of the hotwords in a final transcript by pinyin similarity, for models whose manifest asks for it. The text comes back unchanged when the matcher fails.
  static func correct(_ text: String, hotwords: [LocalSpeechHotword]) -> String {
    guard !hotwords.isEmpty, !text.isEmpty else { return text }
    let rows = hotwords.map { ["text": $0.text, "pinyin": $0.pinyin] }
    guard let value = try? call(msimeClientVoiceHotwordCorrect, ["text": text, "hotwords": rows]) as? [String: Any],
          let corrected = value["text"] as? String
    else { return text }
    return corrected
  }

  /// The message a host error is shown as. Errors are `local_model_<kind>`, some followed by `: <detail>`.
  static func message(for code: String) -> String {
    switch code.split(separator: ":", maxSplits: 1).first.map(String.init) ?? code {
    case "local_model_cancelled": "已取消下载。"
    case "local_model_install_running": "这个模型正在下载。"
    case "local_model_network": "下载失败，请检查网络，或在下方填写镜像地址后重试。"
    case "local_model_http_status": "下载服务器拒绝了请求，请稍后重试或改用镜像地址。"
    case "local_model_size_mismatch", "local_model_checksum_mismatch": "下载的文件校验失败，请重试。"
    case "local_model_unsafe_archive": "模型压缩包内容不安全，已拒绝安装。"
    case "local_model_missing_file": "模型压缩包缺少必要文件。"
    case "local_model_io": "无法写入模型文件，请检查剩余存储空间。"
    case "local_model_invalid_mirror": "镜像地址无效，请填写 https:// 开头的地址或留空。"
    case "local_model_invalid_root": "无法使用模型目录。"
    default: "本地模型操作失败（\(code)）。"
    }
  }

  private final class ProgressBox {
    let report: (LocalSpeechInstallProgress) -> Void
    init(_ report: @escaping (LocalSpeechInstallProgress) -> Void) { self.report = report }
  }

  struct HostFailure: LocalizedError, Equatable {
    let code: String
    var errorDescription: String? { LocalSpeechModelStore.message(for: code) }
  }

  private static func failure(_ code: String) -> HostFailure { HostFailure(code: code) }

  private static func call(_ function: (UnsafePointer<LocalSpeechByte>?, UInt) -> UnsafeMutablePointer<CChar>?,
                           _ object: Any) throws -> Any {
    let data = try JSONSerialization.data(withJSONObject: object)
    return try data.withUnsafeBytes { bytes in
      try decode(function(bytes.bindMemory(to: LocalSpeechByte.self).baseAddress, UInt(data.count)))
    }
  }

  private static func decode(_ pointer: UnsafeMutablePointer<CChar>?) throws -> Any {
    guard let pointer else { throw failure("local_model_unknown") }
    let data = Data(bytes: pointer, count: strlen(pointer))
    msimeClientVoiceStringFree(pointer)
    guard let envelope = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) as? [String: Any]
    else { throw failure("local_model_unknown") }
    guard envelope["ok"] as? Bool == true else { throw failure(envelope["error"] as? String ?? "local_model_unknown") }
    return envelope["value"] ?? NSNull()
  }
}
