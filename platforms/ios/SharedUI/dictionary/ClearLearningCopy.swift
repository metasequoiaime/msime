import Foundation

/// 设置 App「词库 › 学习」里「清除学习记录」的文案和结果提示，做法见 `MetasequoiaInputSessionBridge.resetLearnedData`。
///
/// 与 Android 的 `ClearLearningCopy` 逐字相同，只是确认框里没有云同步那一句、多一句导入的词也会删除：iOS 的账号同步不含个人词库，也没有命名词库，导入和从社区添加的词都是自造词。
enum ClearLearningCopy {
  static let title = "清除学习记录"
  static let subtitle = "删除学到的调频、联想和自造词，设置保留"
  static let action = "清除"
  static let confirmTitle = "清除学习记录？"
  /// 删什么、留什么，两端相同的那一句。
  static let removed =
    "将永久删除本机学到的候选调频、置顶和固定位置、选词记录、整句联想、拼写纠错记录，以及自己添加和修改的词条（自造词），无法撤销。"
  static let confirmMessage = removed + "导入和从社区添加的词条也一并删除。设置、常用语、剪贴板和打字统计不受影响。"
  static let cleared = "已清除学习记录"
  static let busy = "键盘正在使用词库，请先收起键盘再试。"
  static let failed = "清除学习记录失败，请稍后重试。"
}
