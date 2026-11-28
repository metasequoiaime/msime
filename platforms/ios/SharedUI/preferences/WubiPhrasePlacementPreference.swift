import Foundation

/// 五笔自造词入库时排在同码已有词之后还是之前（#11863）。取值与共享偏好的 `WubiPhrasePlacement`（`client-core/preferences.rs`）和 Windows 设置页（`InputSettingsModel.h` 的 `wubi_phrase_placements`）相同：`bottom` 排在同码已有词之后（默认，等于没有这个选项时的行为），`top` 排在它们之前；只管入库这一次，之后照常参与调频。
enum WubiPhrasePlacement: String, CaseIterable {
  case bottom
  case top

  var title: String {
    switch self {
    case .bottom: "同码词后"
    case .top: "同码词前"
    }
  }
}

/// 选择放在共享偏好文档 `wubi_phrase_placement` 里，词库页写、引擎经 host-api 读，没有 App Group 镜像。共享偏好在默认值时不写这个键，所以选回 `bottom` 时删掉它，而不是写 `"bottom"`。
enum WubiPhrasePlacementPreference {
  static let documentKey = "wubi_phrase_placement"

  /// 文档里的值；缺这个键或取值不认识（更新的版本写的）时按 `bottom` 读，与共享偏好读盘时的回退一致。
  static func placement(in document: [String: Any]?) -> WubiPhrasePlacement {
    (document?[documentKey] as? String).flatMap(WubiPhrasePlacement.init(rawValue:)) ?? .bottom
  }

  static func write(_ placement: WubiPhrasePlacement, into document: inout [String: Any]) {
    switch placement {
    case .bottom: document.removeValue(forKey: documentKey)
    case .top: document[documentKey] = placement.rawValue
    }
  }

  /// 写进共享文档；写失败时返回 false。`stateRoot` 只给测试用。
  static func update(_ placement: WubiPhrasePlacement, stateRoot: URL? = nil) -> Bool {
    MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { write(placement, into: &$0) }
  }
}
