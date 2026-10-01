import SwiftUI

/// Helpcode for Quanpin and Shuangpin, one section each.
///
/// Like the punctuation page, these live only in the shared preference document, as the `quanpin_helpcode` and `shuangpin_helpcode` objects, so each write merges one field into its object and leaves the rest of the object as stored. The keyboard hands a change to its live session the next time it appears.
struct HelpcodeSettingsView: View {
  /// The scheme, its shared-document key, and the values the engine ships when the document has none.
  private struct Scheme: Identifiable {
    let key: String
    let label: String
    let defaultSchema: String
    let defaultShown: Bool
    var id: String { key }
  }

  private static let schemes = [
    Scheme(key: "shuangpin_helpcode", label: "双拼", defaultSchema: "lantian", defaultShown: true),
    Scheme(key: "quanpin_helpcode", label: "全拼", defaultSchema: "ziranma", defaultShown: false),
  ]

  private static let schemas: [(String, String)] = [
    ("lantian", "蓝天小雨点"), ("ziranma", "自然码"), ("shouyou2_0", "首右2.0"),
    ("shouyouplus", "首右plus"), ("xiaohe", "小鹤"), ("jiajia", "加加"),
  ]

  @Environment(\.scenePhase) private var scenePhase
  @State private var enabled: [String: Bool] = [:]
  @State private var schema: [String: String] = [:]
  @State private var shown: [String: Bool] = [:]
  @State private var saveFailed = false

  var body: some View {
    Form {
      Section {
        Text("全拼或双拼组字时，先点 Shift 再输入的字母作为辅助码交给输入引擎，用于缩小候选。五笔、九宫格、日语、韩语、粤拼、注音、越南语和本地输入模式不使用辅助码。")
          .font(.footnote).foregroundStyle(.secondary)
      }
      ForEach(Self.schemes) { scheme in
        let on = enabled[scheme.key] ?? true
        Section {
          Toggle("\(scheme.label)辅助码", isOn: stored(scheme.key, "enabled", $enabled, true))
            .accessibilityIdentifier("\(scheme.key).enabled")
          Picker("辅助码方案", selection: stored(scheme.key, "schema", $schema, scheme.defaultSchema)) {
            ForEach(Self.schemas, id: \.0) { Text($0.1).tag($0.0) }
          }
          .disabled(!on)
          .accessibilityIdentifier("\(scheme.key).schema")
          Toggle("在候选栏中显示\(scheme.label)辅助码", isOn: stored(scheme.key, "show_in_candidate_window", $shown, scheme.defaultShown))
            .disabled(!on)
            .accessibilityIdentifier("\(scheme.key).shown")
        } header: {
          Text(scheme.label)
        } footer: {
          if saveFailed && scheme.key == Self.schemes.last?.key {
            Text("设置没有保存，键盘可能正在写入同一份设置，请再试一次。")
          }
        }
      }
    }
    .navigationTitle("辅助码").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
  }

  /// A binding that merges one field into a helpcode object of the shared document; a refused write puts the stored values back.
  private func stored<Value>(_ object: String, _ field: String, _ state: Binding<[String: Value]>, _ fallback: Value) -> Binding<Value> {
    Binding(get: { state.wrappedValue[object] ?? fallback }, set: { value in
      state.wrappedValue[object] = value
      saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences {
        var nested = $0[object] as? [String: Any] ?? [:]
        nested[field] = value
        $0[object] = nested
      }
      if saveFailed { reload() }
    })
  }

  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    for scheme in Self.schemes {
      let stored = preferences[scheme.key] as? [String: Any] ?? [:]
      enabled[scheme.key] = stored["enabled"] as? Bool ?? true
      schema[scheme.key] = stored["schema"] as? String ?? scheme.defaultSchema
      shown[scheme.key] = stored["show_in_candidate_window"] as? Bool ?? scheme.defaultShown
    }
  }
}
