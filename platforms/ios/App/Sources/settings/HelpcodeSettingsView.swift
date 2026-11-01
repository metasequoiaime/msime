import SwiftUI
import UniformTypeIdentifiers

/// Helpcode for Quanpin and Shuangpin, one section each.
///
/// Like the punctuation page, these live only in the shared preference document, as the `quanpin_helpcode` and `shuangpin_helpcode` objects, so each write merges one field into its object and leaves the rest of the object as stored. The keyboard hands a change to its live session the next time it appears.
///
/// 方案列表在内置方案之后列出用户导入的码表（#7176），即 `msime_client_plugins` 的 `catalog` 里 `kind = "helpcode"` 的包；选中时写 `plugins.helpcode_pack_*`，规则见 `HelpcodeChoicePolicy`。「导入辅助码表…」用系统文件选择器选一个 `.txt`，严格按 UTF-8 解码后交给 `import_helpcode_table`，成功后自动选中到发起导入的那一份；「已导入的码表」里左滑删除，删的是正在用的码表时退回内置方案。包装进 App Group 的 `<共享状态目录>/plugins`，键盘读的就是这一份。
struct HelpcodeSettingsView: View {
  /// The scheme, its shared-document key, and the values the engine ships when the document has none.
  struct Scheme: Identifiable {
    let key: String
    let label: String
    let defaultSchema: String
    let defaultShown: Bool
    var id: String { key }
  }

  // 导入和删除在主线程之外改共享文档，要用到这两份方案和 `merge`，所以它们不归主线程。
  nonisolated static let shuangpin = Scheme(key: "shuangpin_helpcode", label: "双拼", defaultSchema: "lantian", defaultShown: true)
  nonisolated static let quanpin = Scheme(key: "quanpin_helpcode", label: "全拼", defaultSchema: "ziranma", defaultShown: false)
  static let schemes = [shuangpin, quanpin]

  /// 引擎自带的辅助码表，形式为 (schema id, title)。「输入」页的「辅助码方案」一行列出的是同一批。
  static let schemas: [(String, String)] = [
    ("lantian", "蓝天小雨点"), ("ziranma", "自然码"), ("shouyou2_0", "首右2.0"),
    ("shouyouplus", "首右plus"), ("xiaohe", "小鹤"), ("jiajia", "加加"), ("wubi86", "五笔 86"),
  ]

  @State private var enabled: [String: Bool] = [:]
  @State private var schema: [String: String] = [:]
  @State private var shown: [String: Bool] = [:]
  /// 每份方案选中的已导入码表，没选时为空字符串。
  @State private var selectedPack: [String: String] = [:]
  /// 已导入的码表；读不出来时为空，方案列表照样列出内置方案。
  @State private var packs: [HelpcodeChoicePolicy.Pack] = []
  @State private var saveFailed = false
  /// 文件选择器是否打开。只当选择器的开关用，发起导入的方案另存在 `importScheme`。
  @State private var importing = false
  /// 正在为哪一份方案选文件：导入成功后选中到这一份。不能和选择器的开关合成一个状态：系统在调用完成回调之前先把开关置为 false，开关的 setter 若顺手清空方案，回调里就读不到是哪一组发起的导入，双拼的导入会落到全拼上。
  @State private var importScheme = Self.shuangpin
  @State private var busy = false
  @State private var notice: String?
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()

  var body: some View {
    Form {
      Section {
        Text("全拼或双拼组字时，先点 Shift 再输入的字母作为辅助码交给输入引擎，用于缩小候选。五笔、九宫格、日语、韩语、粤拼、注音、越南语、藏文、笔画和快捷模式不使用辅助码。")
          .font(.footnote).foregroundStyle(.secondary)
        Text("也可以导入自己的码表：一个 UTF-8 的 .txt，不超过 1 MiB，每行「字=码」，码是 1 到 2 个小写字母，同一个字只能出现一次。码表有问题时整张拒绝，提示里写明文件名、原因和行号。")
          .font(.footnote).foregroundStyle(.secondary)
      }
      ForEach(Self.schemes) { scheme in
        let on = enabled[scheme.key] ?? true
        Section {
          Toggle("\(scheme.label)辅助码", isOn: stored(scheme.key, "enabled", $enabled, true))
            .accessibilityIdentifier("\(scheme.key).enabled")
          Picker("辅助码方案", selection: choice(scheme)) {
            ForEach(Self.schemas, id: \.0) { Text($0.1).tag($0.0) }
            ForEach(HelpcodeChoicePolicy.packOptions(packs, selectedPack: selectedPack[scheme.key] ?? ""), id: \.value) {
              Text($0.label).tag($0.value)
            }
          }
          .disabled(!on || busy)
          .accessibilityIdentifier("\(scheme.key).schema")
          Button("导入辅助码表…") {
            importScheme = scheme
            importing = true
          }
            .disabled(!on || busy)
            .accessibilityIdentifier("\(scheme.key).import")
          Toggle("在候选栏中显示\(scheme.label)辅助码", isOn: stored(scheme.key, "show_in_candidate_window", $shown, scheme.defaultShown))
            .disabled(!on)
            .accessibilityIdentifier("\(scheme.key).shown")
        } header: {
          Text(scheme.label)
        } footer: {
          if saveFailed && scheme.key == Self.schemes.last?.key {
            Text(SettingsSaveFailure.message)
          }
        }
      }
      if !packs.isEmpty {
        Section {
          ForEach(packs) { pack in
            Text(pack.name).accessibilityIdentifier("helpcodePack.\(pack.id)")
          }
          .onDelete { offsets in offsets.first.map { remove(packs[$0]) } }
          .deleteDisabled(busy)
        } header: {
          DesignSectionHeader(info: "左滑删除。删除后，正在用它的方案改回内置方案。") { Text("已导入的码表") }
        }
      }
    }
    .navigationTitle("辅助码").navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive(reload, onAppear: { request.appear() })
    .onDisappear {
      ViewRequestTask.disappear(request: $request, pending: $pending)
      busy = false
    }
    .fileImporter(isPresented: $importing, allowedContentTypes: [.plainText, .text, .data]) { result in
      guard case .success(let url) = result else { return }
      importTable(from: url, into: importScheme)
    }
    .alert("辅助码表", isPresented: Binding(get: { notice != nil }, set: { if !$0 { notice = nil } })) {
      Button("好", role: .cancel) {}
    } message: {
      Text(notice ?? "")
    }
  }

  /// 方案列表的选择：选了包时是 `pack:<id>`，否则是内置方案。写法见 `HelpcodeChoicePolicy.choose`；写不进去时读回实际值。
  private func choice(_ scheme: Scheme) -> Binding<String> {
    Binding(get: {
      HelpcodeChoicePolicy.selectedValue(schema: schema[scheme.key] ?? scheme.defaultSchema,
                                         selectedPack: selectedPack[scheme.key] ?? "")
    }, set: { value in
      saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences {
        HelpcodeChoicePolicy.choose(value, for: scheme, document: &$0)
      }
      reload()
    })
  }

  private func importTable(from url: URL, into scheme: Scheme) {
    guard request.isVisible, !busy else { return }
    busy = true
    let id = ViewRequestTask.claim(request: $request, pending: $pending)
    pending = Task<Void, Never> { @MainActor in
      var finished = false
      defer {
        if request.isCurrent(id) {
          if !finished { busy = false }
          ViewRequestTask.clear(request: $request, pending: $pending, id: id)
        }
      }
      guard (try? request.checkCurrent(id)) != nil else { return }
      let result = await Task.detached(priority: .userInitiated) { Self.importTable(url, into: scheme) }.value
      guard (try? request.checkCurrent(id)) != nil else { return }
      busy = false
      finished = true
      ViewRequestTask.clear(request: $request, pending: $pending, id: id)
      notice = result
      reload()
    }
  }

  private func remove(_ pack: HelpcodeChoicePolicy.Pack) {
    guard request.isVisible, !busy else { return }
    busy = true
    let id = ViewRequestTask.claim(request: $request, pending: $pending)
    pending = Task<Void, Never> { @MainActor in
      var finished = false
      defer {
        if request.isCurrent(id) {
          if !finished { busy = false }
          ViewRequestTask.clear(request: $request, pending: $pending, id: id)
        }
      }
      guard (try? request.checkCurrent(id)) != nil else { return }
      let result = await Task.detached(priority: .userInitiated) { Self.remove(pack) }.value ?? "删除失败，请重试"
      guard (try? request.checkCurrent(id)) != nil else { return }
      busy = false
      finished = true
      ViewRequestTask.clear(request: $request, pending: $pending, id: id)
      notice = result
      reload()
    }
  }

  /// 读文件、严格解码、导入，成功后选中到发起导入的那一份方案；返回给用户看的提示。读写文件并校验整张码表，在主线程之外调用。
  nonisolated static func importTable(_ url: URL, into scheme: Scheme, stateRoot: URL? = nil) -> String {
    let scoped = url.startAccessingSecurityScopedResource()
    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
    let data: Data?
    do {
      data = try BoundedFileReader.read(from: url, maximumBytes: HelpcodeChoicePolicy.maximumTableBytes)
    } catch BoundedFileReader.Failure.tooLarge {
      data = nil
    } catch {
      return "读取文件失败，请重新选择。"
    }
    let text: String
    switch HelpcodeChoicePolicy.decode(data) {
    case .success(let decoded): text = decoded
    case .failure(let failure): return failure.message
    }
    let action: [String: Any] = ["operation": "import_helpcode_table", "file_name": url.lastPathComponent, "text": text]
    switch MetasequoiaInputSessionBridge.plugins(action, stateRoot: stateRoot) {
    case .failure(let failure):
      return HelpcodeChoicePolicy.importFailure(failure)
    case .success(let value):
      guard let pack = value as? [String: Any], let id = pack["id"] as? String, !id.isEmpty else {
        return HelpcodeChoicePolicy.importFailure(.init(code: "invalid_response"))
      }
      let name = pack["name"] as? String ?? id
      let entries = (pack["entries"] as? NSNumber)?.intValue ?? 0
      let selected = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) {
        HelpcodeChoicePolicy.setPack(id, for: scheme, document: &$0)
      }
      return HelpcodeChoicePolicy.importedMessage(name: name.isEmpty ? id : name, entries: entries, selected: selected)
    }
  }

  /// 删除一个导入的码表；全拼或双拼正选着它时把那一份选择清成空字符串，退回内置方案。删除失败时为 nil。在主线程之外调用。
  nonisolated static func remove(_ pack: HelpcodeChoicePolicy.Pack, stateRoot: URL? = nil) -> String? {
    guard case .success = MetasequoiaInputSessionBridge.plugins(
      ["operation": "remove", "kind": "helpcode", "id": pack.id], stateRoot: stateRoot) else { return nil }
    var cleared = false
    let saved = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) {
      cleared = HelpcodeChoicePolicy.clearSelections(of: pack.id, document: &$0)
    }
    // 清选择没写成时，选择仍指着已删的包：键盘照样退回内置方案（host-api 载不入包就用 `schema`），本页把它显示成「未找到」，用户可以再选一次。
    return HelpcodeChoicePolicy.removedMessage(name: pack.name, wasSelected: cleared && saved)
  }

  /// 已导入的码表：`catalog` 里 `kind = "helpcode"` 的包。目录会校验每个包的码表，在主线程之外调用。
  nonisolated static func installedPacks(stateRoot: URL? = nil) -> [HelpcodeChoicePolicy.Pack] {
    guard case .success(let value) = MetasequoiaInputSessionBridge.plugins(["operation": "catalog"], stateRoot: stateRoot) else {
      return []
    }
    return HelpcodeChoicePolicy.packs(inCatalog: value)
  }

  /// A binding that merges one field into a helpcode object of the shared document; a refused write puts the stored values back.
  private func stored<Value>(_ object: String, _ field: String, _ state: Binding<[String: Value]>, _ fallback: Value) -> Binding<Value> {
    let valueState = Binding(get: { state.wrappedValue[object] ?? fallback },
                             set: { state.wrappedValue[object] = $0 })
    let scheme = Self.schemes.first { $0.key == object } ?? Self.quanpin
    return SharedPreferenceBinding.custom(state: valueState, onSave: { saved in
      saveFailed = !saved
      if !saved { reload() }
    }) { document, value in
      Self.merge(field, value, into: scheme, document: &document)
    }
  }

  /// 触摸方案使用的辅助码对象：双拼 profile 用 `shuangpin_helpcode`，其余都用 `quanpin_helpcode`，与 Android 一致。
  static func family(of inputScheme: ChineseInputScheme) -> Scheme {
    inputScheme.shuangpinProfile != nil ? shuangpin : quanpin
  }

  /// 辅助码对象指定的码表；文档里没有时取该系列的默认码表。
  static func schema(of scheme: Scheme, in document: [String: Any]?) -> String {
    (document?[scheme.key] as? [String: Any])?["schema"] as? String ?? scheme.defaultSchema
  }

  /// 把一个字段合并进共享文档中的辅助码对象，其余字段保持原样。文档要求对象中带 `schema`，所以对象缺失时先用该系列的引擎默认值填好。
  nonisolated static func merge(_ field: String, _ value: Any, into scheme: Scheme, document: inout [String: Any]) {
    var nested = document[scheme.key] as? [String: Any]
      ?? ["enabled": true, "schema": scheme.defaultSchema, "show_in_candidate_window": scheme.defaultShown]
    nested[field] = value
    document[scheme.key] = nested
  }

  private func reload() {
    guard request.isVisible, !busy else { return }
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    for scheme in Self.schemes {
      let stored = preferences[scheme.key] as? [String: Any] ?? [:]
      enabled[scheme.key] = stored["enabled"] as? Bool ?? true
      schema[scheme.key] = stored["schema"] as? String ?? scheme.defaultSchema
      shown[scheme.key] = stored["show_in_candidate_window"] as? Bool ?? scheme.defaultShown
      selectedPack[scheme.key] = HelpcodeChoicePolicy.selectedPack(of: scheme, in: preferences)
    }
    let id = ViewRequestTask.claim(request: $request, pending: $pending)
    pending = Task<Void, Never> { @MainActor in
      defer { ViewRequestTask.clear(request: $request, pending: $pending, id: id) }
      guard (try? request.checkCurrent(id)) != nil else { return }
      let loaded = await Task.detached(priority: .userInitiated) { Self.installedPacks() }.value
      guard (try? request.checkCurrent(id)) != nil else { return }
      packs = loaded
    }
  }
}
