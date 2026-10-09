import Foundation

// 码表拼不出来的五笔码，用同字母的全拼候选来回答。码表能回答的码保留自己的候选，所以照打的五笔不受影响。
//
// 选择放在共享偏好文档 `wubi_mixed_pinyin` 里，设置页写、引擎经 host-api 读；`Preferences::for_edition` 会给新建的五笔版把它打开，所以不需要 App Group 镜像。
@MainActor
enum WubiMixedPinyinPreference {
  static let documentKey = "wubi_mixed_pinyin"
  /// 文档里没有这个键时的值：随版本，full 是关、五笔版是开。
  static var documentDefault: Bool { MSIMEAppEdition.wubiMixedPinyinDefault }

  /// 文档里的值；缺这个键时取 `documentDefault`。
  static func isEnabled(in document: [String: Any]?) -> Bool {
    document?[documentKey] as? Bool ?? documentDefault
  }
}
