import Foundation

/// 设置 App「词库 › 学习」里「清除学习记录」的文案和结果提示，做法见 `MetasequoiaInputSessionBridge.resetLearnedData`。
///
/// 与 Android 的 `ClearLearningCopy` 的标题、说明、按钮、确认框第一句、网盘同步那一句和失败提示逐字相同；确认框其余部分各说各的：iOS 的账号同步不含个人词库，也没有命名词库，导入和从社区添加的词都是自造词，跟着删。
enum ClearLearningCopy {
  static let title = "清除学习记录"
  static let subtitle = "删除学到的调频、联想和自造词，设置保留"
  static let action = "清除"
  static let confirmTitle = "清除学习记录？"
  /// 删什么，两端相同的那一句。用户自己加的快捷短语也是自造词，跟着删；常用语不是。
  static let removed =
    "将永久删除本机学到的候选调频、置顶和固定位置、选词记录、整句联想、拼写纠错记录，以及自己添加和修改的词条（自造词，含快捷短语），无法撤销。"
  /// 「我的词条」网盘同步（`IOSDictionarySync`）把删除同步到别的设备，两端相同；一次删得多时它先挡下、等用户确认。
  static let webdav = "开着词库网盘同步时，删掉的自造词也会从其他设备上删除（一次删得多时会先请你确认）。"
  static let confirmMessage = removed + "导入和从社区添加的词条也一并删除。设置、常用语、剪贴板和打字统计不受影响。" + webdav
  static let cleared = "已清除学习记录"
  static let busy = "键盘正在使用词库，请收起键盘、稍等几秒再试。"
  static let failed = "清除学习记录失败，请稍后重试。"
}
