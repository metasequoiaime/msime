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
    case .mandarin: [.quanpin, .nineKey, .shuangpin, .ziranma, .microsoft, .shoudao, .wubi, .zhuyin, .stroke, .handwriting]
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

  /// 内嵌键盘扩展携带的语言词库（粤语、注音、笔画）。键盘会把缺少词库的已启用方案排除在选择器之外，所以「输入」页也从不提供这样的方案。App bundle 不运行 Engine，也没有自己的词库，所以去扩展里找，也就是键盘的 `InputSchemePreference.installedLanguageSchemes` 找到这些词库的地方。没有扩展的 App 不提供这些方案。
  static let installedDictionarySchemes: Set<ChineseInputScheme> = {
    guard let plugIns = Bundle.main.builtInPlugInsURL,
          let contents = try? FileManager.default.contentsOfDirectory(at: plugIns, includingPropertiesForKeys: nil)
    else { return [] }
    return contents.filter { $0.pathExtension == "appex" }
      .compactMap(Bundle.init(url:))
      .reduce(into: Set<ChineseInputScheme>()) { installed, extensionBundle in
        let directory = InputSchemePreference.languageDictionaryDirectory(in: extensionBundle)
        installed.formUnion(InputSchemePreference.installedLanguageSchemes(in: directory) ?? [])
      }
  }()
}
