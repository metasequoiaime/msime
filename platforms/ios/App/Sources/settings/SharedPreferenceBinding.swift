import SwiftUI

/// 共享偏好文档的即时绑定：先更新控件，再把值写入文档，最后由页面处理写入结果。
enum SharedPreferenceBinding {
  static func topLevel<Value>(_ key: String, state: Binding<Value>,
                              onSave: @escaping (Bool) -> Void) -> Binding<Value> {
    make(state: state, onSave: onSave) { document, value in document[key] = value }
  }

  static func nested<Value>(_ object: String, key: String, state: Binding<Value>,
                            onSave: @escaping (Bool) -> Void) -> Binding<Value> {
    make(state: state, onSave: onSave) { document, value in
      var fields = document[object] as? [String: Any] ?? [:]
      fields[key] = value
      document[object] = fields
    }
  }

  private static func make<Value>(state: Binding<Value>, onSave: @escaping (Bool) -> Void,
                                  mutate: @escaping (inout [String: Any], Value) -> Void) -> Binding<Value> {
    Binding(get: { state.wrappedValue }, set: { value in
      state.wrappedValue = value
      onSave(MetasequoiaInputSessionBridge.updateSharedPreferences { mutate(&$0, value) })
    })
  }
}
