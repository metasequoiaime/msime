import Foundation

/// 上屏后联想时候选栏右端放哪些按键，共享文档里是 `next_word_prediction_buttons`（client-core `NextWordPredictionButtons`），三种写法与 Android 的 `PredictionButtonsPolicy` 一一对应。缺键、不是字符串或不认识的写法（较新版本加的样式）都按现有的「只有展开」读。
///
/// 只管联想那一帧（View 的 `prediction` 为真）：普通组字时关闭键永远不出现，展开键照旧。关闭键送取消，引擎清掉联想后报已处理，不上屏也不插入空格。
enum NextWordPredictionButtons: String, CaseIterable, Hashable {
  /// 现有样式：只有展开键，关闭联想靠继续输入。
  case expandOnly = "expand_only"
  /// 关闭键 × 加展开键；候选网格打开时 × 仍在箭头左边。
  case closeAndExpand = "close_and_expand"
  /// 只有关闭键 ×，联想只能在候选栏里左右滑动选择。
  case closeOnly = "close_only"

  /// 上屏后联想的开关，默认关；宿主能力见 client-core 的 `HostPlatform::shows_next_word_prediction`。
  static let enabledKey = "next_word_prediction"
  static let documentKey = "next_word_prediction_buttons"

  /// 设置页的选项名，与 Android「联想栏按键」逐字相同。
  var title: String {
    switch self {
    case .expandOnly: "只有展开 ∨"
    case .closeAndExpand: "关闭 × 和展开 ∨"
    case .closeOnly: "只有关闭 ×"
    }
  }

  /// 文档里的样式，读法同 Android：只认三种写法，其余都是「只有展开」。
  static func style(in document: [String: Any]?) -> NextWordPredictionButtons {
    (document?[documentKey] as? String).flatMap(Self.init(rawValue:)) ?? .expandOnly
  }

  /// 候选栏是否放关闭键：只在显示联想时，并且样式带 ×。
  func showsClose(prediction: Bool) -> Bool {
    prediction && self != .expandOnly
  }

  /// 展开键能不能出现（出现时仍要满足原来的条件）：联想时「只有关闭」不放展开键，其余情况照旧。
  func allowsExpand(prediction: Bool) -> Bool {
    !prediction || self != .closeOnly
  }
}
