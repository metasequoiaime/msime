import Foundation

extension ChineseInputScheme {
  /// 方案作为取值而不是列表项出现时，设计稿显示的简称：设置里的输入行、输入页上某个语言的那一行，以及键盘的空格键。三种全拼布局（26 键、14 键、9 键）都显示全拼，双拼各方案显示 双拼 · <profile>，五笔带上 App Group 镜像里的码表版本；其余方案用完整标题。
  var shortLabel: String {
    switch self {
    case .quanpin, .nineKey, .fourteenKey: "全拼"
    case .shuangpin: "双拼 · 小鹤"
    case .ziranma: "双拼 · 自然码"
    case .microsoft: "双拼 · 微软"
    case .shoudao: "双拼 · 首道"
    case .wubi: WubiProfilePreference.profile == "wubi98" ? "五笔 98" : "五笔 86"
    default: title
    }
  }
}
