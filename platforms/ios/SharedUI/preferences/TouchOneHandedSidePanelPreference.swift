import Foundation

/// 大屏单手模式时空出来的那一侧放什么，共享文档里是 `touch_one_handed_side_panel`（client-core `TouchOneHandedSidePanel`），三端共用：`off` 空着（默认，client-core 不写进文档），`number_pad` 放侧边数字键盘。只在 iPad 全宽键盘的单手模式下、主键区画九键一族的字母层时起作用，见 .agents/notes/implemented/feature/2026-10-29-tablet-one-hand-side-number-pad.md。
///
/// 做成取值而不是开关，是给以后的文本编辑面板（`text_edit`）留位置；今天的版本读到不认识的值按空着画。iOS 的账号同步不带这一项，与其他按键布局偏好一样。
enum TouchOneHandedSidePanel: String, CaseIterable, Hashable {
  case off
  case numberPad = "number_pad"

  static let documentKey = "touch_one_handed_side_panel"

  /// 文档里的取值。缺键、不是字符串或不认识的写法（较新版本加的面板）一律按 `off` 读，与 client-core 的宽松读取一致。
  init(in document: [String: Any]?) {
    self = (document?[Self.documentKey] as? String).flatMap(Self.init(rawValue:)) ?? .off
  }
}

/// 「大屏单手时侧边显示数字键盘」开关：设置页的文字和写共享文档的写法。键盘每次读到文档时直接按 `TouchOneHandedSidePanel(in:)` 取值，不另存 App Group 镜像。
enum TouchOneHandedSidePanelPreference {
  /// 设置页的标题和说明，与 Android、鸿蒙逐字相同。
  static let title = "大屏单手时侧边显示数字键盘"
  static let subtitle = "单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。"

  static func load(stateRoot: URL? = nil) -> TouchOneHandedSidePanel {
    TouchOneHandedSidePanel(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: stateRoot))
  }

  /// 只写这一项，其余保持文档里的样子；关掉时写 `off`，client-core 把它存成缺键。
  @discardableResult
  static func save(_ panel: TouchOneHandedSidePanel, stateRoot: URL? = nil) -> Bool {
    MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) {
      $0[TouchOneHandedSidePanel.documentKey] = panel.rawValue
    }
  }
}
