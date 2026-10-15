import Foundation

/// 「输入」页「语言与方案」卡片中的一种语言，以及能输入它的触屏方案，顺序与其选项面板列出的一致。它对应 Android 的 `TypingPage.Language`：英文没有自己的触屏方案（中/英键始终都在），所以这里不把它当作一种语言。
enum InputLanguage: String, CaseIterable, Identifiable {
  case mandarin, cantonese, japanese, korean, vietnamese, tibetan

  var id: String { rawValue }

  var name: String {
    switch self {
    case .mandarin: "普通话"
    case .cantonese: "粤语"
    case .japanese: "日语"
    case .korean: "韩语"
    case .vietnamese: "越南语"
    case .tibetan: "藏语"
    }
  }

  /// 该语言 30pt 图块上的字形。
  var tile: String {
    switch self {
    case .mandarin: "汉"
    case .cantonese: "粤"
    case .japanese: "あ"
    case .korean: "한"
    case .vietnamese: "Vi"
    case .tibetan: "བོད"
    }
  }

  var schemes: [ChineseInputScheme] {
    switch self {
    // 全拼的三种键盘按 26 键、14 键、9 键排列，与共享界面的普通话方案列表一致。
    case .mandarin: [.quanpin, .fourteenKey, .nineKey, .nineKeyShuangpin, .shuangpin, .ziranma, .microsoft, .shoudao, .wubi, .zhuyin, .stroke, .handwriting]
    case .cantonese: [.cantonese]
    case .japanese: [.japanese, .japaneseNineKey]
    case .korean: [.korean]
    case .vietnamese: [.vietnamese]
    case .tibetan: [.tibetan]
    }
  }

  /// 「普通话」始终保留它的行；其他语言都可以移除后再加回来。
  var isRemovable: Bool { self != .mandarin }

  static func of(_ scheme: ChineseInputScheme) -> InputLanguage {
    allCases.first { $0.schemes.contains(scheme) } ?? .mandarin
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
