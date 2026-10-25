import Foundation

/// 「输入」页「语言与方案」卡片中的一行，以及它能选择的触屏方案。它对应 Android 的 `TypingPage.Language`：英文没有自己的触屏方案（中/英键始终都在），所以这里不把它当作一种语言。
enum InputLanguage: String, CaseIterable, Identifiable {
  case quanpin, shuangpin, wubi, zhuyin, stroke, handwriting
  case cantonese, japanese, korean, vietnamese, tibetan

  var id: String { rawValue }

  /// 保留旧版全拼行的无障碍标识，已安装的 UI 测试和外部自动化仍能找到同一行。
  var accessibilityKey: String { self == .quanpin ? "mandarin" : rawValue }

  var name: String {
    switch self {
    case .quanpin: "全拼"
    case .shuangpin: "双拼"
    case .wubi: "五笔"
    case .zhuyin: "注音"
    case .stroke: "笔画"
    case .handwriting: "手写"
    case .cantonese: "粤拼"
    case .japanese: "日语"
    case .korean: "韩语"
    case .vietnamese: "越南语"
    case .tibetan: "藏语"
    }
  }

  /// 该语言 30pt 图块上的字形。
  var tile: String {
    switch self {
    case .quanpin: "拼"
    case .shuangpin: "双"
    case .wubi: "五"
    case .zhuyin: "注"
    case .stroke: "笔"
    case .handwriting: "写"
    case .cantonese: "粤"
    case .japanese: "あ"
    case .korean: "한"
    case .vietnamese: "Vi"
    case .tibetan: "བོད"
    }
  }

  var schemes: [ChineseInputScheme] {
    switch self {
    case .quanpin: [.quanpin, .fourteenKey, .nineKey]
    case .shuangpin: [.shuangpin, .ziranma, .microsoft, .shoudao, .nineKeyShuangpin]
    case .wubi: [.wubi]
    case .zhuyin: [.zhuyin]
    case .stroke: [.stroke]
    case .handwriting: [.handwriting]
    case .cantonese: [.cantonese]
    case .japanese: [.japanese, .japaneseNineKey]
    case .korean: [.korean]
    case .vietnamese: [.vietnamese]
    case .tibetan: [.tibetan]
    }
  }

  /// 每一行都可以移除，前提是键盘还剩至少一个可运行的方案。
  var isRemovable: Bool { true }

  static func of(_ scheme: ChineseInputScheme) -> InputLanguage {
    allCases.first { $0.schemes.contains(scheme) } ?? .quanpin
  }

  /// 设置合并随包词库与 App Group 已发布的资源包，下载后无需重启主应用。
  static var installedDictionarySchemes: Set<ChineseInputScheme> {
    guard let plugIns = Bundle.main.builtInPlugInsURL,
          let contents = try? FileManager.default.contentsOfDirectory(at: plugIns, includingPropertiesForKeys: nil)
    else { return [] }
    var installed = contents.filter { $0.pathExtension == "appex" }
      .compactMap(Bundle.init(url:))
      .reduce(into: Set<ChineseInputScheme>()) { installed, extensionBundle in
        let directory = InputSchemePreference.languageDictionaryDirectory(in: extensionBundle)
        installed.formUnion(InputSchemePreference.installedLanguageSchemes(in: directory) ?? [])
      }
    if (try? IOSResourcePacks.list())?.contains(where: { $0.id == "language-dictionaries" && $0.installed }) == true {
      installed.formUnion([.cantonese, .zhuyin, .stroke])
    }
    return installed
  }
}
