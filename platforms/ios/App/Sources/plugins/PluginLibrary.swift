import SwiftUI

/// 「插件」页的列表、详情和编辑器共用的状态：读到的目录、共享文档里插件那几项和按键音开关。页面推入详情、在详情里启用或删除之后回到列表，看到的是同一份。
///
/// 读写插件目录都经 `msime_client_plugins`（`MetasequoiaInputSessionBridge.plugins`），放在主线程之外；包装在 App Group 的 `<共享状态目录>/plugins`，键盘下次获得焦点时读到。启用、选用这些写的是共享文档，与辅助码页一样同步写、写完重读。
@MainActor final class PluginLibrary: ObservableObject {
  enum State: Equatable { case loading, loaded, failed }

  @Published private(set) var catalog = PluginCatalog()
  @Published private(set) var state = State.loading
  @Published private(set) var preferences = PluginPreferences()
  @Published private(set) var soundEnabled = KeyboardFeedbackPreference.soundEnabled
  @Published private(set) var scheme = InputSchemePreference.scheme
  /// 导入、删除或保存正在进行。
  @Published private(set) var working = false
  /// 列表里「导入插件」下面的一句话：刚导入或保存了什么。
  @Published var notice = ""
  /// 刚在编辑器里保存过的包：它的详情页顶上说明还差哪一步。
  @Published var justSaved: String?
  private var generation = 0

  var missing: [PluginMissingSelection] {
    state == .loaded ? PluginCatalogPolicy.missingSelections(preferences, packages: catalog.packages) : []
  }

  func marker(_ pack: PluginPackage) -> String? {
    PluginCatalogPolicy.marker(pack, preferences: preferences, soundEnabled: soundEnabled, scheme: scheme)
  }

  /// 重读共享文档和目录。目录要校验每个包，放在主线程之外。
  func reload() {
    readPreferences()
    generation += 1
    let current = generation
    Task {
      let value = await Task.detached(priority: .userInitiated) {
        MetasequoiaInputSessionBridge.plugins(["operation": "catalog"])
      }.value
      guard current == generation else { return }
      if case .success(let json) = value, let catalog = PluginCatalog(json: json) {
        self.catalog = catalog
        state = .loaded
      } else if state != .loaded {
        state = .failed
      }
    }
  }

  func readPreferences() {
    preferences = PluginPreferences(document: MetasequoiaInputSessionBridge.loadSharedPreferences())
    soundEnabled = KeyboardFeedbackPreference.soundEnabled
    scheme = InputSchemePreference.scheme
  }

  /// 改一次共享文档并读回；写不进去时返回 false，页面说保存失败。
  @discardableResult func update(_ mutate: (inout [String: Any]) -> Void) -> Bool {
    let saved = MetasequoiaInputSessionBridge.updateSharedPreferences(mutate)
    readPreferences()
    return saved
  }

  func setTable(_ pack: PluginPackage, enabled: Bool) -> Bool {
    update { PluginCatalogPolicy.setTable(pack.kind, id: pack.id, enabled: enabled, document: &$0) }
  }

  /// 设为当前音效包：写 `plugins.key_sound.pack`，并打开按键音开关（选用即使用，与桌面「设为当前音效包」同时打开按键音一样）。
  func chooseKeySound(_ pack: PluginPackage) -> Bool {
    let saved = update { KeySoundPackPolicy.choose(pack.id, document: &$0) }
    if saved && !KeyboardFeedbackPreference.soundEnabled {
      KeyboardFeedbackPreference.set(true, forKey: KeyboardFeedbackPreference.soundKey)
    }
    soundEnabled = KeyboardFeedbackPreference.soundEnabled
    return saved
  }

  /// 导入用系统文件选择器选中的 `.zip`：先复制进私有暂存目录再安装，见 `PluginImport`。
  func importArchive(_ url: URL) async -> String? {
    guard !working else { return nil }
    working = true
    defer { working = false }
    let result = await Task.detached(priority: .userInitiated) { PluginImport.importArchive(url) }.value
    switch result {
    case .done(let pack):
      notice = PluginCatalogPolicy.importedNotice(pack)
      reload()
      return nil
    case .failed(let message):
      return message
    }
  }

  /// 删除一个包，并把选中、启用它的设置清掉；失败时返回提示。
  func remove(_ pack: PluginPackage) async -> String? {
    guard !working else { return nil }
    working = true
    defer { working = false }
    let action: [String: Any] = ["operation": "remove", "kind": pack.kind.rawValue, "id": pack.id]
    let result = await Task.detached(priority: .userInitiated) { MetasequoiaInputSessionBridge.plugins(action) }.value
    if case .failure(let failure) = result {
      return PluginCatalogPolicy.errorMessage(failure, fallback: "删除失败，请重试。")
    }
    update { PluginCatalogPolicy.withoutRemovedPack(pack.kind, id: pack.id, document: &$0) }
    reload()
    return nil
  }

  /// 编辑器读出一个已安装的包。
  nonisolated func loadAuthored(kind: PluginKind, id: String) async -> Result<PluginEditorModel.Draft, PluginFailureMessage> {
    await Task.detached(priority: .userInitiated) {
      switch MetasequoiaInputSessionBridge.plugins(["operation": "authoring_load", "kind": kind.rawValue, "id": id]) {
      case .failure(let failure):
        return .failure(PluginFailureMessage(PluginCatalogPolicy.errorMessage(failure, fallback: "无法读取这个插件，请重试。")))
      case .success(let value):
        guard let draft = PluginEditorModel.Draft(authored: value) else {
          return .failure(PluginFailureMessage("无法读取这个插件，请重试。"))
        }
        return .success(draft)
      }
    }.value
  }

  /// 保存编辑器里的包：按导入同样的规则校验并安装，成功后重读目录，返回装好的包。
  func saveAuthored(_ draft: PluginEditorModel.Draft) async -> PluginInstallOutcome {
    guard !working else { return .failed("正在处理上一个操作，请稍候。") }
    working = true
    defer { working = false }
    let action: [String: Any] = ["operation": "authoring_save", "pack": PluginEditorModel.pack(from: draft),
                                 "replace": draft.id != nil]
    let result = await Task.detached(priority: .userInitiated) { MetasequoiaInputSessionBridge.plugins(action) }.value
    switch result {
    case .failure(let failure):
      return .failed(PluginCatalogPolicy.errorMessage(failure, fallback: "保存失败，请重试。"))
    case .success(let value):
      guard let pack = PluginPackage(json: value) else { return .failed("保存失败，请重试。") }
      notice = "已保存\(pack.kindLabel)「\(pack.name)」。"
      justSaved = pack.key
      reload()
      return .done(pack)
    }
  }

  /// 社区装好一个包之后重读目录。
  func installed(_ pack: PluginPackage) {
    notice = PluginCatalogPolicy.importedNotice(pack)
    reload()
  }
}

/// 装进一个包（导入、保存、从社区安装）的结果。
enum PluginInstallOutcome {
  case done(PluginPackage)
  case failed(String)
}

/// 一句给用户看的失败说明。
struct PluginFailureMessage: Error, Equatable {
  let message: String
  init(_ message: String) { self.message = message }
}

/// 导入 `.zip`：选择器交回的是 App 沙箱之外的文件，先复制进 App 私有的暂存目录（固定的文件名 `pack.zip`），再由 `msime_client_plugins` 的 `import` 按与桌面相同的规则检查并安装，拒绝时不留下任何已安装的内容。复制前先看大小，超过 client-services 的上限（`import.rs` 的 `MAX_ARCHIVE_BYTES`，80 MiB）就不复制。不论结果如何，暂存目录用完就删。与鸿蒙插件页的导入相同；手机上不提供导入文件夹。
enum PluginImport {
  /// `import.rs` 的 `MAX_ARCHIVE_BYTES`。
  static let maximumArchiveBytes = 80 * 1024 * 1024
  static let archiveName = "pack.zip"

  /// 暂存目录：App 自己的临时目录下每次导入一个新文件夹。
  static func stagingDirectory(root: URL = FileManager.default.temporaryDirectory) -> URL {
    root.appendingPathComponent("plugin-import", isDirectory: true)
      .appendingPathComponent(UUID().uuidString, isDirectory: true)
  }

  static func importArchive(_ url: URL, stateRoot: URL? = nil) -> PluginInstallOutcome {
    let scoped = url.startAccessingSecurityScopedResource()
    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
    let fm = FileManager.default
    guard let values = try? url.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey]),
          values.isRegularFile == true else {
      return .failed(PluginCatalogPolicy.errorMessage(.init(code: "plugin_invalid", detail: "压缩包不是普通文件"),
                                                      fallback: "导入失败，请重试。"))
    }
    if let size = values.fileSize, size > maximumArchiveBytes {
      return .failed(PluginCatalogPolicy.errorMessage(.init(code: "plugin_archive", detail: "压缩包太大"),
                                                      fallback: "导入失败，请重试。"))
    }
    let staging = stagingDirectory()
    defer { try? fm.removeItem(at: staging) }
    let source = staging.appendingPathComponent(archiveName)
    do {
      try fm.createDirectory(at: staging, withIntermediateDirectories: true)
      try fm.copyItem(at: url, to: source)
    } catch {
      return .failed("读取文件失败，请重新选择。")
    }
    switch MetasequoiaInputSessionBridge.plugins(["operation": "import", "source": source.path], stateRoot: stateRoot) {
    case .failure(let failure):
      return .failed(PluginCatalogPolicy.errorMessage(failure, fallback: "导入失败，请重试。"))
    case .success(let value):
      guard let pack = PluginPackage(json: value) else { return .failed("导入失败，请重试。") }
      return .done(pack)
    }
  }
}
