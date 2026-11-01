import Foundation

private typealias PackProgressCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?, UInt64, UInt64) -> Void
@_silgen_name("msime_client_resource_packs")
private func listResourcePacks(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_resource_pack_install")
private func installResourcePack(_ request: UnsafePointer<UInt8>?, _ length: UInt, _ progress: PackProgressCallback?, _ context: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_resource_pack_cancel")
private func cancelResourcePack(_ id: UnsafePointer<UInt8>?, _ length: UInt)
@_silgen_name("msime_client_string_free")
private func freeResourcePackResponse(_ response: UnsafeMutablePointer<CChar>?)

/// 与 Android 使用相同的资源锁、校验与原子发布；宿主下载，键盘只读取已发布资源。
enum IOSResourcePacks {
  struct Pack: Decodable, Identifiable {
    let id: String
    let state: String
    let size: UInt64
    var installed: Bool { state == "installed" }
  }

  struct Failure: Error { let code: String }
  private final class Progress {
    let report: (String, UInt64, UInt64) -> Void
    init(_ report: @escaping (String, UInt64, UInt64) -> Void) { self.report = report }
  }

  static func requiredPack(for scheme: ChineseInputScheme) -> String? {
    if scheme.isJapanese && MSIMEAppEdition.identifier != "japanese" { return "japanese" }
    return scheme.needsLanguageDictionary ? "language-dictionaries" : nil
  }

  /// 这里只查发布标记和文件属性，不联网，也不重新计算整个词库的哈希。
  static func list(stateRoot: URL? = nil) throws -> [Pack] {
    let request = try requestData(["state_root": root(stateRoot)])
    let response = request.withUnsafeBytes {
      listResourcePacks($0.bindMemory(to: UInt8.self).baseAddress, UInt(request.count))
    }
    let value = try read(response)
    return try JSONDecoder().decode([Pack].self, from: JSONSerialization.data(withJSONObject: value))
  }

  /// 阻塞接口必须在工作线程调用。取消、断点续传和完整性校验都由共享层处理。
  static func install(_ id: String, stateRoot: URL? = nil, files: [URL]? = nil,
                      progress: @escaping (String, UInt64, UInt64) -> Void) throws {
    var arguments: [String: Any] = ["state_root": root(stateRoot), "pack": id]
    if let files { arguments["files"] = files.map(\.path) }
    let request = try requestData(arguments)
    let context = Unmanaged.passRetained(Progress(progress))
    defer { context.release() }
    let response = request.withUnsafeBytes {
      installResourcePack($0.bindMemory(to: UInt8.self).baseAddress, UInt(request.count), { context, phase, done, total in
        guard let context, let phase else { return }
        Unmanaged<Progress>.fromOpaque(context).takeUnretainedValue().report(String(cString: phase), done, total)
      }, context.toOpaque())
    }
    _ = try read(response)
    IOSSharedPaths.excludeFromBackup(URL(fileURLWithPath: root(stateRoot)).appendingPathComponent("resource-packs"))
  }

  static func cancel(_ id: String) {
    let data = Data(id.utf8)
    data.withUnsafeBytes { cancelResourcePack($0.bindMemory(to: UInt8.self).baseAddress, UInt(data.count)) }
  }

  static func root(_ override: URL?) -> String { override?.path ?? MetasequoiaInputSessionBridge.sharedStateDirectory }

  /// 主应用的词库在内嵌扩展里；键盘与测试宿主则在自身 bundle 里。
  static func resourceBundles(in bundle: Bundle = .main) -> [Bundle] {
    var bundles = [bundle]
    if let plugins = bundle.builtInPlugInsURL,
       let contents = try? FileManager.default.contentsOfDirectory(at: plugins, includingPropertiesForKeys: nil) {
      bundles += contents.filter { $0.pathExtension == "appex" }.compactMap(Bundle.init(url:))
    }
    return bundles
  }

  static func bundled(_ id: String, in bundle: Bundle = .main) -> Bool {
    let names: [String]
    switch id {
    case "japanese": names = ["EngineResources/msime-japanese.dat"]
    case "language-dictionaries":
      names = ["language-dictionaries/msime-cantonese.db", "language-dictionaries/msime-zhuyin.db", "language-dictionaries/msime-stroke.db"]
    // 其余资源包（如英文音标表）不随 App 分发，只能下载；不能按语言词库的文件判断，否则带着语言词库的版本会把它们当成已安装。
    default: return false
    }
    return resourceBundles(in: bundle).contains { resourceBundle in
      guard let resources = resourceBundle.resourceURL else { return false }
      return names.allSatisfy { FileManager.default.fileExists(atPath: resources.appendingPathComponent($0).path) }
    }
  }

  private static func requestData(_ arguments: [String: Any]) throws -> Data {
    let data = try JSONSerialization.data(withJSONObject: arguments)
    guard data.count <= 16_384 else { throw Failure(code: "invalid resource pack request") }
    return data
  }

  private static func read(_ pointer: UnsafeMutablePointer<CChar>?) throws -> Any {
    guard let pointer else { throw Failure(code: "unavailable") }
    defer { freeResourcePackResponse(pointer) }
    guard let envelope = try JSONSerialization.jsonObject(with: Data(bytes: pointer, count: strlen(pointer))) as? [String: Any] else {
      throw Failure(code: "invalid_response")
    }
    guard envelope["ok"] as? Bool == true else { throw Failure(code: envelope["error"] as? String ?? "invalid_response") }
    return envelope["value"] ?? NSNull()
  }
}
