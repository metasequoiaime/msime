import SwiftUI
import CoreImage
import UIKit

private final class KeyboardBrandButton: UIButton {
  let brandImageView = UIImageView()

  override init(frame: CGRect) {
    super.init(frame: frame)
    brandImageView.contentMode = .scaleAspectFit
    brandImageView.accessibilityIdentifier = "keyboardBrandIcon"
    addSubview(brandImageView)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override func layoutSubviews() {
    super.layoutSubviews()
    brandImageView.bounds = CGRect(x: 0, y: 0, width: 28, height: 28)
    brandImageView.center = CGPoint(x: bounds.midX, y: bounds.midY)
  }
}

@MainActor
final class KeyboardViewController: UIInputViewController, UIGestureRecognizerDelegate {
  private enum LetterCaseState {
    case lowercase, shifted, capsLock
  }

  private let skinBackdrop = KeyboardSkinBackgroundView()
  private var keyboardRoot: UIStackView?
  private var nineGrid: UIStackView?
  private var nineControls: UIStackView?
  private var nineSidebarWidth: NSLayoutConstraint?
  private var fullSymbolsWidth: NSLayoutConstraint?
  private var bottomLanguageWidth: NSLayoutConstraint?
  private var bottomLanguageButton: UIButton?
  private var appliedLayout: KeyboardGeometry?

  private var keyboardHeightConstraint: NSLayoutConstraint?
  /// Seeded from the stored setting because updatePreferredKeyboardHeight() runs long before the
  /// first shared-preferences sync; starting at zero discarded the user's height on every load.
  private var sharedKeyboardHeightAdjustment =
    CGFloat(KeyboardLayoutPreference.heightAdjustment)
  private let session = MetasequoiaInputSessionBridge()
  /// 「全角输入」 for the running keyboard: starts from the shared `character_width`, then the 全角 card switches it.
  private var fullWidthInput = false
  /// The document's `character_width` as last applied, so a reload replaces the card's switch only when that field changed.
  private var appliedCharacterWidth: String?
  /// 「中文标点」 for the running keyboard: starts from the shared `chinese_punctuation`; the 更多 card flips it, and switching back to Chinese restores the document's value, as Windows does on every 中/英 switch.
  private var chinesePunctuation = true
  private var appliedChinesePunctuation = true
  private lazy var snapshotWorker: DictionarySnapshotWorker = {
    let worker = DictionarySnapshotWorker(session: session)
    worker.report = { [weak self] in self?.showDiagnostic($0) }
    worker.applied = { [weak self] in self?.synchronizePersonalDictionary(force: true) }
    return worker
  }()
  private let candidateGlossQueue = DispatchQueue(
    label: "app.msime.ios.candidate-gloss", qos: .utility)
  private let statisticsQueue = DispatchQueue(
    label: "app.msime.ios.typing-statistics", qos: .utility)
  private var candidateGlossEpoch: UInt64 = 0
  private var candidateGlossRequestedGeneration: UInt64?
  private let translations = CandidateTranslationStore()
  /// The translation service the shared document picks; `.none` means nothing was chosen or the chosen provider is incomplete, and no words leave the device.
  private var translationRoute: TranslationRoute = .none
  private lazy var onlineCandidates: OnlineCandidateProvider = {
    let provider = OnlineCandidateProvider(session: session)
    provider.onApplied = { [weak self] in self?.render($0) }
    return provider
  }()
  private var servicePanel: UIViewController?
  private var replyPanel: UIHostingController<ReplyKeyboardView>?
  private weak var compositionContainer: UIView?
  private let replyModel = ReplyKeyboardModel()
  private var personalDictionaryTimer: Timer?
  private var synchronizingPersonalDictionary = false
  private let preeditButton = UIButton()
  private let exitLocalModeButton = UIButton()
  /// 漢 on the candidate row: the touch counterpart of a Korean keyboard's Hanja key. Shown only while a Korean syllable composes; it lists the syllable's Hanja on the strip and closes the list again.
  private let hanjaButton = UIButton()
  private var localModeTrigger: String?
  private var standardRowHeights: [(UIView, NSLayoutConstraint)] = []
  private let candidateScrollView = CandidateScrollView()
  private let diagnosticLabel = UILabel()
  private let expandCandidatesButton = UIButton()
  private let candidateStack = UIStackView()
  private let candidateEmptySpacer = UIView()
  private let schemeButton = UIButton()
  private let shortcutBar = UIStackView()
  private var candidateContent: UIStackView?
  private let scriptShortcut = UIButton()
  /// 工具栏常驻的「回复」按钮，打开或收起高情商回复面板。
  private let replyShortcut = UIButton()
  private let emojiShortcut = UIButton()
  private let skinShortcut = UIButton()
  private let layoutShortcut = UIButton()
  /// Optional buttons from 工具栏按钮 (`touch_toolbar`), hidden unless the user pins them; each is also in the 更多 panel.
  private let clipboardShortcut = UIButton()
  private let aiShortcut = UIButton()
  private let characterSetShortcut = UIButton()
  private let fullwidthShortcut = UIButton()
  private let punctuationShortcut = UIButton()
  private var clipboardPanel: KeyboardClipboardView?
  private var skinPicker: KeyboardSkinPickerView?
  private var schemePicker: KeyboardSchemePickerView?
  private let moreShortcut = KeyboardBrandButton()
  private var morePicker: KeyboardMorePickerView?
  private let handwriting = HandwritingInputView()
  private var handwritingResults: [String] = []
  private var handwritingActionHeight: NSLayoutConstraint?
  private var layoutPicker: KeyboardLayoutPickerView?
  private var candidatePanel: KeyboardCandidatePanelView?
  private var candidatePanelGeneration: UInt64?
  private var emojiPicker: KeyboardEmojiPickerView?
  private var symbolPanel: KeyboardSymbolPanelView?
  private var nineKeyHoldPopup: UIView?
  private struct NineKeyGridKey {
    let button: UIButton
    let digit: Int
    let letters: String?
    let numberHint: UILabel?
  }
  private var nineKeyGridKeys: [NineKeyGridKey] = []
  private enum MoreToolsPage { case root, localInput }
  private var moreTools: [KeyboardToolSection] = []
  private var moreToolsPage: MoreToolsPage = .root
  private let dismissShortcut = UIButton()
  private var letterButtons: [(button: UIButton, lowercase: String, hint: UILabel)] = []
  private var microsoftFinalKey: UIButton?
  /// Comma and full stop at the end of the third letter row; shown only on the tablet keyboard.
  private var letterRowPunctuationKeys: [UIButton] = []
  private var formFactor: KeyboardFormFactor { .resolve(traitCollection) }
  private var letterRowViews: [UIView] = []
  /// The iPad digit row above the letters and the Tab key before Q; see `KeyboardLayoutPreference.tabletFullKeys`.
  private var numberRowView: UIStackView?
  private var tabKey: UIButton?
  private var symbolRowViews: [UIView] = []
  /// The four Dachen rows, shown instead of the letter rows while the Zhuyin scheme is active.
  private var zhuyinRowViews: [UIView] = []
  // Symbol keys show the punctuation they actually emit in Chinese mode.
  private var symbolKeyFaces: [(key: UIButton, ascii: String, chinese: String)] = []
  /// 符号页第三排的 `=` 键：藏文方案下换成威利叠写用的 `+`（见 `symbolRowKey`）。
  private weak var tibetanPlusKey: UIButton?
  private var layoutToggleButton: UIButton?
  private weak var shiftButton: UIButton?
  private weak var enterButton: UIButton?
  private weak var spaceButton: UIButton?
  private var cursorMovement = SpaceCursorMovement()
  private var backspaceRepeatTimer: Timer?
  private var didRepeatBackspace = false
  private var hasComposition = false
  private var pairedPunctuation = PairedPunctuationStack()
  /// Japanese conversion keeps the selected candidate in the strip until Return commits it.
  private var japaneseConversionIndex: Int?
  private var isChineseMode = true

  /// The mode a newly opened keyboard starts in, from the shared `default_ime_mode`.
  ///
  /// Only a new keyboard reads it: once the 中/英 key has been pressed, that choice holds for as long as this keyboard lives, and a settings change never flips the mode under the typist. iOS does not tell a keyboard which app it is typing into, so `ime_mode_scope` has nothing to key a per-app memory on and is not used here (see `HostCapabilities::ime_mode_scope`).
  static func startsInChinese(_ preferences: [String: Any]?) -> Bool {
    preferences?["default_ime_mode"] as? String != "english"
  }
  private var inputContext = KeyboardInputContext()
  private var inputScheme: ChineseInputScheme = .quanpin
  private var usesShuangpin: Bool { inputScheme.shuangpinProfile != nil }
  // In an active Quanpin or Shuangpin composition, Shift marks the next letter as Engine helpcode.
  // Idle Chinese input keeps the existing shortcut that switches to English capitalization.
  private var helpcodeCompositionEligible: Bool {
    !visiblePreedit.isEmpty && !isInLocalMode
      && (inputScheme == .quanpin || usesShuangpin)
  }
  private var entersHelpcode: Bool {
    isChineseMode && letterCaseState != .lowercase && helpcodeCompositionEligible
  }
  private var supportsLocalTools: Bool { isChineseMode && inputScheme != .wubi && inputScheme.writesChinese }
  /// Whether the letter keys feed the Korean syllable automaton: their faces are jamo and Shift picks the tense consonants instead of switching to English.
  private var typesKorean: Bool { isChineseMode && inputScheme.isKorean && !isInLocalMode }
  /// Whether the Hanja list of the composing Korean syllable is on the strip. The Engine offers Korean no candidates until MSIME_CONVERT_HANJA (msime_client.h), so a Korean composition with candidates is that list.
  private var koreanHanjaListOpen: Bool { typesKorean && hasComposition && !visibleCandidates.isEmpty }
  /// Whether the Dachen keys are on screen and feed the Zhuyin editor: their faces are bopomofo and tone marks, and each sends the ASCII key the Engine reads for it.
  private var typesZhuyin: Bool { isChineseMode && inputScheme.isZhuyin && !isInLocalMode }
  /// Whether the Zhuyin candidate list is on the strip. The editor offers no candidates while the list is closed, so a Zhuyin composition with candidates is that list.
  private var zhuyinListOpen: Bool { typesZhuyin && hasComposition && !visibleCandidates.isEmpty }
  /// Whether the letter keys feed the Vietnamese word: Shift and Caps Lock give capitals, as in English, instead of switching to English.
  private var typesVietnamese: Bool { isChineseMode && inputScheme.isVietnamese && !isInLocalMode }
  /// 字母键是否在拼藏文威利转写：Shift 和大写锁定给出大写字母，大写字母是另一种拼写（T D N Sh A I U M H 等），不是切到英文。
  private var typesTibetan: Bool { isChineseMode && inputScheme.isTibetan && !isInLocalMode }
  /// 字母键是否按 Shift 给出的大小写交给 Engine（越南语和藏文），而不是把 Shift 当成切到英文。
  private var typesCasedLetters: Bool { isChineseMode && inputScheme.typesCasedLetters && !isInLocalMode }
  /// 正在组的内容是否已经就是正文（韩语音节、注音转换结果、越南语单词、藏文音节），因此回车不是确认而是直接上屏，除非有可选的列表打开着。
  private var composesInPlace: Bool { isChineseMode && inputScheme.composesInPlace && !isInLocalMode }
  private var nineKeyRows: [UIView] = []
  private var actionRow: UIStackView!
  private var actionDeleteButton: UIButton!
  private var actionGlobeButton: UIButton!
  private var globeWidthConstraint: NSLayoutConstraint?
  private var japaneseKeys: JapaneseNineKeyView!
  private weak var japaneseGlobeButton: UIButton?
  private weak var japaneseSpaceButton: UIButton?
  private weak var japaneseReturnButton: UIButton?
  private var japaneseHeight: NSLayoutConstraint!
  private var nineKeyHeight: NSLayoutConstraint!
  private var nineKeySymbolsButton: UIButton!
  private let punctuationStack = UIStackView()
  private var quickPunctuationButton: UIButton!
  private var quickPunctuationWidth: NSLayoutConstraint?
  private var symbolDeleteWidth: NSLayoutConstraint?
  private var standardActionWidths: [NSLayoutConstraint] = []
  private var nineKeyActionWidths: [NSLayoutConstraint] = []
  private let nineKeyContainer = UIStackView()
  private let spellingScrollView = UIScrollView()
  private let spellingStack = UIStackView()
  private var spellingButtons: [UIButton] = []
  private var usesTraditionalOutput = false
  /// 高情商回复面板是否由工具栏的「回复」按钮打开。它是工具而不是方案：打开时盖住键区，关闭后回到原来的键盘，引擎方案始终不变。
  private var replyKeyboardShown = false
  private var reportedStatisticsFailure = false
  /// Key presses waiting for the next write. Counted only while statistics are on, which the store is asked on every appearance; with them off nothing is kept, not even in memory.
  private var keyPresses = TypingKeyCounter()
  private var countsKeyPresses = false
  private var keyPressFlushTimer: Timer?
  private var visiblePreedit = ""
  /// The already-chosen half of a phrase at the front of `visiblePreedit`, which 「候选栏预编辑」 never hides.
  private var visiblePhrasePrefix = ""
  /// The spelling with the Engine's caret drawn in, while the user has moved that caret off the end.
  private var visibleCaretSpelling: String?
  /// The spelling the caret can be moved through and where the caret sits in it, while one is being composed; nil for a Japanese reading, whose conversion owns the caret keys.
  private var editableSpelling: (text: String, caret: Int)?
  /// Whether the current space-bar drag moves the caret inside the composition rather than in the document.
  private var spaceDragEditsComposition = false
  /// The desktop candidate skin the strip draws with, or nil while it follows the keyboard skin (see CandidatePalette).
  private var candidatePalette: CandidatePalette?
  private var candidateRevision: UInt64 = 0
  private var visibleCandidates: [String] = []
  private var visibleCandidateCodes: [String] = []
  private var visibleCandidateGlosses: [String] = []
  /// Offline glosses in the non-English target languages, by language code and then candidate text. Replaced per language by each answer; an entry left over from an earlier composition is still that word's gloss.
  private var candidateTargetGlosses: [String: [String: String]] = [:]
  private var offlineGlossInstalled: (resources: String, languages: Set<String>)?
  private var visibleCandidateAnnotations: [String] = []
  private var visibleCandidateSources: [Int] = []
  private var visibleCandidateFixedPositions: [Int] = []
  private var visibleCandidatePageCount = 0
  private var appliedCandidateColumnWidth: CGFloat = 0
  private var visibleCandidatesAnsweredByPinyinFallback = false
  private var visibleDiagnostic: String?
  private var diagnosticDismissTimer: Timer?
  private var shuangpinKeyHints: [String: String] = [:]
  // What the last commit armed, if anything. These belong to the editor rather than to Engine:
  // a different document, a moved caret, or a session rebuilt while the keyboard was away all
  // mean the gesture is about something else, which is what the editor generation carries.
  // The last snapshot's own answers. Every one of these was in the response the keyboard just
  // received; asking the session again costs a full C ABI round trip per question, and the render
  // path asks several times for every keystroke.
  private var currentLocalMode = "none"
  private var currentNineKeySpellings: [String] = []
  private var appliedLayoutInputs: KeyboardLayoutInputs?
  private var candidateGlossTimer: Timer?
  /// Engine's local mode, as of the last snapshot.
  ///
  /// Every snapshot carries it, and every path that can change it renders one, so this is the
  /// same answer the session would give. Asking the session instead costs a full C ABI round trip
  /// - the whole view serialised to JSON and parsed back - and the keystroke path asked more than
  /// thirty times per key before this.
  private var isInLocalMode: Bool { !currentLocalMode.isEmpty && currentLocalMode != "none" }
  private var armedPunctuationRepeat: Any?
  private var armedSpaceConversion: Any?
  private var showsSymbols = false
  private var letterCaseState = LetterCaseState.lowercase
  private var isAutomaticShift = false
  private var lastShiftTapTime: TimeInterval?
  // UIKit sends textWillChange/textDidChange for the keyboard's own edits too, not just for edits
  // the host makes. textWillChange cancels the composition, so every commit that was meant to leave
  // a residual composition running destroyed it a runloop turn later. The proxy is cross-process,
  // so the callback does not arrive inside insertText and a simple set/clear flag is already false
  // by the time it lands — the count has to stay raised until the callback consumes it.
  private var pendingOwnEdits = 0
  /// What 行内预编辑 last wrote into the host as marked text; empty when nothing is marked.
  private var inlineMarkedText = ""

  /// The chips the strip numbers, the page size the session was given (see CandidatePageSizePreference), so a digit picks the chip carrying its number. Everything past it is in the expanded panel.
  private var candidatePageSize: Int {
    CandidatePageSizePreference.clamped(session.sharedPreferences?["candidate_page_size"] as? Int)
  }
  // The composition sits on its own line above the candidates. Both rows are reserved whether or
  // not anything is being composed, so no row appears or disappears mid-typing.
  // Not private: the height assertions derive from it rather than restating the sum.
  static let compositionRowHeight: CGFloat = 32
  private static let candidateStripHeight: CGFloat = compositionRowHeight + 38
  private static let candidateRowHeight: CGFloat = 38
  static let glossLineHeight: CGFloat = 14
  static func glossHeight(lines: Int) -> CGFloat { CGFloat(max(lines, 0)) * glossLineHeight }
  // A larger candidate or composition size grows its row with it, but a smaller one never shrinks the row below the default: the row is also the touch target.
  static func compositionRowHeight(preeditScale: CGFloat) -> CGFloat {
    max(compositionRowHeight, ceil(compositionRowHeight * preeditScale))
  }
  static func candidateRowHeight(candidateScale: CGFloat) -> CGFloat {
    max(candidateRowHeight, ceil(candidateRowHeight * candidateScale))
  }
  static func candidateStripHeight(
    glossLines: Int, candidateScale: CGFloat = 1, preeditScale: CGFloat = 1
  ) -> CGFloat {
    compositionRowHeight(preeditScale: preeditScale) + candidateRowHeight(candidateScale: candidateScale)
      + glossHeight(lines: glossLines)
  }
  /// What the strip adds to the keyboard's base height, which already counts one default candidate row.
  static func stripExtraHeight(
    glossLines: Int, candidateScale: CGFloat = 1, preeditScale: CGFloat = 1
  ) -> CGFloat {
    candidateStripHeight(glossLines: glossLines, candidateScale: candidateScale, preeditScale: preeditScale)
      - candidateRowHeight
  }
  /// Height reserved below the candidate row for composition and configured gloss lines.
  /// Tests and host layout consumers use this contract so the default gloss row stays accounted for.
  static var stripExtraHeight: CGFloat {
    stripExtraHeight(glossLines: configuredGlossLines(fullAccess: false, onlineRoute: false))
  }

  /// `onlineRoute` is whether the shared document picks a translation service at all; without one a language that needs the network can never be filled.
  /// `offline` is the language codes whose offline gloss dictionary is installed.
  static func canFillGloss(_ language: CandidateTranslationLanguage, fullAccess: Bool, onlineRoute: Bool,
                           offline: Set<String> = []) -> Bool {
    !CandidateTranslationPreference.needsNetwork(language, offline: offline)
      || (CandidateTranslationPreference.onlineEnabled && fullAccess && onlineRoute)
  }

  static func configuredGlossLines(fullAccess: Bool, onlineRoute: Bool, offline: Set<String> = []) -> Int {
    guard CandidateGlossPreference.enabled else { return 0 }
    var lines = canFillGloss(CandidateTranslationPreference.primary, fullAccess: fullAccess, onlineRoute: onlineRoute,
                             offline: offline) ? 1 : 0
    if let secondary = CandidateTranslationPreference.secondary,
       canFillGloss(secondary, fullAccess: fullAccess, onlineRoute: onlineRoute, offline: offline) {
      lines += 1
    }
    return lines
  }

  /// The lines reserved under each candidate for a scheme: the configured gloss lines, and in the Korean scheme one more for the Hanja's 훈음, which is drawn there whatever the gloss setting.
  static func stripGlossLines(scheme: ChineseInputScheme, fullAccess: Bool, onlineRoute: Bool,
                              offline: Set<String> = []) -> Int {
    configuredGlossLines(fullAccess: fullAccess, onlineRoute: onlineRoute, offline: offline)
      + (scheme.isKorean ? 1 : 0)
  }
  private var glossLineCount = 0
  private var candidateFontScale: CGFloat = 1
  private var preeditFontScale: CGFloat = 1
  private var candidateFontFamilies: [String] = []
  private var candidateStripHeightConstraint: NSLayoutConstraint?
  private var compositionRowHeightConstraint: NSLayoutConstraint?
  private var shortcutBarTopConstraint: NSLayoutConstraint?

  private var feedbackStrength: KeyboardHapticStrength?
  private var feedbackGenerator: UIImpactFeedbackGenerator?
  private var keyFeedback: UIImpactFeedbackGenerator {
    let strength = KeyboardFeedbackPreference.hapticStrength
    if let feedbackGenerator, feedbackStrength == strength { return feedbackGenerator }
    let generator: UIImpactFeedbackGenerator
    if #available(iOS 17.5, *) {
      if let feedbackGenerator { view.removeInteraction(feedbackGenerator) }
      generator = UIImpactFeedbackGenerator(style: strength.style, view: view)
    } else {
      generator = UIImpactFeedbackGenerator(style: strength.style)
    }
    feedbackStrength = strength
    feedbackGenerator = generator
    return generator
  }

  private let letterRows = [
    Array("qwertyuiop"),
    Array("asdfghjkl"),
    Array("zxcvbnm"),
  ]
  private let symbolRows = [
    Array("1234567890").map(String.init),
    [",", ".", "?", "!", ";", ":", "'", "\"", "@", "/"],
    ["(", ")", "[", "]", "<", ">", "\\", "-", "_", "="],
  ]

  /// Chinese punctuation faces copied from the Engine's punctuation contract.
  ///
  /// The key input remains ASCII so the Engine chooses the punctuation mark (including quote alternation and book-title nesting) and smart punctuation; only the visible face changes. English and local-input modes keep the literal ASCII face.
  static let chineseSymbolFaces: [String: String] = [
    ",": "，", ".": "。", "?": "？", "!": "！", ";": "；", ":": "：",
    "(": "（", ")": "）", "[": "【", "]": "】", "\\": "、",
    "<": "《", ">": "》", "'": "‘", "\"": "“", "_": "——",
  ]

  /// Whether Chinese punctuation is on for marks the keyboard writes itself rather than through the Engine (the Zhuyin symbol panel): a 中文 lock, or the 中文标点 switch under 跟随中英文; an 英文 lock turns it off. Not private: the scheme tests pin it.
  static func writesChinesePunctuation(switchOn: Bool, punctuationLock: String?) -> Bool {
    switch punctuationLock ?? "follow" {
    case "chinese": return true
    case "english": return false
    default: return switchOn
    }
  }

  /// 藏文威利转写里属于拼写的符号，与 Engine 组字时报告的 `spelling_symbols`（`'+-./`）一致：这些键先作为字符交给会话，而不是走标点路由。不是 private：方案测试会固定它。
  static let tibetanSpellingSymbols: Set<String> = ["'", "+", "-", ".", "/"]

  /// 符号页按键实际发出的字符：藏文方案下 `=` 键发 `+`，让组字中的叠写（如 `pad+ma`）经 `handleSymbol` 交给 Engine；符号面板会先上屏组字，不能用来叠写。其他方案原样发出。不是 private：方案测试会固定它。
  static func symbolRowKey(_ symbol: String, tibetan: Bool) -> String {
    tibetan && symbol == "=" ? "+" : symbol
  }

  /// What a Zhuyin symbol-panel key writes: the Chinese mark for an ASCII mark that has one while Chinese punctuation is on, else the key itself. Not private: the scheme tests pin it.
  static func zhuyinSymbolText(_ symbol: String, chinesePunctuation: Bool) -> String {
    chinesePunctuation ? chineseSymbolFaces[symbol] ?? symbol : symbol
  }

  /// Whether the Zhuyin symbol panel writes Chinese marks now.
  private var zhuyinWritesChinesePunctuation: Bool {
    Self.writesChinesePunctuation(switchOn: chinesePunctuation,
                                  punctuationLock: session.sharedPreferences?["punctuation_lock"] as? String)
  }

  override func loadView() {
    inputView = KeyboardInputView(frame: .zero, inputViewStyle: .keyboard)
  }

  override func viewDidLoad() {
    super.viewDidLoad()
    translations.onArrival = { [weak self] in self?.renderCandidateStrip() }
    inputScheme = InputSchemePreference.scheme
    isChineseMode = ImeModeMemoryPreference.startsInChinese(fallback: Self.startsInChinese(session.sharedPreferences))
    appliedCharacterWidth = CharacterWidthPreference.value(in: session.sharedPreferences)
    setFullWidthInput(CharacterWidthPreference.startsFullwidth(in: session.sharedPreferences))
    appliedChinesePunctuation = Self.sharedChinesePunctuation(session.sharedPreferences)
    chinesePunctuation = appliedChinesePunctuation
    synchronizeAICredential()
    applyKeyboardAppearance()
    configureDiagnosticLog()
    DiagnosticLog.shared.write("keyboard_loaded full_access=\(hasFullAccess ? 1 : 0) idiom=\(UIDevice.current.userInterfaceIdiom == .pad ? "pad" : "phone")")
    if session.initializationFailed { DiagnosticLog.shared.write("runtime_initialization_failed") }
    glossLineCount = currentGlossLines()
    usesTraditionalOutput = ChineseOutputPreference.usesTraditional
    _ = applyInputScheme()
    applyLearningPreferences()
    view.backgroundColor = MetasequoiaTheme.keyboardBackground
    skinBackdrop.translatesAutoresizingMaskIntoConstraints = false
    view.insertSubview(skinBackdrop, at: 0)
    NSLayoutConstraint.activate([
      skinBackdrop.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      skinBackdrop.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      skinBackdrop.topAnchor.constraint(equalTo: view.topAnchor),
      skinBackdrop.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
    installKeyboard()
    let height = view.heightAnchor.constraint(equalToConstant:
      260 + currentStripExtraHeight + CGFloat(KeyboardLayoutPreference.heightAdjustment))
    height.priority = .init(999)
    height.identifier = "keyboardHeight"
    height.isActive = true
    keyboardHeightConstraint = height
    updatePreferredKeyboardHeight()
    updateReturnKey()
    updateSpaceKeyTitle()
    // installKeyboard builds the candidate strip before the letter rows exist, so the hints the
    // scheme button gathered there have not reached any key yet.
    updateLetterCaseControls()
    updateCandidateStrip(preedit: "", candidates: [])
    applyKeyboardSkin()
    synchronizeInputContext()
    synchronizeReplyKeyboard()
    // The host going to the background may end the extension without a viewWillDisappear.
    NotificationCenter.default.addObserver(
      self, selector: #selector(flushKeyPresses), name: .NSExtensionHostWillResignActive, object: nil)
  }

  override func viewDidAppear(_ animated: Bool) {
    super.viewDidAppear(animated)
    // UIKit may publish the document identifier and keyboard type one run-loop turn after the
    // extension appears. Refresh the first frame once those host traits are available.
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      // Do not let a delayed proxy callback cancel text typed between appearance and this turn.
      // The next keyboard activation retries synchronization if the host withheld its identifier.
      guard !self.hasComposition else { return }
      self.synchronizeInputContext()
      self.synchronizeInputSchemePreference()
      self.updateLanguageModeButton()
      self.updateLetterCaseControls()
      self.updateCandidateStrip(preedit: self.visiblePreedit, candidates: self.visibleCandidates)
    }
    synchronizeInputContext()
    prepareKeyFeedback()
    synchronizePersonalDictionary(force: true)
    snapshotWorker.tick(idle: !hasComposition && !isInLocalMode, fullAccess: hasFullAccess, force: true)
    personalDictionaryTimer?.invalidate()
    personalDictionaryTimer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
      MainActor.assumeIsolated {
        guard let self else { return }
        self.synchronizePersonalDictionary(force: false)
        self.snapshotWorker.tick(idle: !self.hasComposition && !self.isInLocalMode, fullAccess: self.hasFullAccess)
      }
    }
  }

  override func viewWillAppear(_ animated: Bool) {
    super.viewWillAppear(animated)
    DiagnosticLog.shared.write("focus_in")
    KeyboardUsageReporting.presented(fullAccess: hasFullAccess)
    do { try session.resumeDictionarySession() }
    catch {
      DiagnosticLog.shared.write("dictionary_resume_failed")
      showDiagnostic(error.localizedDescription)
    }
    // A fresh editing session owes us no callbacks. Clearing the count here bounds the damage if
    // UIKit ever skips the delegate pair for one of our own edits: the worst case is that a single
    // host-initiated change is treated as an echo, not a counter that stays raised forever.
    pendingOwnEdits = 0
    inlineMarkedText = ""
    if schemePicker != nil { closeKeyboardPicker() }
    synchronizeInputContext()
    synchronizeInputSchemePreference()
    synchronizeChineseOutputPreference()
    applyLearningPreferences()
    synchronizeTranslationRoute()
    // The Tauri settings app writes the shared PreferencesStore rather than the
    // legacy App Group UserDefaults used by the old SwiftUI settings page.
    // Reload it off-thread so a fuzzy-pinyin change is visible the next time
    // the keyboard appears without blocking UIKit's input lifecycle.
    session.reloadSharedPreferences { [weak self] loaded in
      guard let self else { return }
      // Not applied also covers a document no newer than the one the session has, which is not a failure.
      guard loaded else { DiagnosticLog.shared.write("preferences_not_applied"); return }
      self.configureDiagnosticLog()
      DiagnosticLog.shared.write("preferences_applied")
      // A candidate skin, theme or colour synced from the desktop arrives with the document.
      self.refreshCandidatePalette()
      self.applyKeyboardAppearance()
      self.synchronizeSharedTouchPreferences()
      self.synchronizeCharacterWidth()
      self.synchronizeChinesePunctuation()
      self.synchronizeAICredential()
      self.synchronizeChineseOutputPreference()
      self.applyLearningPreferences()
      self.synchronizeTranslationRoute()
      // The local-mode menu follows the modes the settings app leaves on.
      self.updatePreeditButton()
    }
    candidateGlossEpoch &+= 1
    candidateGlossRequestedGeneration = nil
    translations.cancel()
    onlineCandidates.cancel()
    renderCandidateStrip()
    scheduleCandidateGlosses()
    applyKeyboardSkin()
    synchronizeReplyKeyboard()
    startCountingKeyPresses()
  }

  /// iOS ends a keyboard extension that keeps using too much memory, so a warning is worth a line when a report says the keyboard vanished.
  override func didReceiveMemoryWarning() {
    super.didReceiveMemoryWarning()
    DiagnosticLog.shared.write("memory_warning")
  }

  /// Point the diagnostic log at the shared directory while `diagnostic_log.server` is on, and stop it writing once it is off.
  private func configureDiagnosticLog() {
    let preferences = session.sharedPreferences ?? MetasequoiaInputSessionBridge.loadSharedPreferences()
    DiagnosticLog.shared.configure(directory: session.stateDirectory ?? MetasequoiaInputSessionBridge.sharedStateDirectory,
                                   enabled: DiagnosticLog.isEnabled(in: preferences))
  }

  override func selectionWillChange(_ textInput: UITextInput?) {
    super.selectionWillChange(textInput)
    replyModel.invalidateContext()
    handwriting.clear()
    closeKeyboardService()
  }

  override func textWillChange(_ textInput: UITextInput?) {
    super.textWillChange(textInput)
    replyModel.invalidateContext()
    handwriting.clear()
    closeKeyboardService()
    // Our own edit coming back to us: the composition it produced is still the live one.
    if pendingOwnEdits > 0 {
      pendingOwnEdits -= 1
      return
    }
    // With 行内预编辑 the host already holds the letters: moving the caret out of marked text makes them ordinary text, and clearing the field removes them. Committing the composition as well would write it a second time, so the engine lets it go instead.
    if !inlineMarkedText.isEmpty {
      inlineMarkedText = ""
      textDocumentProxy.unmarkText()
      render(discardComposition())
      return
    }
    // A genuine host-initiated change — the caret moved, the field was cleared, the document was
    // swapped. Commit what is composed rather than discarding it, the way macOS commits on every
    // automatic boundary.
    render(session.finishComposition())
  }

  /// 键盘放弃组字时发 MSIME_CANCEL。就地组字的方案第一次取消可能只退一步（msime_client.h）：关闭韩语汉字列表或注音列表，或把越南语单词、藏文音节退回原始按键（藏文是威利原文）。第二次取消再丢掉剩下的内容，否则 Engine 里会一直留着组字，而宿主已经把它当作输入的文字。
  private func discardComposition() -> MetasequoiaInputSnapshot {
    let inPlace = composesInPlace
    let snapshot = session.cancel()
    return inPlace && !snapshot.preedit.isEmpty ? session.cancel() : snapshot
  }

  override func textDidChange(_ textInput: UITextInput?) {
    super.textDidChange(textInput)
    synchronizeInputContext()
    updateReturnKey()
    updateAutomaticCapitalization()
    synchronizeReplyKeyboard()
  }

  override func viewWillDisappear(_ animated: Bool) {
    super.viewWillDisappear(animated)
    DiagnosticLog.shared.write("focus_out")
    KeyboardUsageReporting.dismissed()
    replyModel.setText("")
    // 回复面板是工具，键盘收起时跟其他面板一起关掉，下次出现回到原来的键盘。
    closeReplyKeyboard()
    handwriting.deactivate()
    snapshotWorker.stop()
    candidateGlossEpoch &+= 1
    candidateGlossRequestedGeneration = nil
    translations.cancel()
    onlineCandidates.cancel()
    closeKeyboardService()
    candidateGlossTimer?.invalidate()
    candidateGlossTimer = nil
    personalDictionaryTimer?.invalidate()
    personalDictionaryTimer = nil
    closeKeyboardPicker()
    cursorMovement.cancel()
    spaceButton?.configuration?.title = "空格"
    // Putting the keyboard away used to drop whatever was composed. macOS commits in
    // prepareForDeactivation: for the same reason: the user typed those letters and never asked to
    // throw them away.
    render(session.finishComposition())
    _ = session.suspendDictionarySession()
    pendingOwnEdits = 0
    cancelBackspacePress()
    diagnosticDismissTimer?.invalidate()
    diagnosticDismissTimer = nil
    keyPressFlushTimer?.invalidate()
    keyPressFlushTimer = nil
    flushKeyPresses()
    // A reused controller must not count presses on its next appearance until that appearance's isEnabled() answer arrives.
    countsKeyPresses = false
  }

  private func installKeyboard() {
    let root = UIStackView()
    keyboardRoot = root
    root.axis = .vertical
    root.spacing = 7
    root.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(root)

    NSLayoutConstraint.activate([
      root.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 5),
      root.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -5),
      root.topAnchor.constraint(equalTo: view.topAnchor, constant: 7),
      root.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -7),
    ])

    root.addArrangedSubview(makeCandidateStrip())
    let numberRow = makeNumberRow()
    numberRowView = numberRow
    root.addArrangedSubview(numberRow)
    for (index, row) in letterRows.enumerated() {
      let rowView = makeLetterRow(row, includesShift: index == letterRows.count - 1)
      letterRowViews.append(rowView)
      root.addArrangedSubview(rowView)
    }
    for (index, row) in ZhuyinKeyLayout.rows.enumerated() {
      let rowView = makeZhuyinRow(row, includesDelete: index == ZhuyinKeyLayout.rows.count - 1)
      rowView.isHidden = true
      zhuyinRowViews.append(rowView)
      root.addArrangedSubview(rowView)
    }
    root.addArrangedSubview(makeNineKeyLayout())
    let japaneseSymbols = makeKey(title: "123", accessibilityLabel: "切换到数字和符号") { [weak self] in
      self?.countKeyPress(TypingKeyID.layer)
      self?.toggleLayout()
    }
    japaneseSymbols.accessibilityIdentifier = "japaneseSymbols"
    let japaneseEmoji = makeKey(title: "^_^", accessibilityLabel: "顔文字と絵文字") { [weak self] in
      self?.countKeyPress(TypingKeyID.emoji)
      self?.showEmojiPicker()
    }
    japaneseEmoji.accessibilityIdentifier = "japaneseEmoji"
    let japaneseLanguage = makeKey(title: "英", accessibilityLabel: "切换中英文") { [weak self] in
      self?.countKeyPress(TypingKeyID.language)
      self?.toggleInputMode()
    }
    japaneseLanguage.accessibilityIdentifier = "japaneseLanguage"
    let japaneseGlobe = makeSymbolKey(symbol: "globe", accessibilityLabel: "选择下一个键盘")
    japaneseGlobe.accessibilityIdentifier = "japaneseGlobe"
    japaneseGlobe.addTarget(
      self, action: #selector(handleInputModeButton(_:event:)), for: .allTouchEvents)
    let japaneseSpace = makeKey(title: "空白", accessibilityLabel: "空白") { [weak self] in
      self?.countKeyPress(TypingKeyID.space)
      self?.handleSpace()
    }
    japaneseSpace.accessibilityIdentifier = "japaneseSpace"
    let japaneseReturn = makeKey(title: "改行", accessibilityLabel: "改行", emphasized: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.enter)
      self?.handleReturn()
    }
    japaneseReturn.accessibilityIdentifier = "japaneseReturn"
    japaneseSpaceButton = japaneseSpace
    japaneseReturnButton = japaneseReturn
    japaneseKeys = JapaneseNineKeyView(makeKey: { [unowned self] title, label, action in
      makeKey(title: title, accessibilityLabel: label, action: action)
    }, makeDelete: { [unowned self] in makeDeleteKey() },
       sideKeys: [japaneseSpace, japaneseReturn],
       modeKeys: [japaneseSymbols, japaneseEmoji, japaneseLanguage, japaneseGlobe])
    japaneseGlobeButton = japaneseGlobe
    japaneseKeys.onKeyPress = { [weak self] index in self?.countKeyPress(TypingKeyID.japaneseKana(index)) }
    japaneseKeys.onInput = { [weak self] input in
      guard let self, isChineseMode, inputScheme.isJapanese else { return }
      playInputClick()
      for character in input { render(session.handleCharacter(String(character))) }
    }
    japaneseKeys.onSymbol = { [weak self] symbol in self?.handleSymbol(symbol) }
    japaneseKeys.onDelete = { [weak self] in self?.handleBackspace() }
    japaneseKeys.onVariant = { [weak self] in
      guard let self, isChineseMode, inputScheme.isJapanese else { return }
      playInputClick()
      render(session.cycleKanaVariant())
    }
    root.addArrangedSubview(japaneseKeys)
    handwriting.isHidden = true
    handwriting.onInsert = { [weak self] text in
      guard let self, inputScheme == .handwriting, isChineseMode else { return }
      render(session.finishComposition())
      insertOwnText(ChineseTextConversion.outputString(text, traditional: usesTraditionalOutput), source: .handwriting)
      playInputClick()
    }
    handwriting.onResults = { [weak self] words in
      guard let self, inputScheme == .handwriting, isChineseMode else { return }
      handwritingResults = words
      updateCandidateStrip(preedit: "", candidates: words)
    }
    handwriting.canDownload = { [weak self] in self?.hasFullAccess == true }
    handwriting.onDelete = { [weak self] in
      self?.countKeyPress(TypingKeyID.backspace)
      self?.handleBackspace()
    }
    root.addArrangedSubview(handwriting)
    for row in symbolRows {
      let rowView = makeSymbolRow(row)
      rowView.isHidden = true
      symbolRowViews.append(rowView)
      root.addArrangedSubview(rowView)
    }
    actionRow = makeActionRow()
    root.addArrangedSubview(actionRow)
    standardRowHeights = ([numberRow] + letterRowViews + zhuyinRowViews + symbolRowViews).map {
      ($0, $0.heightAnchor.constraint(equalTo: actionRow.heightAnchor))
    }
    // Keep the three keypad rows the same height as the bottom controls.
    nineKeyHeight = nineKeyContainer.heightAnchor.constraint(
      equalTo: actionRow.heightAnchor, multiplier: 3, constant: 14)
    // The kana surface now has a dedicated punctuation row beneath the three kana rows.
    japaneseHeight = japaneseKeys.heightAnchor.constraint(
      equalToConstant: KeyboardLayoutPreference.rowSpacing * 3 + 4 * 44)
    // Extra handwriting space belongs to the canvas, not enlarged Space/Return keys.
    handwritingActionHeight = actionRow.heightAnchor.constraint(equalToConstant: 44)
    updateKeyboardLayout()
  }

  /// A key in the nine-key sidebar: text only, on the sidebar's translucent fill. The fill, outline and shadow decorateKey gives a grid key are all removed - a shadow left under a clear key draws as a grey block offset from its title.
  private static func drawBareInSidebar(_ button: UIButton) {
    button.configuration?.background.backgroundColor = .clear
    button.configuration?.background.customView = nil
    button.configuration?.background.strokeWidth = 0
    button.layer.shadowOpacity = 0
  }

  private func makeNineKeyLayout() -> UIView {
    nineKeyContainer.axis = .horizontal
    nineKeyContainer.spacing = 6
    let sidebar = UIView()
    sidebar.accessibilityIdentifier = "nineKeySidebar"
    sidebar.backgroundColor = KeyboardTheme.current.keyBackground.withAlphaComponent(0.5)
    sidebar.layer.cornerRadius = 8
    punctuationStack.axis = .vertical
    punctuationStack.distribution = .fillEqually
    for symbol in ["，", "。", "？", "！"] {
      let button = makeKey(title: symbol, accessibilityLabel: "符号 \(symbol)") { [weak self] in
        self?.countKeyPress(TypingKeyID.punctuation)
        self?.handleSymbol(symbol)
      }
      Self.drawBareInSidebar(button)
      punctuationStack.addArrangedSubview(button)
    }
    for content in [punctuationStack, makeSpellingStrip()] {
      content.translatesAutoresizingMaskIntoConstraints = false
      sidebar.addSubview(content)
      NSLayoutConstraint.activate([
        content.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor),
        content.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor),
        content.topAnchor.constraint(equalTo: sidebar.topAnchor),
        content.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor),
      ])
    }
    nineKeyContainer.addArrangedSubview(sidebar)
    nineSidebarWidth = sidebar.widthAnchor.constraint(equalTo: nineKeyContainer.widthAnchor, multiplier: 0.14)
    nineSidebarWidth?.isActive = true
    let nineKeyGrid = UIStackView()
    nineGrid = nineKeyGrid
    nineKeyGrid.axis = .vertical
    nineKeyGrid.spacing = 7
    nineKeyGrid.distribution = .fillEqually
    nineKeyContainer.addArrangedSubview(nineKeyGrid)
    // Keys 2-9 share their legends with the hold menu below. Key 1 remains the pinyin
    // separator and has no direct English/digit hold option.
    for rowIndex in 0..<3 {
      let row = makeRow()
      for column in 0..<3 {
        let digit = rowIndex * 3 + column + 1
        let letters = Self.nineKeyLetters[digit]
        let button = makeKey(
          title: letters ?? "分词",
          accessibilityLabel: letters.map { "\(digit) \($0)" } ?? "拼音分词"
        ) { [weak self] in
          guard let self else { return }
          self.countKeyPress(TypingKeyID.nineKey(digit))
          if self.showsSymbols { self.handleSymbol(String(digit)) }
          else if letters == nil { self.handleCharacter("'") }
          else { self.handleCharacter(String(digit)) }
        }
        button.accessibilityIdentifier = "nineKey\(digit)"
        if var configuration = button.configuration {
          configuration.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 0, bottom: 0, trailing: 0)
          configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
            var attributes = attributes
            attributes.font = KeyboardTheme.current.usesMonospacedFont
              ? .monospacedSystemFont(ofSize: 21, weight: .medium) : .systemFont(ofSize: 21, weight: .medium)
            return attributes
          }
          button.configuration = configuration
        }
        button.titleLabel?.adjustsFontSizeToFitWidth = true
        button.titleLabel?.minimumScaleFactor = 0.7
        var numberHint: UILabel?
        if let letters {
          let number = UILabel()
          numberHint = number
          number.text = String(digit)
          number.font = .systemFont(ofSize: 10)
          number.textColor = KeyboardTheme.current.accent
          number.accessibilityIdentifier = "keyNumberHint"
          number.translatesAutoresizingMaskIntoConstraints = false
          number.isAccessibilityElement = false
          button.addSubview(number)
          NSLayoutConstraint.activate([
            number.topAnchor.constraint(equalTo: button.topAnchor, constant: 3),
            number.centerXAnchor.constraint(equalTo: button.centerXAnchor),
          ])
          button.tag = digit
          let hold = UILongPressGestureRecognizer(target: self, action: #selector(handleNineKeyHold(_:)))
          hold.minimumPressDuration = 0.3
          button.addGestureRecognizer(hold)
          button.accessibilityHint = "长按输入 \(digit) 或 \(letters)"
        }
        nineKeyGridKeys.append(
          NineKeyGridKey(button: button, digit: digit, letters: letters, numberHint: numberHint))
        row.addArrangedSubview(button)
      }
      nineKeyGrid.addArrangedSubview(row)
      nineKeyRows.append(row)
    }
    let controls = UIStackView()
    nineControls = controls
    controls.axis = .vertical
    controls.spacing = 7
    controls.distribution = .fillEqually
    let delete = makeDeleteKey()
    delete.accessibilityIdentifier = "nineKeyDelete"
    controls.addArrangedSubview(delete)
    let period = makeKey(title: ".", accessibilityLabel: "句点") { [weak self] in
      self?.countKeyPress(TypingKeyID.character("."))
      self?.handleSymbol(".")
    }
    period.configuration?.contentInsets = .zero
    period.accessibilityIdentifier = "nineKeyPeriod"
    controls.addArrangedSubview(period)
    let zero = makeKey(title: "0", accessibilityLabel: "数字 0") { [weak self] in
      self?.countKeyPress(TypingKeyID.nineKey(0))
      self?.handleSymbol("0")
    }
    controls.addArrangedSubview(zero)
    nineKeyContainer.addArrangedSubview(controls)
    controls.widthAnchor.constraint(equalTo: sidebar.widthAnchor).isActive = true
    return nineKeyContainer
  }

  private func applyNineKeyDigitLayer(_ digits: Bool) {
    for key in nineKeyGridKeys {
      key.button.configuration?.title = digits ? String(key.digit) : (key.letters ?? "分词")
      key.button.accessibilityLabel = digits
        ? "数字 \(key.digit)"
        : (key.letters.map { "\(key.digit) \($0)" } ?? "拼音分词")
      key.numberHint?.isHidden = digits
      key.button.accessibilityHint = digits ? nil : key.letters.map { "长按输入 \(key.digit) 或 \($0)" }
    }
  }

  private static let nineKeyLetters: [Int: String] = [
    2: "ABC", 3: "DEF", 4: "GHI", 5: "JKL", 6: "MNO", 7: "PQRS", 8: "TUV", 9: "WXYZ",
  ]

  @objc private func handleNineKeyHold(_ gesture: UILongPressGestureRecognizer) {
    guard gesture.state == .began, let key = gesture.view as? UIButton,
      let letters = Self.nineKeyLetters[key.tag] else { return }
    // The hold replaces the tap, so the press is counted here and the option picked from the menu is not a second one.
    countKeyPress(TypingKeyID.nineKey(key.tag))
    showNineKeyHoldOptions(from: key, digit: key.tag, letters: letters)
  }

  private func showNineKeyHoldOptions(from key: UIButton, digit: Int, letters: String) {
    dismissNineKeyHoldOptions()
    playInputClick()
    let skin = KeyboardTheme.current
    let backdrop = UIView()
    backdrop.accessibilityIdentifier = "nineKeyHoldBackdrop"
    backdrop.backgroundColor = .clear
    backdrop.translatesAutoresizingMaskIntoConstraints = false
    backdrop.addGestureRecognizer(
      UITapGestureRecognizer(target: self, action: #selector(dismissNineKeyHoldOptionsGesture)))

    let options = UIStackView()
    options.axis = .horizontal
    options.spacing = 4
    options.distribution = .fillEqually
    options.accessibilityIdentifier = "nineKeyHoldOptions"
    options.backgroundColor = skin.background
    options.layer.cornerRadius = 10
    options.layer.borderWidth = 1
    options.layer.borderColor = skin.accent.withAlphaComponent(0.3).cgColor
    options.isLayoutMarginsRelativeArrangement = true
    options.layoutMargins = UIEdgeInsets(top: 5, left: 5, bottom: 5, right: 5)
    options.translatesAutoresizingMaskIntoConstraints = false

    for option in [String(digit)] + letters.lowercased().map(String.init) {
      let item = makeKey(title: option, accessibilityLabel: "输入 \(option)") { [weak self] in
        self?.commitNineKeyHoldOption(option)
      }
      item.configuration?.contentInsets = .zero
      item.accessibilityIdentifier = "nineKeyHoldOption-\(option)"
      item.widthAnchor.constraint(equalToConstant: 36).isActive = true
      item.heightAnchor.constraint(equalToConstant: 38).isActive = true
      options.addArrangedSubview(item)
    }

    view.addSubview(backdrop)
    backdrop.addSubview(options)
    let centred = options.centerXAnchor.constraint(equalTo: key.centerXAnchor)
    centred.priority = .defaultHigh
    NSLayoutConstraint.activate([
      backdrop.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      backdrop.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      backdrop.topAnchor.constraint(equalTo: view.topAnchor),
      backdrop.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      options.bottomAnchor.constraint(equalTo: key.topAnchor, constant: -6),
      centred,
      options.leadingAnchor.constraint(greaterThanOrEqualTo: view.leadingAnchor, constant: 6),
      options.trailingAnchor.constraint(lessThanOrEqualTo: view.trailingAnchor, constant: -6),
    ])
    nineKeyHoldPopup = backdrop
    UIAccessibility.post(notification: .layoutChanged, argument: options)
  }

  private func commitNineKeyHoldOption(_ text: String) {
    playInputClick()
    render(session.finishComposition())
    insertOwnText(text)
    dismissNineKeyHoldOptions()
  }

  @objc private func dismissNineKeyHoldOptionsGesture() {
    dismissNineKeyHoldOptions()
  }

  private func dismissNineKeyHoldOptions() {
    nineKeyHoldPopup?.removeFromSuperview()
    nineKeyHoldPopup = nil
  }

  private func makeCandidateStrip() -> UIView {
    let container = UIView()
    compositionContainer = container
    container.accessibilityIdentifier = "candidateStrip"
    container.backgroundColor = Self.stripBackground(KeyboardTheme.current, palette: nil)
    container.layer.cornerRadius = 12

    // The composition gets its own line. Sharing the candidate row cost it up to 28% of the width
    // and left the candidates that much narrower, on the one row where width is worth most.
    let compositionRow = UIView()
    compositionRow.accessibilityIdentifier = "compositionRow"
    compositionRow.translatesAutoresizingMaskIntoConstraints = false
    container.addSubview(compositionRow)

    var preeditConfiguration = UIButton.Configuration.plain()
    preeditConfiguration.contentInsets = NSDirectionalEdgeInsets(
      top: 4, leading: 8, bottom: 4, trailing: 8)
    // Truncate the tail. The head of a spelling is what tells the typist where a long composition
    // went wrong, so dropping it is dropping the useful half.
    preeditConfiguration.titleLineBreakMode = .byTruncatingTail
    preeditConfiguration.baseForegroundColor = KeyboardTheme.current.accent
    preeditConfiguration.titleTextAttributesTransformer = Self.fontTransformer(
      .subheadline, scale: preeditFontScale)
    preeditButton.configuration = preeditConfiguration
    preeditButton.setContentCompressionResistancePriority(.defaultHigh, for: .horizontal)
    preeditButton.showsMenuAsPrimaryAction = true
    preeditButton.accessibilityIdentifier = "preeditButton"

    updateLanguageModeButton()


    updateSchemeButton()
    schemeButton.addAction(UIAction { [weak self] _ in self?.showSchemePicker() }, for: .primaryActionTriggered)


    candidateStack.axis = .horizontal
    candidateStack.spacing = 6
    candidateStack.translatesAutoresizingMaskIntoConstraints = false
    candidateScrollView.showsHorizontalScrollIndicator = false
    candidateScrollView.addSubview(candidateStack)

    diagnosticLabel.font = .preferredFont(forTextStyle: .footnote)
    diagnosticLabel.textColor = MetasequoiaTheme.coneUIColor
    diagnosticLabel.adjustsFontForContentSizeCategory = true
    diagnosticLabel.adjustsFontSizeToFitWidth = true
    diagnosticLabel.minimumScaleFactor = 0.7
    diagnosticLabel.isHidden = true
    diagnosticLabel.accessibilityIdentifier = "diagnosticLabel"

    configureStripButton(
      expandCandidatesButton, symbol: "chevron.down", label: "展开全部候选",
      identifier: "expandCandidates")
    expandCandidatesButton.addAction(
      UIAction { [weak self] _ in self?.showCandidatePanel() },
      for: .primaryActionTriggered)

    preeditButton.translatesAutoresizingMaskIntoConstraints = false
    compositionRow.addSubview(preeditButton)

    let content = UIStackView(arrangedSubviews: [
      candidateScrollView, diagnosticLabel,
      candidateEmptySpacer, expandCandidatesButton, exitLocalModeButton, hanjaButton,
    ])
    content.axis = .horizontal
    content.alignment = .center
    content.spacing = 12
    content.translatesAutoresizingMaskIntoConstraints = false
    container.addSubview(content)
    candidateContent = content
    var exitConfiguration = UIButton.Configuration.plain()
    exitConfiguration.image = UIImage(systemName: "xmark.circle.fill")
    exitConfiguration.contentInsets = .zero
    exitLocalModeButton.configuration = exitConfiguration
    exitLocalModeButton.accessibilityIdentifier = "exitLocalModeButton"
    exitLocalModeButton.accessibilityLabel = "退出本地模式"
    exitLocalModeButton.widthAnchor.constraint(equalToConstant: 38).isActive = true
    exitLocalModeButton.isHidden = true
    exitLocalModeButton.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      render(session.cancel())
    }, for: .primaryActionTriggered)
    // 漢 sits at the trailing end of the candidate row, after the list it opens, so the Hanja scroll beside it and it stays put while the list fills the row.
    var hanjaConfiguration = UIButton.Configuration.plain()
    hanjaConfiguration.title = "漢"
    hanjaConfiguration.baseForegroundColor = KeyboardTheme.current.accent
    hanjaConfiguration.contentInsets = .zero
    hanjaConfiguration.background.cornerRadius = 9
    hanjaConfiguration.titleTextAttributesTransformer = Self.fontTransformer(.body, scale: 1)
    hanjaButton.configuration = hanjaConfiguration
    hanjaButton.accessibilityIdentifier = "hanjaButton"
    hanjaButton.widthAnchor.constraint(equalToConstant: 38).isActive = true
    hanjaButton.isHidden = true
    hanjaButton.addTarget(self, action: #selector(prepareKeyFeedback), for: .touchDown)
    hanjaButton.addAction(UIAction { [weak self] _ in self?.convertKoreanSyllableToHanja() }, for: .primaryActionTriggered)
    installShortcutBar(in: container)

    let stripHeight = container.heightAnchor.constraint(
      equalToConstant: currentStripHeight)
    candidateStripHeightConstraint = stripHeight
    let compositionHeight = compositionRow.heightAnchor.constraint(
      equalToConstant: Self.compositionRowHeight(preeditScale: preeditFontScale))
    compositionRowHeightConstraint = compositionHeight
    NSLayoutConstraint.activate([
      stripHeight,
      compositionRow.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 14),
      compositionRow.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -8),
      compositionRow.topAnchor.constraint(equalTo: container.topAnchor),
      compositionHeight,
      preeditButton.leadingAnchor.constraint(equalTo: compositionRow.leadingAnchor),
      preeditButton.trailingAnchor.constraint(lessThanOrEqualTo: compositionRow.trailingAnchor),
      preeditButton.centerYAnchor.constraint(equalTo: compositionRow.centerYAnchor),
      content.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 14),
      content.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -8),
      content.topAnchor.constraint(equalTo: compositionRow.bottomAnchor),
      content.bottomAnchor.constraint(equalTo: container.bottomAnchor),
      candidateStack.leadingAnchor.constraint(
        equalTo: candidateScrollView.contentLayoutGuide.leadingAnchor),
      candidateStack.trailingAnchor.constraint(
        equalTo: candidateScrollView.contentLayoutGuide.trailingAnchor),
      candidateStack.topAnchor.constraint(
        equalTo: candidateScrollView.contentLayoutGuide.topAnchor),
      candidateStack.bottomAnchor.constraint(
        equalTo: candidateScrollView.contentLayoutGuide.bottomAnchor),
      candidateStack.heightAnchor.constraint(
        equalTo: candidateScrollView.frameLayoutGuide.heightAnchor),
    ])
    return container
  }

  private func installShortcutBar(in container: UIView) {
    shortcutBar.axis = .horizontal
    shortcutBar.distribution = .fill
    shortcutBar.spacing = 0
    shortcutBar.accessibilityIdentifier = "keyboardShortcutBar"
    shortcutBar.translatesAutoresizingMaskIntoConstraints = false
    let brand = moreShortcut
    brand.brandImageView.image = Self.brandTemplate()
      ?? UIImage(systemName: "leaf.fill")?.withRenderingMode(.alwaysTemplate)
    brand.brandImageView.tintColor = KeyboardTheme.current.accent
    shortcutBar.addArrangedSubview(brand)
    brand.widthAnchor.constraint(equalToConstant: 44).isActive = true
    let shortcuts = [layoutShortcut, scriptShortcut, replyShortcut, emojiShortcut, skinShortcut, clipboardShortcut, aiShortcut,
                     characterSetShortcut, fullwidthShortcut, punctuationShortcut, schemeButton, dismissShortcut]
    for button in shortcuts {
      shortcutBar.addArrangedSubview(button)
    }
    for button in shortcuts where button !== schemeButton {
      button.widthAnchor.constraint(equalTo: schemeButton.widthAnchor).isActive = true
    }
    container.addSubview(shortcutBar)
    // The shortcut bar stands in for the candidates, so it takes their row rather than the
    // composition's; the composition line stays reserved either way and nothing shifts when a
    // composition starts.
    let shortcutTop = shortcutBar.topAnchor.constraint(
      equalTo: container.topAnchor, constant: Self.compositionRowHeight(preeditScale: preeditFontScale))
    shortcutBarTopConstraint = shortcutTop
    NSLayoutConstraint.activate([
      shortcutBar.leadingAnchor.constraint(equalTo: container.leadingAnchor),
      shortcutBar.trailingAnchor.constraint(equalTo: container.trailingAnchor),
      shortcutTop,
      shortcutBar.bottomAnchor.constraint(equalTo: container.bottomAnchor),
    ])
    scriptShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      countKeyPress(TypingKeyID.voice)
      showKeyboardVoice()
    }, for: .primaryActionTriggered)
    replyShortcut.addAction(UIAction { [weak self] _ in self?.toggleReplyKeyboard() }, for: .primaryActionTriggered)
    emojiShortcut.addAction(UIAction { [weak self] _ in
      self?.countKeyPress(TypingKeyID.emoji)
      self?.showEmojiPicker()
    }, for: .primaryActionTriggered)
    layoutShortcut.addAction(UIAction { [weak self] _ in self?.showLayoutPicker() }, for: .primaryActionTriggered)
    skinShortcut.addAction(UIAction { [weak self] _ in self?.showSkinPicker() }, for: .primaryActionTriggered)
    clipboardShortcut.addAction(UIAction { [weak self] _ in self?.showClipboardHistory() }, for: .primaryActionTriggered)
    aiShortcut.addAction(UIAction { [weak self] _ in self?.closeKeyboardPicker(); self?.showKeyboardAI() }, for: .primaryActionTriggered)
    characterSetShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      selectTraditionalOutput(!usesTraditionalOutput)
    }, for: .primaryActionTriggered)
    fullwidthShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      setFullWidthInput(!fullWidthInput)
      updateShortcutButtons()
    }, for: .primaryActionTriggered)
    punctuationShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      setChinesePunctuation(!chinesePunctuation)
      updateShortcutButtons()
    }, for: .primaryActionTriggered)
    moreShortcut.addAction(UIAction { [weak self] _ in self?.showMorePicker() }, for: .primaryActionTriggered)
    dismissShortcut.addAction(UIAction { [weak self] _ in self?.dismissKeyboard() }, for: .primaryActionTriggered)
    updateShortcutButtons()
  }

  private func updateShortcutButtons() {
    func configure(_ button: UIButton, title: String?, symbol: String?, label: String, id: String) {
      var configuration = UIButton.Configuration.plain()
      configuration.title = title
      configuration.image = symbol.flatMap { UIImage(systemName: $0) }
      configuration.baseForegroundColor = KeyboardTheme.current.accent
      configuration.contentInsets = .zero
      configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
        var attributes = attributes
        attributes.font = .systemFont(ofSize: 16, weight: .medium)
        return attributes
      }
      button.configuration = configuration
      button.accessibilityLabel = label
      button.accessibilityIdentifier = id
    }
    // 简繁是一次性设置，不占常驻工具位；这个位置只在顶部语音入口开启时出现。简繁切换移进了「更多」，与其他设一次就不再动的设置放在一起。
    configure(scriptShortcut, title: nil, symbol: "waveform", label: "语音结果", id: "layoutVoiceShortcut")
    scriptShortcut.isEnabled = true
    scriptShortcut.accessibilityValue = nil
    scriptShortcut.isHidden = !KeyboardLayoutPreference.voiceShortcutEnabled
    // 回复按钮在任何方案下都常驻；面板打开时它的值读作「已打开」，再点一次收起。
    configure(replyShortcut, title: nil, symbol: "bubble.left.and.text.bubble.right",
      label: "高情商回复", id: "replyShortcut")
    replyShortcut.accessibilityValue = replyPanel != nil ? "已打开" : nil
    configure(emojiShortcut, title: nil, symbol: "face.smiling", label: "表情", id: "emojiShortcut")
    configure(skinShortcut, title: nil, symbol: "tshirt", label: "切换皮肤", id: "skinShortcut")
    skinShortcut.accessibilityValue = KeyboardTheme.current.title
    configure(layoutShortcut, title: nil, symbol: "slider.horizontal.3", label: "键盘设置", id: "layoutShortcut")
    layoutShortcut.accessibilityValue = "默认键位"
    configure(moreShortcut, title: nil, symbol: nil, label: "更多快捷设置", id: "moreShortcut")
    // The switches show their state as a character, the way the Windows floating toolbar does: 简/繁, 全/半, and a Chinese or ASCII comma.
    let pinned = TouchToolbarPreference(in: session.sharedPreferences)
    configure(clipboardShortcut, title: nil, symbol: "doc.on.clipboard", label: "剪贴板历史", id: "clipboardShortcut")
    configure(aiShortcut, title: nil, symbol: "sparkles", label: "AI 润色", id: "aiShortcut")
    configure(characterSetShortcut, title: usesTraditionalOutput ? "繁" : "简", symbol: nil,
              label: "简繁切换", id: "characterSetShortcut")
    characterSetShortcut.accessibilityValue = usesTraditionalOutput ? "繁体" : "简体"
    characterSetShortcut.isEnabled = !(isChineseMode && !inputScheme.writesChinese)
    configure(fullwidthShortcut, title: fullWidthInput ? "全" : "半", symbol: nil,
              label: "全角半角", id: "fullwidthShortcut")
    fullwidthShortcut.accessibilityValue = fullWidthInput ? "全角" : "半角"
    configure(punctuationShortcut, title: chinesePunctuation ? "，" : ",", symbol: nil,
              label: "中英文标点", id: "punctuationShortcut")
    punctuationShortcut.accessibilityValue = chinesePunctuation ? "中文标点" : "英文标点"
    punctuationShortcut.isEnabled = isChineseMode && (inputScheme.writesChinese || inputScheme.isCantonese || inputScheme.isZhuyin)
      && (session.sharedPreferences?["punctuation_lock"] as? String ?? "follow") == "follow"
    layoutShortcut.isHidden = !pinned.layout
    emojiShortcut.isHidden = !pinned.emoji
    skinShortcut.isHidden = !pinned.skin
    clipboardShortcut.isHidden = !pinned.clipboard
    aiShortcut.isHidden = !pinned.ai
    characterSetShortcut.isHidden = !pinned.characterSet
    fullwidthShortcut.isHidden = !pinned.fullwidth
    punctuationShortcut.isHidden = !pinned.punctuation
    moreTools = makeToolSections()
    updateMorePickerPage()
    configure(dismissShortcut, title: nil, symbol: "chevron.down", label: "收起键盘", id: "dismissShortcut")
  }

  /// 开关就摆在面板里,不再藏进二级页。
  ///
  /// 这些开关原来在「键盘设置」卡片后面:面板打开后看到的是六张一模一样的入口卡,要再点一次才知道按键音开没开。面板本来就会滚动,分组标题也已经能区分两类,多出来的那一层只是把状态藏起来。本地输入仍然是二级页 —— 八个模式是一份列表,不是一组开关。
  private func makeToolSections() -> [KeyboardToolSection] {
    [
      KeyboardToolSection(title: nil, kind: .opens, columns: 2, tools: [
        KeyboardTool(title: "表情", symbol: "face.smiling") { [weak self] in self?.showEmojiPicker() },
        KeyboardTool(title: "剪贴板历史", symbol: "doc.on.clipboard") { [weak self] in self?.showClipboardHistory() },
        KeyboardTool(title: "AI 润色", symbol: "sparkles") { [weak self] in
          self?.closeKeyboardPicker(); self?.showKeyboardAI()
        },
        KeyboardTool(title: "语音结果", symbol: "waveform") { [weak self] in
          self?.closeKeyboardPicker(); self?.showKeyboardVoice()
        },
        KeyboardTool(title: "本地输入", symbol: "textformat.123",
                     enabled: supportsLocalTools && !enabledLocalInputModes.isEmpty) { [weak self] in
          self?.showMoreToolsPage(.localInput)
        },
        // 键盘里改得了的只有这一面板上这些。皮肤、词库、账号、统计都在应用里,而用户正打着字,没有别的路走过去。
        KeyboardTool(title: "应用设置", symbol: "gearshape") { [weak self] in
          guard let self else { return }
          closeKeyboardPicker()
          KeyboardAppLauncher.open(KeyboardAppLauncher.settingsURL, from: self)
        },
      ]),
      KeyboardToolSection(title: "设置", kind: .toggle, columns: 2, tools: [
        // 简繁是开关而不是两张选择卡:它本来就是一个布尔值,拆成两张只是多占一行。
        KeyboardTool(title: "繁体输出", symbol: "character.textbox",
                     selected: usesTraditionalOutput,
                     enabled: !(isChineseMode && !inputScheme.writesChinese)) { [weak self] in
          guard let self else { return }
          selectTraditionalOutput(!usesTraditionalOutput)
        },
        KeyboardTool(title: "按键音", symbol: "speaker.wave.2",
                     selected: KeyboardFeedbackPreference.soundEnabled) { [weak self] in
          KeyboardFeedbackPreference.defaults.set(!KeyboardFeedbackPreference.soundEnabled,
                                                   forKey: KeyboardFeedbackPreference.soundKey)
          if KeyboardFeedbackPreference.soundEnabled { UIDevice.current.playInputClick() }
          self?.updateShortcutButtons()
        },
        withHaptics(KeyboardTool(title: "按键振动", symbol: "iphone.radiowaves.left.and.right",
                     selected: KeyboardFeedbackPreference.hapticsEnabled) { [weak self] in
          KeyboardFeedbackPreference.defaults.set(!KeyboardFeedbackPreference.hapticsEnabled,
                                                   forKey: KeyboardFeedbackPreference.hapticsKey)
          if KeyboardFeedbackPreference.hapticsEnabled {
            self?.keyFeedback.impactOccurred(intensity: KeyboardFeedbackPreference.hapticStrength.intensity)
            self?.prepareKeyFeedback()
          }
          self?.updateShortcutButtons()
        }),
        KeyboardTool(title: "全角输入", symbol: "character.cursor.ibeam",
                     selected: fullWidthInput) { [weak self] in
          guard let self else { return }
          self.setFullWidthInput(!self.fullWidthInput)
          self.updateShortcutButtons()
        },
        // A locked punctuation setting decides on its own, and English mode types ASCII marks anyway, so the switch only means something in Chinese mode under 跟随中英文, and only for the schemes that write Chinese marks: those whose marks go through the Engine's punctuation route, and Zhuyin, whose symbol panel picks the mark by the same switch.
        KeyboardTool(title: "中文标点", symbol: "textformat.characters",
                     selected: chinesePunctuation,
                     enabled: isChineseMode && (inputScheme.writesChinese || inputScheme.isCantonese || inputScheme.isZhuyin)
                       && (session.sharedPreferences?["punctuation_lock"] as? String ?? "follow") == "follow") { [weak self] in
          guard let self else { return }
          self.setChinesePunctuation(!self.chinesePunctuation)
          self.updateShortcutButtons()
        },
        withHaptics(KeyboardTool(title: "振动强度", symbol: "waveform",
                     enabled: KeyboardFeedbackPreference.hapticsEnabled,
                     caption: KeyboardFeedbackPreference.hapticStrength.title) { [weak self] in
          guard let self else { return }
          let strengths = KeyboardHapticStrength.allCases
          let current = strengths.firstIndex(of: KeyboardFeedbackPreference.hapticStrength) ?? 0
          let next = strengths[(current + 1) % strengths.count]
          KeyboardFeedbackPreference.defaults.set(next.rawValue,
                                                   forKey: KeyboardFeedbackPreference.strengthKey)
          keyFeedback.impactOccurred(intensity: next.intensity)
          prepareKeyFeedback()
          updateShortcutButtons()
        }),
      ].compactMap { $0 }),
      // Windows 用 Ctrl+Shift+Alt+C 清除候选缓存。iOS 不把硬件键交给第三方键盘,那组快捷键在这里按不出来,所以放成面板里的一张卡。
      KeyboardToolSection(title: "维护", kind: .opens, columns: 2, tools: [
        KeyboardTool(title: "清除候选缓存", symbol: "arrow.counterclockwise") { [weak self] in
          self?.closeKeyboardPicker(); self?.resetCandidateCache()
        },
      ]),
    ]
  }

  private func resetCandidateCache() {
    let snapshot = session.resetCache()
    render(snapshot)
    guard snapshot.diagnosticText == nil else { return }
    showDiagnostic("已清除候选缓存")
    renderCandidateStrip()
  }

  /// Vibration controls only where there is a Taptic Engine to drive; an iPad would show switches that do nothing.
  private func withHaptics(_ tool: KeyboardTool) -> KeyboardTool? {
    KeyboardFeedbackPreference.hapticsAvailable ? tool : nil
  }

  private func makeLocalModeSections() -> [KeyboardToolSection] {
    [
      backToToolsSection(),
      KeyboardToolSection(title: "本地输入", kind: .opens, columns: 2,
                          tools: enabledLocalInputModes.map { mode in
        KeyboardTool(title: mode.title, enabled: supportsLocalTools) { [weak self] in
          self?.closeKeyboardPicker()
          self?.openLocalInputMode(mode.trigger)
        }
      }),
    ]
  }

  private func backToToolsSection() -> KeyboardToolSection {
    KeyboardToolSection(title: nil, kind: .opens, columns: 1, tools: [
      KeyboardTool(title: "返回工具", symbol: "chevron.left") { [weak self] in
        self?.showMoreToolsPage(.root)
      },
    ])
  }

  private func showMoreToolsPage(_ page: MoreToolsPage) {
    moreToolsPage = page
    updateMorePickerPage()
  }

  private func updateMorePickerPage() {
    let sections: [KeyboardToolSection]
    switch moreToolsPage {
    case .root: sections = moreTools
    case .localInput: sections = makeLocalModeSections()
    }
    morePicker?.update(sections: sections)
  }

  private func makeSpellingStrip() -> UIView {
    spellingScrollView.showsVerticalScrollIndicator = false
    // The strip is addressed by this identifier from accessibility and from the tests; it was
    // dropped on the way over from the Apple client, where the same view carries it.
    spellingScrollView.accessibilityIdentifier = "nineKeySpellingStrip"
    spellingScrollView.disableEdgeEffects()
    spellingStack.axis = .vertical
    spellingStack.spacing = 6
    spellingStack.translatesAutoresizingMaskIntoConstraints = false
    spellingScrollView.addSubview(spellingStack)
    NSLayoutConstraint.activate([

      spellingStack.leadingAnchor.constraint(equalTo: spellingScrollView.contentLayoutGuide.leadingAnchor),
      spellingStack.trailingAnchor.constraint(equalTo: spellingScrollView.contentLayoutGuide.trailingAnchor),
      spellingStack.topAnchor.constraint(equalTo: spellingScrollView.contentLayoutGuide.topAnchor),
      spellingStack.bottomAnchor.constraint(equalTo: spellingScrollView.contentLayoutGuide.bottomAnchor),
      spellingStack.widthAnchor.constraint(equalTo: spellingScrollView.frameLayoutGuide.widthAnchor),
    ])
    return spellingScrollView
  }

  private func updateSpellingStrip() {
    let spellings = currentNineKeySpellings
    while spellingButtons.count < spellings.count {
      let button = UIButton(type: .system)
      button.addAction(UIAction { [weak self, weak button] _ in
        guard let self, let button else { return }
        self.playInputClick()
        self.render(self.session.chooseNineKeySpelling(at: UInt(button.tag)))
      }, for: .primaryActionTriggered)
      spellingButtons.append(button)
      spellingStack.addArrangedSubview(button)
    }
    for (index, button) in spellingButtons.enumerated() {
      guard spellings.indices.contains(index) else {
        button.isHidden = true
        continue
      }
      let spelling = spellings[index]
      // Drawn like the punctuation keys that share the sidebar: bare text in the key colour on the sidebar's own fill. The accent is kept for the reading and the chosen candidate.
      var configuration = UIButton.Configuration.plain()
      configuration.title = spelling
      configuration.titleLineBreakMode = .byClipping
      configuration.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 2, bottom: 6, trailing: 2)
      configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
        var attributes = attributes
        attributes.font = .systemFont(ofSize: 14)
        return attributes
      }
      configuration.baseForegroundColor = KeyboardTheme.current.keyForeground
      button.configuration = configuration
      Self.drawBareInSidebar(button)
      button.tag = index
      button.isHidden = false
      button.accessibilityLabel = "选择拼音 \(spelling)"
      button.accessibilityIdentifier = "nineKeySpelling_\(spelling)"
    }
    spellingScrollView.setContentOffset(.zero, animated: false)
    updateKeyboardLayoutIfNeeded()
  }

  /// Digits 1–0 for the full-size iPad keyboard. They go through the same path as the symbol layer's digits, so with a composition open 1–9 pick candidates as on the desktop.
  private func makeNumberRow() -> UIStackView {
    let row = makeRow()
    for digit in "1234567890" {
      let text = String(digit)
      let key = makeKey(title: text, accessibilityLabel: text) { [weak self] in
        self?.countKeyPress(TypingKeyID.character(text))
        self?.handleSymbol(text)
      }
      key.accessibilityIdentifier = "numberRowKey\(text)"
      row.addArrangedSubview(key)
    }
    row.accessibilityIdentifier = "numberRow"
    row.isHidden = true
    return row
  }

  /// A row of Dachen keys. Each key draws its bopomofo symbol or tone mark and sends its ASCII key; the bottom row ends with Backspace, since Dachen takes the whole digit row and the symbol panel keeps its own.
  private func makeZhuyinRow(_ keys: [String], includesDelete: Bool) -> UIStackView {
    let row = makeRow()
    for key in keys {
      let face = ZhuyinKeyLayout.keycap(for: key) ?? key
      let button = makeKey(title: face, accessibilityLabel: "注音 \(face)") { [weak self] in
        self?.countKeyPress(TypingKeyID.character(key))
        self?.handleZhuyinKey(key)
      }
      button.accessibilityIdentifier = "zhuyinKey\(key)"
      row.addArrangedSubview(button)
    }
    if includesDelete {
      let delete = makeDeleteKey()
      delete.accessibilityIdentifier = "zhuyinDelete"
      row.addArrangedSubview(delete)
    }
    row.accessibilityIdentifier = "zhuyinRow"
    return row
  }

  private func makeLetterRow(_ letters: [Character], includesShift: Bool) -> UIStackView {
    let row = makeRow()
    if includesShift {
      let button = makeSymbolKey(symbol: "shift", accessibilityLabel: "大写") { [weak self] in
        self?.countKeyPress(TypingKeyID.shift)
        self?.toggleLetterCase()
      }
      button.accessibilityIdentifier = "shiftButton"
      shiftButton = button
      row.addArrangedSubview(button)
    }
    for letter in letters {
      let text = String(letter)
      let button = makeKey(title: text, accessibilityLabel: text.uppercased()) { [weak self] in
        self?.countKeyPress(TypingKeyID.character(text))
        self?.handleCharacter(text)
      }
      (button as? KeyboardKeyButton)?.showsPressPreview = true
      letterButtons.append((button: button, lowercase: text, hint: attachHintLabel(to: button)))
      row.addArrangedSubview(button)
    }
    if includesShift {
      let delete = makeDeleteKey()
      delete.accessibilityIdentifier = "letterDeleteKey"
      row.addArrangedSubview(delete)
      row.distribution = .fill
      // Keep Shift and Delete easy to hit; distribute the seven letters evenly between them.
      let shift = row.arrangedSubviews[0]
      let keys = Array(row.arrangedSubviews.dropFirst().dropLast())
      NSLayoutConstraint.activate([
        shift.widthAnchor.constraint(equalToConstant: 44),
        delete.widthAnchor.constraint(equalTo: shift.widthAnchor),
      ] + keys.dropFirst().map { $0.widthAnchor.constraint(equalTo: keys[0].widthAnchor) })
      // The iPad system keyboard ends this row with comma and full stop. They start hidden and
      // their widths sit below required so the stack view's own hiding constraint wins on phones.
      for ascii in [",", "."] {
        let chinese = Self.chineseSymbolFaces[ascii] ?? ascii
        let key = makeKey(title: chinese, accessibilityLabel: "符号 \(chinese)") { [weak self] in
          self?.countKeyPress(TypingKeyID.character(ascii))
          self?.handleSymbol(ascii)
        }
        key.accessibilityIdentifier = ascii == "," ? "letterRowCommaKey" : "letterRowPeriodKey"
        key.isHidden = true
        symbolKeyFaces.append((key, ascii, chinese))
        letterRowPunctuationKeys.append(key)
        row.insertArrangedSubview(key, at: row.arrangedSubviews.count - 1)
        let width = key.widthAnchor.constraint(equalTo: keys[0].widthAnchor)
        width.priority = .init(999)
        width.isActive = true
      }
    }
    if letters == letterRows[0] {
      let tab = makeSymbolKey(symbol: "arrow.right.to.line", accessibilityLabel: "Tab") { [weak self] in
        self?.countKeyPress(TypingKeyID.tab)
        self?.handleTab()
      }
      tab.accessibilityIdentifier = "tabKey"
      tab.isHidden = true
      tabKey = tab
      row.insertArrangedSubview(tab, at: 0)
      row.distribution = .fill
      // Letters stay equal; Tab is one and a half keys wide, below required so a hidden Tab leaves the letters to fill the row.
      let keys = Array(row.arrangedSubviews.dropFirst())
      NSLayoutConstraint.activate(keys.dropFirst().map { $0.widthAnchor.constraint(equalTo: keys[0].widthAnchor) })
      let width = tab.widthAnchor.constraint(equalTo: keys[0].widthAnchor, multiplier: 1.5)
      width.priority = .init(999)
      width.isActive = true
    }
    if letters == letterRows[1] {
      let key = makeKey(title: ";", accessibilityLabel: "微软双拼 ing") { [weak self] in
        self?.countKeyPress(TypingKeyID.character(";"))
        self?.handleCharacter(";")
      }
      key.accessibilityIdentifier = "microsoftFinalKey"
      (key as? KeyboardKeyButton)?.showsPressPreview = true
      microsoftFinalKey = key
      letterButtons.append((button: key, lowercase: ";", hint: attachHintLabel(to: key)))
      row.addArrangedSubview(key)
    }
    return row
  }

  // A key is about 36pt wide and the longest Xiaohe mapping is nine characters, so the hint has to
  // be one line that shrinks rather than a subtitle that wraps: wrapping pushed the letter itself
  // out of the top of the key.
  private func attachHintLabel(to button: UIButton) -> UILabel {
    let label = UILabel()
    label.font = .systemFont(ofSize: 9, weight: .regular)
    label.textColor = KeyboardTheme.current.accent.withAlphaComponent(0.8)
    label.textAlignment = .center
    label.numberOfLines = 1
    label.adjustsFontSizeToFitWidth = true
    label.minimumScaleFactor = 0.6
    label.isHidden = true
    label.isAccessibilityElement = false
    label.translatesAutoresizingMaskIntoConstraints = false
    button.addSubview(label)

    NSLayoutConstraint.activate([
      label.leadingAnchor.constraint(equalTo: button.leadingAnchor, constant: 2),
      label.trailingAnchor.constraint(equalTo: button.trailingAnchor, constant: -2),
      label.bottomAnchor.constraint(equalTo: button.bottomAnchor, constant: -2),
    ])
    return label
  }

  private func makeSymbolRow(_ symbols: [String]) -> UIStackView {
    let row = makeRow()
    for symbol in symbols {
      let key = makeKey(title: symbol, accessibilityLabel: "符号 \(symbol)") { [weak self] in
          guard let self else { return }
          let sent = Self.symbolRowKey(symbol, tibetan: typesTibetan)
          countKeyPress(TypingKeyID.character(sent))
          handleSymbol(sent)
        }
      (key as? KeyboardKeyButton)?.showsPressPreview = true
      if let chinese = Self.chineseSymbolFaces[symbol] {
        symbolKeyFaces.append((key, symbol, chinese))
      }
      if Self.symbolRowKey(symbol, tibetan: true) != symbol {
        tibetanPlusKey = key
      }
      row.addArrangedSubview(key)
    }
    return row
  }

  private func makeActionRow() -> UIStackView {
    let row = UIStackView()
    row.axis = .horizontal
    row.alignment = .fill
    row.distribution = .fill
    row.spacing = 6

    let layoutToggle = makeKey(title: "123", accessibilityLabel: "切换到数字和符号", function: true) {
      [weak self] in
      self?.countKeyPress(TypingKeyID.layer)
      self?.toggleLayout()
    }
    if var configuration = layoutToggle.configuration {
      configuration.contentInsets = NSDirectionalEdgeInsets(
        top: 0, leading: 4, bottom: 0, trailing: 4)
      configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
        attributes in
        var attributes = attributes
        attributes.font = .systemFont(ofSize: 17, weight: .medium)
        return attributes
      }
      layoutToggle.configuration = configuration
    }
    layoutToggle.titleLabel?.adjustsFontSizeToFitWidth = true
    layoutToggle.titleLabel?.minimumScaleFactor = 0.7
    layoutToggle.titleLabel?.lineBreakMode = .byClipping
    layoutToggle.accessibilityIdentifier = "layoutToggleButton"
    layoutToggleButton = layoutToggle
    nineKeySymbolsButton = makeKey(title: "符", accessibilityLabel: "符号", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.symbol)
      self?.showSymbolPanel()
    }
    nineKeySymbolsButton.configuration?.contentInsets = .zero
    row.addArrangedSubview(nineKeySymbolsButton)
    row.addArrangedSubview(layoutToggle)

    let globe = makeSymbolKey(symbol: "globe", accessibilityLabel: "选择下一个键盘")
    globe.accessibilityIdentifier = "inputModeSwitchButton"
    globe.addTarget(
      self, action: #selector(handleInputModeButton(_:event:)), for: .allTouchEvents)
    row.addArrangedSubview(globe)

    let delete = makeDeleteKey()
    delete.accessibilityIdentifier = "symbolDeleteKey"
    actionDeleteButton = delete
    row.addArrangedSubview(delete)

    let punctuation = makeKey(title: ",", accessibilityLabel: "常用标点") { [weak self] in
      guard let self else { return }
      // The key itself is counted, wherever it sits: it types the first quick mark of the mode, which is not always a comma, but the heatmap shows the key that went down, as Android and Harmony do.
      countKeyPress(TypingKeyID.quickPunctuation)
      handleSymbol(quickPunctuationSymbols[0])
    }
    punctuation.configuration?.contentInsets = .zero
    punctuation.accessibilityIdentifier = "quickPunctuationKey"
    punctuation.accessibilityHint = "轻点输入，长按选择常用标点"
    quickPunctuationButton = punctuation
    quickPunctuationWidth = punctuation.widthAnchor.constraint(equalToConstant: 44)
    row.addArrangedSubview(punctuation)

    let space = makeKey(title: "空格", accessibilityLabel: "空格") { [weak self] in
      self?.countKeyPress(TypingKeyID.space)
      self?.handleSpace()
    }
    space.accessibilityIdentifier = "spaceKey"
    space.accessibilityHint = "轻点输入空格或选词，左右滑动移动光标"
    space.accessibilityCustomActions = [
      UIAccessibilityCustomAction(name: "光标左移") { [weak self] _ in self?.moveCursor(by: -1); return self != nil },
      UIAccessibilityCustomAction(name: "光标右移") { [weak self] _ in self?.moveCursor(by: 1); return self != nil },
    ]
    let pan = UIPanGestureRecognizer(target: self, action: #selector(handleSpacePan(_:)))
    pan.name = "spaceCursorPan"
    pan.maximumNumberOfTouches = 1
    pan.cancelsTouchesInView = true
    pan.delegate = self
    space.addGestureRecognizer(pan)
    spaceButton = space
    row.addArrangedSubview(space)
    let language = makeKey(title: "中/英", accessibilityLabel: "切换中英文", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.language)
      self?.toggleInputMode()
    }
    language.configuration?.contentInsets = .zero
    language.configuration?.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
      var attributes = attributes
      attributes.font = .systemFont(ofSize: 13, weight: .medium)
      return attributes
    }
    language.accessibilityIdentifier = "bottomLanguageKey"
    language.isHidden = false
    bottomLanguageButton = language
    bottomLanguageWidth = language.widthAnchor.constraint(equalToConstant: 34)
    fullSymbolsWidth = nineKeySymbolsButton.widthAnchor.constraint(equalToConstant: 34)
    row.addArrangedSubview(language)

    let enter = makeKey(title: "换行", accessibilityLabel: "换行", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.enter)
      self?.handleReturn()
    }
    enter.accessibilityIdentifier = "returnKey"
    enter.titleLabel?.adjustsFontSizeToFitWidth = true
    enter.titleLabel?.minimumScaleFactor = 0.65
    enterButton = enter
    row.addArrangedSubview(enter)

    standardActionWidths = [
      layoutToggle.widthAnchor.constraint(equalToConstant: 48.4),
      space.widthAnchor.constraint(greaterThanOrEqualToConstant: 44),
      enter.widthAnchor.constraint(equalToConstant: 59.4),
    ]
    symbolDeleteWidth = delete.widthAnchor.constraint(equalToConstant: 44)
    nineKeyActionWidths = [
      nineKeySymbolsButton.widthAnchor.constraint(equalTo: nineKeyContainer.widthAnchor, multiplier: 0.14),
      layoutToggle.widthAnchor.constraint(equalTo: nineKeySymbolsButton.widthAnchor),
      enter.widthAnchor.constraint(equalTo: nineKeySymbolsButton.widthAnchor, multiplier: 1.3),
    ]
    actionGlobeButton = globe
    return row
  }

  private func makeDeleteKey() -> UIButton {
    let delete = makeSymbolKey(symbol: "delete.left", accessibilityLabel: "删除")
    delete.addTarget(self, action: #selector(beginBackspacePress), for: .touchDown)
    delete.addTarget(self, action: #selector(finishBackspacePress), for: .touchUpInside)
    delete.addTarget(
      self,
      action: #selector(cancelBackspacePress),
      for: [.touchUpOutside, .touchCancel, .touchDragExit])
    return delete
  }

  private func makeRow() -> UIStackView {
    let row = UIStackView()
    row.axis = .horizontal
    row.alignment = .fill
    row.distribution = .fillEqually
    row.spacing = 6
    return row
  }

  private func handleCharacter(_ character: String) {
    playInputClick()
    if isChineseMode {
      synchronizeInputSchemePreference()
      // A host setting can change while this view is open. Do not start an alphabetic composition
      // from a stale 26-key tap after switching to nine keys; local utilities still need letters.
      if inputScheme == .nineKey && !isInLocalMode
        && !("2"..."9").contains(character) && character != "'" {
        return
      }
      // Korean sends the letter under the jamo on the key, in upper case for a tense consonant or an extra vowel, which is how the Engine tells ㄲ from ㄱ. The finished syllable, if this key started a new one, arrives as the snapshot's commit.
      if typesKorean, let key = DubeolsikKeyLayout.keyInput(for: character, shifted: letterCaseState != .lowercase) {
        render(session.handleCharacter(key, shifted: key != character))
        if letterCaseState == .shifted {
          letterCaseState = .lowercase
          lastShiftTapTime = nil
          updateLetterCaseControls()
        }
        return
      }
      // 越南语和藏文按 Shift 或大写锁定给出的大小写取字母，与英文一样：越南语的 Engine 在声调和元音符号里保留这个大小写，藏文威利转写的大写字母本身就是另一个字母（如 `T` 是 ཊ、`A` 是长元音）。
      if typesCasedLetters, let letter = character.first, letter.isLetter {
        let shifted = letterCaseState != .lowercase
        render(session.handleCharacter(shifted ? character.uppercased() : character, shifted: shifted))
        if letterCaseState == .shifted {
          letterCaseState = .lowercase
          lastShiftTapTime = nil
          updateLetterCaseControls()
        }
        return
      }
      if entersHelpcode, let letter = character.first, letter.isLetter {
        render(session.handleCharacter(character.uppercased(), shifted: true))
        if letterCaseState == .shifted {
          letterCaseState = .lowercase
          lastShiftTapTime = nil
          updateLetterCaseControls()
        }
        return
      }
      render(session.handleCharacter(character))
    } else {
      let output = letterCaseState == .lowercase ? character : character.uppercased()
      insertDirectText(output)
      refreshEnglishSuggestions()
      if letterCaseState == .shifted {
        letterCaseState = .lowercase
        lastShiftTapTime = nil
        updateLetterCaseControls()
      }
    }
  }

  /// A Dachen key: its ASCII key goes to the Zhuyin editor, which types the symbol, marks the tone or, for Space after a toned syllable, opens the list. With the list open a digit 1-9 would pick a row instead, so the list is closed first and the key types what its face shows; rows are picked on the strip. A key the editor leaves unclaimed (a tone key with nothing composing) is the host's to type, so its ASCII key is inserted after whatever the editor committed, as on a hardware keyboard and on Android.
  private func handleZhuyinKey(_ key: String) {
    playInputClick()
    guard isChineseMode else { return }
    synchronizeInputSchemePreference()
    guard typesZhuyin else { return }
    if zhuyinListOpen && ZhuyinKeyLayout.selectsWhileListOpen(key) { render(session.cancel()) }
    let snapshot = session.handleCharacter(key)
    render(snapshot)
    if !snapshot.isHandled { insertDirectText(key) }
  }

  /// Read the current word from the host document instead of maintaining a shadow
  /// buffer; autocorrect, cursor movement and external edits then stay truthful.
  private var englishWordBeforeCursor: String {
    guard !isChineseMode, EnglishSuggestionsPreference.isEnabled else { return "" }
    let before = textDocumentProxy.documentContextBeforeInput ?? ""
    return EnglishSuggestionPolicy.currentWord(before: before)
  }

  private func refreshEnglishSuggestions() {
    let prefix = englishWordBeforeCursor
    guard prefix.count >= 2 else {
      updateCandidateStrip(preedit: "", candidates: [])
      return
    }
    updateCandidateStrip(
      preedit: "", candidates: session.englishCompletions(forPrefix: prefix, limit: 12))
  }

  private func useEnglishSuggestion(at index: Int) {
    guard visibleCandidates.indices.contains(index) else { return }
    let typed = englishWordBeforeCursor
    let startedCapitalized = typed.first?.isUppercase ?? false
    guard let replacement = EnglishSuggestionPolicy.replacement(
      typed: typed, candidate: visibleCandidates[index], startedCapitalized: startedCapitalized)
    else {
      updateCandidateStrip(preedit: "", candidates: [])
      return
    }
    for _ in 0..<replacement.deleteCount { deleteOwnBackward() }
    insertDirectText(replacement.insert)
    updateCandidateStrip(preedit: "", candidates: [])
  }

  /// A digit past the chips on show while composing. The session answers it with an unhandled diagnostic carrying no preedit, which the path below would take for "no composition" and type the digit into the document mid-word, so it does nothing instead. Not private: the page-size tests pin it.
  static func digitHasNoChip(_ digit: String, chips: Int, composing: Bool) -> Bool {
    guard composing, let number = Int(digit) else { return false }
    return number > chips
  }

  private func handleSymbol(_ symbol: String) {
    playInputClick()
    if !isChineseMode {
      // 标点锁定为中文 keeps Chinese marks in English mode too; the shared route decides, as it does in Chinese mode.
      if session.sharedPreferences?["punctuation_lock"] as? String == "chinese",
         let punctuation = KeyboardPunctuationContext.engineInput(for: symbol, japanese: false) {
        let preceding = KeyboardPunctuationContext.precedingScalar(textDocumentProxy.documentContextBeforeInput)
        let snapshot = session.handlePunctuationWithContext(punctuation, preceding: preceding)
        if snapshot.isHandled {
          render(snapshot)
          refreshEnglishSuggestions()
          return
        }
      }
      insertDirectText(symbol)
      refreshEnglishSuggestions()
      return
    }

    // Unicode mode reads a hexadecimal code point, so while it is open its digits are input rather
    // than candidate numbers. Its letters already reach the session through handleCharacter.
    if session.isInUnicodeMode, symbol.count == 1, symbol >= "0", symbol <= "9" {
      render(session.handleCharacter(symbol))
      return
    }

    // Zhuyin spells with the digit row and with , . / ; -, so the symbol panel cannot hand its keys to the Engine: they would type ㄅ or ㄝ or pick a list row. The panel commits the conversion and types the mark itself, as the key draws it: the Chinese face for an ASCII mark that has one while Chinese punctuation is on, the ASCII mark otherwise.
    if typesZhuyin {
      render(session.finishComposition())
      insertDirectText(Self.zhuyinSymbolText(symbol, chinesePunctuation: zhuyinWritesChinesePunctuation))
      return
    }

    // 韩语、越南语和藏文的数字作为字符交给会话。汉字列表打开时数字 1-9 从当前页选字，以已处理返回并把汉字作为上屏；VNI 数字给正在拼的越南语单词加符号；其余情况下数字结束正在拼的音节或单词，以未处理按键的上屏返回，随后再把数字打进去。藏文的数字保持原样，不转成藏文数字。
    if typesKorean || typesCasedLetters, symbol.count == 1, symbol >= "0", symbol <= "9" {
      let snapshot = session.handleCharacter(symbol)
      render(snapshot)
      if !snapshot.isHandled { insertDirectText(symbol) }
      return
    }

    if symbol.count == 1, symbol >= "1", symbol <= "9" {
      // handleCandidateKey numbers from the engine's first candidate, which stops matching the
      // strip as soon as it is showing a later page. Off the first page the digit has to select the
      // absolute index the chip with that number is actually displaying, and a digit with no chip
      // on this page has to do nothing: falling through would hand it to handleCandidateKey and
      // commit a first-page candidate the user cannot see.
      let composing = !visiblePreedit.isEmpty || !visibleCandidates.isEmpty
      if Self.digitHasNoChip(symbol, chips: min(candidatePageSize, visibleCandidates.count), composing: composing) { return }
      let snapshot = session.handleCandidateKey(symbol)
      if !snapshot.isHandled && snapshot.preedit.isEmpty {
        insertDirectText(symbol)
      }
      render(snapshot)
      return
    }

    // 藏文威利转写的 `'`（achung）、`+`（叠写）、`.`（消歧）、`-` 和 `/`（垂符）作为字符交给会话：Engine 在它们属于拼写时收进原文，`/` 上屏「藏文+།」或单独一个「།」。Engine 不收的（例如没有组字时的 `+`）以未处理返回、不带上屏，再按下面的标点路由照常写出 ASCII 标点。
    if typesTibetan, Self.tibetanSpellingSymbols.contains(symbol) {
      let spellingSnapshot = session.handleCharacter(symbol)
      if spellingSnapshot.isHandled {
        render(spellingSnapshot)
        return
      }
    }

    if symbol == "'" {
      let separatorSnapshot = session.handleCharacter(symbol)
      if separatorSnapshot.isHandled {
        render(separatorSnapshot)
        return
      }
    }

    guard let punctuation = KeyboardPunctuationContext.engineInput(
      for: symbol, japanese: inputScheme.isJapanese, asciiMarks: typesKorean || typesCasedLetters) else {
      render(session.finishComposition())
      insertDirectText(symbol)
      return
    }
    let editor = smartPunctuationEditor
    // Pressing the same mark again right after it landed as ASCII means the user wanted the
    // Chinese one after all. The shared layer decides; it declines unless the document still
    // holds exactly what the first press committed.
    let decision = session.smartPunctuationDecision(
      character: punctuation, preceding: precedingCharacter,
      timestampMilliseconds: smartPunctuationNow, editorGeneration: editor,
      repeatSnapshot: armedPunctuationRepeat, spaceSnapshot: nil)
    if let chinese = decision["replace_with"] as? String {
      clearSmartPunctuationArming()
      replacePrecedingCharacter(with: chinese)
      return
    }

    let paired = session.sharedPreferences?["paired_punctuation"] as? Bool ?? true
    // The closing half of a pair the keyboard closed is already to the right of the caret, so its key moves past it rather than writing a second one. Only between compositions: a key pressed mid-spelling commits the spelling with its mark.
    if paired && !hasComposition
      && pairedPunctuation.stepOver(ascii: punctuation, editor: editor, following: textDocumentProxy.documentContextAfterInput) {
      clearSmartPunctuationArming()
      textDocumentProxy.adjustTextPosition(byCharacterOffset: 1)
      return
    }

    let preceding = KeyboardPunctuationContext.precedingScalar(documentContextBeforeComposition)
    var snapshot = session.handlePunctuationWithContext(punctuation, preceding: preceding)
    if snapshot.isHandled {
      let reopened = PairedPunctuationPolicy.reopenQuote(snapshot.commitText, ascii: punctuation, enabled: paired)
      if reopened != snapshot.commitText { snapshot = snapshot.replacingCommit(reopened) }
      render(snapshot)
      let completion = PairedPunctuationPolicy.completion(snapshot.commitText, enabled: paired)
      if let completion { closePair(completion, editor: editor) }
      armSmartPunctuation(punctuation, commit: snapshot.commitText, editor: editor, autoClosedPair: completion != nil)
      return
    }

    // A symbol the session declines is an automatic commit, and macOS resolves those with
    // finish_composition — the leading candidate. commitRaw committed the raw pinyin letters
    // instead, so typing "nihao" then "@" produced "nihao@" rather than "你好@".
    render(session.finishComposition())
    insertDirectText(punctuation)
    // The mark reached the document by this route too, so the follow-up gestures are about it
    // just the same. What insertDirectText actually wrote is what arms them: full-width input
    // rewrites the mark on the way out, and arming the ASCII the key carries would then describe
    // a character that is not there.
    armSmartPunctuation(
      punctuation,
      commit: FullWidthInputPolicy.output(punctuation, enabled: fullWidthInput),
      editor: editor)
  }

  /// 成对标点自动补全: write the closing mark and put the caret back between the two. The Engine has just committed the opening mark, so nothing is composed and the caret move ends nothing.
  private func closePair(_ completion: PairedPunctuationCompletion, editor: UInt64) {
    insertOwnText(completion.closing)
    if completion.opening == "<" { session.balancePairedPunctuationAfterAutoClose(opening: completion.opening) }
    textDocumentProxy.adjustTextPosition(byCharacterOffset: -1)
    pairedPunctuation.push(closing: completion.closing, editor: editor)
  }

  /// Milliseconds on the host's own clock, for the two-second repeat window.
  private var smartPunctuationNow: UInt64 {
    UInt64(Date().timeIntervalSince1970 * 1000)
  }

  /// The editor the gestures belong to. No document identifier means no editor to be sure about,
  /// and 0 never matches a real one, so every armed gesture declines.
  private var smartPunctuationEditor: UInt64 {
    guard let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else {
      return 0
    }
    // The first eight UUID bytes, read directly rather than through hashValue: Swift's hashing is
    // seeded per process, and a value that changes between runs would be a poor thing to compare
    // an armed gesture against. Reserve 0 for "no document".
    let bytes = document.uuid
    let low = [bytes.0, bytes.1, bytes.2, bytes.3, bytes.4, bytes.5, bytes.6, bytes.7]
      .reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    return low == 0 ? 1 : low
  }

  private var precedingCharacter: String? {
    documentContextBeforeComposition?.unicodeScalars.last.map { String($0) }
  }

  /// The text before the caret, without the letters 行内预编辑 has marked there.
  private var documentContextBeforeComposition: String? {
    InlineCompositionPolicy.contextBefore(textDocumentProxy.documentContextBeforeInput, marked: inlineMarkedText)
  }

  /// Remember what this commit makes possible next.
  ///
  /// Only a commit arms anything: a press that left a composition running has not put a mark in
  /// the document for a follow-up gesture to be about.
  private func armSmartPunctuation(_ ascii: String, commit: String?, editor: UInt64, autoClosedPair: Bool = false) {
    guard let commit, !commit.isEmpty, editor != 0 else {
      clearSmartPunctuationArming()
      return
    }
    let armed = session.smartPunctuationArming(
      ascii: ascii, commit: commit, timestampMilliseconds: smartPunctuationNow,
      // With the closing half already to the right of the caret, a space typed next is inside the pair rather than after a finished mark, so the shared layer does not arm the space conversion.
      editorGeneration: editor, autoClosedPair: autoClosedPair)
    armedPunctuationRepeat = armed["repeat"] as? [String: Any]
    armedSpaceConversion = armed["space"] as? [String: Any]
  }

  private func clearSmartPunctuationArming() {
    armedPunctuationRepeat = nil
    armedSpaceConversion = nil
  }

  /// Rewrite the character before the caret, keeping the host's own edit accounting straight.
  private func replacePrecedingCharacter(with text: String) {
    deleteOwnBackward()
    insertOwnText(text)
  }

  private func synchronizeInputContext() {
    guard let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else { return }
    applyInputContext(keyboardType: textDocumentProxy.keyboardType ?? .default, documentIdentifier: document)
  }

  // Explicit UIKit-trait boundary also allows layout/state regression tests without a fake engine.
  func applyInputContext(keyboardType: UIKeyboardType, documentIdentifier: UUID) {
    guard let chinese = inputContext.languageOverride(for: keyboardType, document: documentIdentifier,
                                                      isChinese: isChineseMode) else { return }
    // textWillChange normally finishes in the old field. If UIKit skipped that boundary, never
    // insert its remaining preedit into the new field while changing the keyboard's presentation.
    // The marked text belonged to the old field too, so there is nothing here to clear.
    inlineMarkedText = ""
    render(discardComposition())
    isChineseMode = chinese
    showsSymbols = false
    letterCaseState = .lowercase
    isAutomaticShift = false
    lastShiftTapTime = nil
    updateLanguageModeButton()
    updateAutomaticCapitalization()
    updateCandidateStrip(preedit: "", candidates: [])
  }

  private func toggleInputMode() {
    playInputClick()
    let snapshot = isChineseMode ? endComposition(at: .modeSwitch) : session.cancel()
    render(snapshot)
    isChineseMode.toggle()
    if !inputContext.isInLatinField {
      ImeModeMemoryPreference.record(chinese: isChineseMode)
    }
    if isChineseMode && chinesePunctuation != appliedChinesePunctuation {
      setChinesePunctuation(appliedChinesePunctuation)
    }
    letterCaseState = .lowercase
    isAutomaticShift = false
    lastShiftTapTime = nil
    updateLanguageModeButton()
    updateAutomaticCapitalization()
  }

  private func toggleLetterCase() {
    // 韩语的 Shift 是布局自己的 Shift：选出 ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ，而不是切到英文。越南语和藏文的 Shift 与英文一样给出大写字母。
    if isChineseMode && !helpcodeCompositionEligible && !typesKorean && !typesCasedLetters {
      toggleInputMode()
      letterCaseState = .lowercase
    }

    playInputClick()
    isAutomaticShift = false
    let now = ProcessInfo.processInfo.systemUptime
    if letterCaseState == .shifted,
      let lastShiftTapTime,
      now - lastShiftTapTime <= 0.35
    {
      letterCaseState = .capsLock
    } else {
      letterCaseState = letterCaseState == .lowercase ? .shifted : .lowercase
    }
    self.lastShiftTapTime = now
    updateLetterCaseControls()
  }

  private func updateAutomaticCapitalization() {
    // 越南语用拉丁字母书写，所以单词与英文一样跟随输入框的自动大写。藏文不跟随：威利转写的大写字母是另一种拼写，自动大写会把句首的 `ka` 变成 `Ka`。
    guard !isChineseMode || typesVietnamese else {
      isAutomaticShift = false
      updateLetterCaseControls()
      return
    }
    guard letterCaseState != .capsLock else {
      updateLetterCaseControls()
      return
    }

    let mode: EnglishCapitalizationMode
    let capitalization = (inputContext.keyboardType == .URL || inputContext.keyboardType == .emailAddress)
      ? UITextAutocapitalizationType.none : (textDocumentProxy.autocapitalizationType ?? .sentences)
    switch capitalization {
    case .none:
      mode = .none
    case .words:
      mode = .words
    case .sentences:
      mode = .sentences
    case .allCharacters:
      mode = .allCharacters
    @unknown default:
      mode = .sentences
    }

    isAutomaticShift = EnglishCapitalizationPolicy.shouldShift(
      for: mode,
      contextBeforeInput: textDocumentProxy.documentContextBeforeInput)
    letterCaseState = isAutomaticShift ? .shifted : .lowercase
    lastShiftTapTime = nil
    updateLetterCaseControls()
  }

  private func updateLetterCaseControls() {
    // 拼音和罗马字的键面用大写，切到英文才回小写。键面大小写通常只是外观；组合中的
    // Shift 通过无障碍标签显示辅码状态，并把下一字母作为大写辅码交给 Engine。
    // 本地模式除外：那里敲入的就是键面上的字面字符，保持小写才不会误导用户。
    let casedLetters = typesCasedLetters
    let shifted = letterCaseState != .lowercase && (!isChineseMode || entersHelpcode || casedLetters)
    // Read once, not once per key. `isInLocalMode` looks like a property and is a full C ABI
    // round trip: it serialises the whole view - preedit, every candidate, its codes and glosses -
    // to JSON in Rust and parses it back in Swift. Asking for it inside the loop below made that
    // happen twenty-seven times for every keystroke.
    let inLocalMode = isInLocalMode
    // 越南语和藏文的键面与英文一样显示实际要打的大小写。
    let usesUppercase = (isChineseMode && !inLocalMode && !casedLetters) || shifted
    let korean = typesKorean
    let koreanShifted = korean && letterCaseState != .lowercase
    for (button, lowercase, hintLabel) in letterButtons {
      // Korean draws the jamo each key types, with the doubled consonants and extra vowels while Shift is on.
      if korean, let jamo = DubeolsikKeyLayout.keycap(for: lowercase, shifted: koreanShifted) {
        if var configuration = button.configuration {
          configuration.title = jamo
          configuration.contentInsets = .zero
          button.configuration = configuration
        }
        hintLabel.text = nil
        hintLabel.isHidden = true
        button.accessibilityLabel = "字母 \(jamo)"
        button.accessibilityValue = nil
        continue
      }
      // A hint only means something while the key feeds a double-pinyin composition, so English
      // mode drops it even though the scheme underneath is unchanged.
      let hint = isChineseMode && !inLocalMode ? shuangpinKeyHints[lowercase.uppercased()] : nil
      if var configuration = button.configuration {
        configuration.title = usesUppercase ? lowercase.uppercased() : lowercase
        // The hint sits along the bottom edge, so the letter is lifted clear of it instead of
        // staying centred in the whole key.
        configuration.contentInsets = NSDirectionalEdgeInsets(
          top: 0, leading: 0, bottom: hint == nil ? 0 : 11, trailing: 0)
        button.configuration = configuration
      }
      hintLabel.text = hint
      hintLabel.isHidden = hint == nil
      button.accessibilityLabel =
        shifted
        ? "大写 \(lowercase.uppercased())" : "字母 \(lowercase.uppercased())"
      button.accessibilityValue = hint
    }

    guard let button = shiftButton, var configuration = button.configuration else { return }
    switch letterCaseState {
    case .lowercase:
      configuration.image = UIImage(systemName: "shift")
      configuration.background.backgroundColor = KeyboardTheme.current.functionKeyBackground
      button.accessibilityLabel = korean ? "双辅音" : isChineseMode && !casedLetters ? "切换到英文大写" : "大写"
      button.accessibilityValue = "关闭"
    case .shifted:
      configuration.image = UIImage(systemName: "shift.fill")
      configuration.background.backgroundColor =
        KeyboardTheme.current.accent.withAlphaComponent(0.22)
      button.accessibilityLabel = korean ? "双辅音" : "大写"
      button.accessibilityValue = isAutomaticShift ? "自动开启" : "下一字母"
    case .capsLock:
      configuration.image = UIImage(systemName: "capslock.fill")
      configuration.background.backgroundColor =
        KeyboardTheme.current.accent.withAlphaComponent(0.32)
      button.accessibilityLabel = korean ? "双辅音锁定" : "大写锁定"
      button.accessibilityValue = "开启"
    }
    button.configuration = configuration
    decorateKey(button)
  }

  private func updateLanguageModeButton() {
    var configuration = UIButton.Configuration.filled()
    configuration.title = isChineseMode ? Self.languageKeyTitle(inputScheme) : "英"
    // A function key like 123 in the design (`X('中')`, dc.html L2217); only return is ever emphasized.
    configuration.baseForegroundColor = KeyboardTheme.current.keyForeground
    configuration.baseBackgroundColor = KeyboardTheme.current.functionKeyBackground
    configuration.contentInsets = NSDirectionalEdgeInsets(
      top: 3, leading: 5, bottom: 3, trailing: 5)
    configuration.background.cornerRadius = 8
    configuration.background.backgroundColor = KeyboardTheme.current.functionKeyBackground
    bottomLanguageButton?.configuration = configuration
    if let button = bottomLanguageButton { decorateKey(button) }
    bottomLanguageButton?.accessibilityIdentifier = "bottomLanguageKey"
    bottomLanguageButton?.accessibilityLabel =
      isChineseMode ? "切换到英文输入" : "切换到所选输入方案"
    bottomLanguageButton?.accessibilityValue = isChineseMode
      ? Self.languageKeyValue(inputScheme) : "英文输入"
    updateShortcutButtons()
    updateKeyboardLayout()
  }

  /// The face of the 中/英 key while the scheme is on: the language it writes, with Cantonese and Zhuyin named apart from Mandarin. Not private: the scheme tests pin it.
  static func languageKeyTitle(_ scheme: ChineseInputScheme) -> String {
    if scheme.isJapanese { return "日" }
    switch scheme {
    case .korean: return "한"
    case .cantonese: return "粤"
    case .zhuyin: return "注"
    case .vietnamese: return "越"
    case .tibetan: return "藏"
    default: return "中"
    }
  }

  /// The accessibility value of the 中/英 key while the scheme is on.
  static func languageKeyValue(_ scheme: ChineseInputScheme) -> String {
    if scheme.isJapanese { return "日语输入" }
    switch scheme {
    case .korean: return "韩语输入"
    case .cantonese: return "粤语输入"
    case .zhuyin: return "注音输入"
    case .vietnamese: return "越南语输入"
    case .tibetan: return "藏文输入"
    default: return "中文输入"
    }
  }

  /// Japanese names its space key by the action it performs: 空白 while idle and 変換 while
  /// choosing a candidate. Other schemes keep the shared 空格 label.
  private func updateSpaceKeyTitle() {
    let title = inputScheme.isJapanese ? (hasComposition ? "変換" : "空白") : "空格"
    if var configuration = japaneseSpaceButton?.configuration,
       configuration.title != title {
      configuration.title = title
      japaneseSpaceButton?.configuration = configuration
      japaneseSpaceButton?.accessibilityLabel = title
    }
    if var configuration = spaceButton?.configuration,
       configuration.title != title {
      configuration.title = title
      spaceButton?.configuration = configuration
      spaceButton?.accessibilityLabel = title
    }
    japaneseKeys?.setComposing(hasComposition)
  }

  private func updateReturnKey() {
    let title: String
    switch textDocumentProxy.returnKeyType ?? .default {
    case .default:
      title = "换行"
    case .go:
      title = "前往"
    case .google, .search, .yahoo:
      title = "搜索"
    case .join:
      title = "加入"
    case .next:
      title = "下一项"
    case .route:
      title = "路线"
    case .send:
      title = "发送"
    case .done:
      title = "完成"
    case .emergencyCall:
      title = "紧急呼叫"
    case .continue:
      title = "继续"
    @unknown default:
      title = "换行"
    }

    // 回车上屏正在组的内容而不是执行输入框的动作，所以键面要写明。韩语音节和越南语单词已经是正文：回车上屏它们后仍执行输入框的动作，所以键面保留输入框给的名字，除非音节的汉字列表打开着，那时回车只选汉字。注音转换结果只由回车上屏，与大千键盘一样，所以那里显示确认。藏文也一样：回车只上屏转出的藏文、不加音节点，运行时以已处理返回，不换行，所以显示确认。
    let confirms = hasComposition && (!returnKeepsFieldAction || koreanHanjaListOpen)
    let shownTitle = !confirms ? title : inputScheme.isJapanese ? "確定" : "确认"
    if var configuration = japaneseReturnButton?.configuration {
      let japaneseTitle = hasComposition ? "確定" : "改行"
      if configuration.title != japaneseTitle {
        configuration.title = japaneseTitle
        japaneseReturnButton?.configuration = configuration
        japaneseReturnButton?.accessibilityLabel = japaneseTitle
      }
    }
    if var configuration = enterButton?.configuration {
      configuration.title = shownTitle
      enterButton?.configuration = configuration
    }
    enterButton?.accessibilityLabel = shownTitle
    styleReturnKey()
  }

  /// Whether Return commits the open composition and then still does the field's action: Korean and Vietnamese, whose runtime answers the commit unhandled.
  private var returnKeepsFieldAction: Bool {
    isChineseMode && (inputScheme.isKorean || inputScheme.isVietnamese)
  }

  /// Return is a function key at rest and fills with the accent only while it commits a composition (`X(..., hasComp)`, dc.html L2217). A keyboard design keeps its own action colour throughout.
  private func styleReturnKey() {
    guard let enterButton, var configuration = enterButton.configuration else { return }
    let skin = KeyboardTheme.current
    guard skin.design == nil else { return }
    // A Korean syllable or a Vietnamese word is nearly always open while typing, and Return does not merely confirm it, so the key stays a function key until the syllable's Hanja list opens.
    let emphasized = hasComposition && (!returnKeepsFieldAction || koreanHanjaListOpen)
    let background = emphasized ? skin.actionBackground : skin.functionKeyBackground
    let foreground = emphasized ? skin.actionForeground : skin.keyForeground
    guard configuration.background.backgroundColor != background || configuration.baseForegroundColor != foreground
    else { return }
    configuration.background.backgroundColor = background
    configuration.baseForegroundColor = foreground
    enterButton.configuration = configuration
  }

  private func applyLearningPreferences() {
    let nativeMode: MetasequoiaFrequencyAdjustmentMode
    switch FrequencyAdjustmentPreference.mode {
    case .disabled: nativeMode = .disabled
    case .pin: nativeMode = .pin
    case .halve: nativeMode = .halve
    case .linear: nativeMode = .linear
    case .promote: nativeMode = .promote
    }
    var mode = nativeMode
    var triggerCount = FrequencyAdjustmentPreference.triggerCount
    var linearStep = FrequencyAdjustmentPreference.linearStep
    // The shared PreferencesStore is the canonical source; the App Group copy only stands in until the first document reload.
    if let frequency = session.sharedPreferences?["frequency"] as? [String: Any] {
      switch frequency["mode"] as? String {
      case "pin": mode = .pin
      case "halve": mode = .halve
      case "linear": mode = .linear
      case "promote": mode = .promote
      case "disabled": mode = .disabled
      default: break
      }
      if let value = Self.sharedPreferenceInt(frequency["trigger_count"], range: FrequencyAdjustmentPreference.countRange) {
        triggerCount = value
      }
      if let value = Self.sharedPreferenceInt(frequency["linear_step"], range: FrequencyAdjustmentPreference.countRange) {
        linearStep = value
      }
    }
    _ = session.setFrequencyAdjustmentMode(
      mode, triggerCount: triggerCount, linearStep: linearStep)
    _ = session.setLearningEnabled(DictionaryLearningPreference.enabled)
    let fuzzyRules = FuzzyPinyinPreference.settings(in: session.sharedPreferences)?.bits ?? 0
    if session.fuzzyPinyinRulesApplied != fuzzyRules {
      _ = session.setFuzzyPinyinRules(fuzzyRules)
    }
    session.setWubiMixedPinyin(WubiMixedPinyinPreference.isEnabled)
    applyCandidateGlossLayout()
  }

  private func currentGlossLines() -> Int {
    Self.stripGlossLines(scheme: inputScheme, fullAccess: hasFullAccess, onlineRoute: translationRoute != .none,
                         offline: offlineGlossLanguages)
  }

  /// Whether the shared gloss path serves this scheme's candidates: Chinese ones, and the Hanja rows of the Korean scheme. Japanese is left out, as the Engine's translation query leaves it out.
  private var glossesCandidates: Bool { inputScheme.writesChinese || inputScheme.isKorean }

  private var offlineGlossLanguages: Set<String> {
    guard let resources = session.candidateGlossResources() else { return [] }
    if let installed = offlineGlossInstalled, installed.resources == resources { return installed.languages }
    let languages = CandidateTranslationPreference.offlineGlossLanguages(resources: resources)
    offlineGlossInstalled = (resources, languages)
    return languages
  }

  private var currentStripHeight: CGFloat {
    Self.candidateStripHeight(
      glossLines: glossLineCount, candidateScale: candidateFontScale, preeditScale: preeditFontScale)
  }

  private var currentStripExtraHeight: CGFloat {
    Self.stripExtraHeight(
      glossLines: glossLineCount, candidateScale: candidateFontScale, preeditScale: preeditFontScale)
  }

  /// Gloss lines and the candidate and composition sizes all decide how tall the strip is, so they are applied together.
  private func applyCandidateGlossLayout() {
    let lines = currentGlossLines()
    let tablet = formFactor == .tablet
    let candidateScale = CandidateFontPreference.candidateScale(in: session.sharedPreferences, tablet: tablet)
    let preeditScale = CandidateFontPreference.preeditScale(in: session.sharedPreferences, tablet: tablet)
    let families = CandidateFontPreference.families(in: session.sharedPreferences)
    guard lines != glossLineCount || candidateScale != candidateFontScale
      || preeditScale != preeditFontScale || families != candidateFontFamilies else { return }
    candidateFontFamilies = families
    glossLineCount = lines
    if preeditScale != preeditFontScale {
      preeditFontScale = preeditScale
      preeditButton.configuration?.titleTextAttributesTransformer = Self.fontTransformer(
        .subheadline, scale: preeditScale)
    }
    candidateFontScale = candidateScale
    candidateStripHeightConstraint?.constant = currentStripHeight
    compositionRowHeightConstraint?.constant = Self.compositionRowHeight(preeditScale: preeditScale)
    shortcutBarTopConstraint?.constant = Self.compositionRowHeight(preeditScale: preeditScale)
    updatePreferredKeyboardHeight()
    renderCandidateStrip()
  }

  private static func fontTransformer(
    _ style: UIFont.TextStyle, scale: CGFloat, families: [String] = [], weight: UIFont.Weight = .regular
  ) -> UIConfigurationTextAttributesTransformer {
    UIConfigurationTextAttributesTransformer { attributes in
      var attributes = attributes
      attributes.font = weighted(CandidateFontPreference.font(style, scale: scale, families: families), weight)
      return attributes
    }
  }

  /// `font` at `weight`, keeping its family, cascade and size; the regular weight returns it untouched.
  static func weighted(_ font: UIFont, _ weight: UIFont.Weight) -> UIFont {
    guard weight != .regular else { return font }
    var traits = font.fontDescriptor.object(forKey: .traits) as? [UIFontDescriptor.TraitKey: Any] ?? [:]
    traits[.weight] = weight
    return UIFont(descriptor: font.fontDescriptor.addingAttributes([.traits: traits]), size: font.pointSize)
  }

  private func applyInputScheme() -> MetasequoiaInputSnapshot {
    switch inputScheme {
    case .nineKey: session.switchToNineKey()
    case .ziranma, .microsoft, .shoudao: session.switch(toShuangpinProfile: inputScheme.rawValue)
    case .wubi: session.switchToWubi()
    case .japanese, .japaneseNineKey: session.switchToJapanese()
    case .korean: session.switchToKorean()
    case .cantonese: session.switchToCantonese()
    case .zhuyin: session.switchToZhuyin()
    case .vietnamese: session.switchToVietnamese()
    case .tibetan: session.switchToTibetan()
    case .handwriting: session.switch(toShuangpin: false)
    case .quanpin: session.switch(toShuangpin: usesShuangpin)
    case .shuangpin: session.switch(toShuangpinProfile: "xiaohe")
    }
  }

  private func selectInputScheme(_ scheme: ChineseInputScheme, persistShared: Bool = true) {
    guard InputSchemePreference.offeredSchemes.contains(scheme) else { return }
    if scheme == inputScheme { return }
    playInputClick()
    let source = typingSource
    // An open Korean syllable, Zhuyin conversion or Vietnamese word is text the user already wrote, so leaving the scheme commits it; the switch below would otherwise discard it along with the marked text.
    if composesInPlace && hasComposition { render(session.finishComposition(), source: source) }
    handwriting.clear()
    inputScheme = scheme
    let snapshot = applyInputScheme()
    InputSchemePreference.scheme = scheme
    if persistShared {
      _ = session.setTouchKeyboardScheme(scheme, enabledSchemes: InputSchemePreference.enabledSchemes)
    }
    showsSymbols = false
    updateSchemeButton()
    updateLanguageModeButton()
    render(snapshot, source: source)
    updateShortcutButtons()
    // The Korean scheme reserves a line for the 훈음, so the strip height follows the scheme.
    applyCandidateGlossLayout()
    synchronizeReplyKeyboard()
  }

  /// 工具栏「回复」按钮：面板开着就收起回到原键盘，没开就打开。
  private func toggleReplyKeyboard() {
    if replyKeyboardShown { closeReplyKeyboard() } else { openReplyKeyboard() }
  }

  private func openReplyKeyboard() {
    closeKeyboardPicker()
    closeKeyboardService()
    replyKeyboardShown = true
    synchronizeReplyKeyboard()
  }

  private func closeReplyKeyboard() {
    replyKeyboardShown = false
    synchronizeReplyKeyboard()
  }

  private func synchronizeReplyKeyboard() {
    guard replyKeyboardShown else {
      replyModel.resetResults()
      dismissReplyPanel()
      return
    }
    guard replyPanel == nil else { return }
    let panel = UIHostingController(rootView: ReplyKeyboardView(model: replyModel,
      paste: { [weak self] in
        guard let self else { return }
        guard hasFullAccess else { replyModel.status = "粘贴与 AI 需要允许完全访问"; return }
        replyModel.setText(UIPasteboard.general.string ?? "")
      }, generate: { [weak self] style in self?.generateReply(style: style) }))
    replyPanel = panel
    addChild(panel)
    panel.view.accessibilityIdentifier = "replyKeyboard"
    panel.view.accessibilityViewIsModal = true
    panel.view.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(panel.view)
    NSLayoutConstraint.activate([
      panel.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      panel.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      panel.view.topAnchor.constraint(equalTo: compositionContainer?.bottomAnchor ?? view.topAnchor),
      panel.view.bottomAnchor.constraint(equalTo: view.bottomAnchor)
    ])
    panel.didMove(toParent: self)
    updateShortcutButtons()
  }

  private func dismissReplyPanel() {
    guard let panel = replyPanel else { return }
    panel.willMove(toParent: nil)
    panel.view.removeFromSuperview()
    panel.removeFromParent()
    replyPanel = nil
    updateShortcutButtons()
  }

  private func generateReply(style: String) {
    guard hasFullAccess else { replyModel.status = "请在系统键盘设置中允许完全访问"; return }
    guard let configuration = KeyboardAIService.configuration() else {
      replyModel.status = "请在水杉 App → AI 设置中保存键盘 AI 配置"; return
    }
    guard !hasComposition, let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else {
      replyModel.status = "请先完成输入，再选择回复方式"; return
    }
    let context = KeyboardDocumentContext(document: document,
      before: textDocumentProxy.documentContextBeforeInput, selected: textDocumentProxy.selectedText,
      after: textDocumentProxy.documentContextAfterInput)
    let matches: () -> Bool = { [weak self] in
      guard let self, hasFullAccess, replyKeyboardShown,
            KeyboardAIService.configuration() == configuration else { return false }
      return context.matches(document: KeyboardHostContext.documentIdentifier(for: textDocumentProxy),
        before: textDocumentProxy.documentContextBeforeInput, selected: textDocumentProxy.selectedText,
        after: textDocumentProxy.documentContextAfterInput)
    }
    playInputClick()
    replyModel.generate(style: style, request: { text, prompt in
      guard matches() else { throw ServiceFailure(message: "输入位置已变化，请重试") }
      var requestConfiguration = configuration
      requestConfiguration.prompt = prompt
      let result = try await CustomServiceClient.request(kind: .ai, configuration: requestConfiguration,
        text: text, token: KeyboardAIService.token(for: configuration))
      guard matches() else { throw ServiceFailure(message: "输入位置已变化，请重试") }
      return result
    }, insert: { [weak self] result in
      guard let self, matches() else { return false }
      insertOwnText(result, source: .reply)
      // 面板贴满键区，会挡住刚插入的文字和能修改它的退格键——留在屏幕上的唯一删除键改的是粘贴进来的原文。面板自己的状态已经提示去聊天应用里确认，所以插入后它像其他选择器一样收起，回到原来的键盘；工具栏的「回复」按钮能再打开它，粘贴的原文仍保留在模型里。
      closeReplyKeyboard()
      return true
    })
  }

  private func showKeyboardAI() {
    guard hasFullAccess else { showDiagnostic("AI 需要开启键盘的“允许完全访问”。"); return }
    guard let configuration = KeyboardAIService.configuration() else {
      showDiagnostic("请在水杉 App 的 AI 设置中启用键盘 AI 并保存配置。"); return
    }
    guard !hasComposition, let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy),
          let selected = textDocumentProxy.selectedText, !selected.isEmpty, selected.count <= 10_000 else {
      showDiagnostic("请先完成输入，再选中要润色的文字（最多一万字）。"); return
    }
    closeKeyboardPicker()
    closeKeyboardService()
    let selection = KeyboardDocumentContext(document: document, before: textDocumentProxy.documentContextBeforeInput,
                                         selected: selected, after: textDocumentProxy.documentContextAfterInput)
    let matches: () -> Bool = { [weak self] in
      guard let self, hasFullAccess, servicePanel != nil else { return false }
      return selection.matches(document: KeyboardHostContext.documentIdentifier(for: textDocumentProxy),
        before: textDocumentProxy.documentContextBeforeInput, selected: textDocumentProxy.selectedText,
        after: textDocumentProxy.documentContextAfterInput)
    }
    let panel = UIHostingController(rootView: KeyboardAIView(text: selected, configuration: configuration,
      canSend: matches, insert: { [weak self] result in
        guard let self, matches(), KeyboardAIService.configuration() == configuration else { return false }
        insertOwnText(result, source: .ai)
        return true
      }, close: { [weak self] in self?.closeKeyboardService() }))
    servicePanel = panel
    addChild(panel)
    panel.view.frame = view.bounds
    panel.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    view.addSubview(panel.view)
    panel.didMove(toParent: self)
  }

  private func showKeyboardVoice() {
    guard hasFullAccess else { showDiagnostic("语音结果需要开启键盘的“允许完全访问”。"); return }
    guard !hasComposition, !isInLocalMode else { showDiagnostic("请先完成当前输入，再插入语音结果。"); return }
    do {
      let store = VoiceTextHandoffStore()
      let entry = try store.read()
      let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy)
      let context = document.map { KeyboardDocumentContext(document: $0,
        before: textDocumentProxy.documentContextBeforeInput, selected: textDocumentProxy.selectedText,
        after: textDocumentProxy.documentContextAfterInput) }
      closeKeyboardPicker()
      closeKeyboardService()
      let panel = UIHostingController(rootView: KeyboardVoiceView(entry: entry, insert: { [weak self] in
        guard let self, hasFullAccess, servicePanel != nil, let context, let entry,
              context.matches(document: KeyboardHostContext.documentIdentifier(for: textDocumentProxy),
                before: textDocumentProxy.documentContextBeforeInput, selected: textDocumentProxy.selectedText,
                after: textDocumentProxy.documentContextAfterInput) else {
          throw ServiceFailure(message: "输入位置已变化，请关闭后重新打开语音结果。")
        }
        let text = try store.consume(entry.id)
        insertOwnText(text, source: .voice)
      }, close: { [weak self] in self?.closeKeyboardService() }, record: { [weak self] in
        guard let self else { return }
        closeKeyboardService()
        KeyboardAppLauncher.open(KeyboardAppLauncher.voiceURL, from: self)
      }))
      panel.overrideUserInterfaceStyle = KeyboardAppearancePreference.style(KeyboardAppearancePreference.voiceKey, in: session.sharedPreferences)
      servicePanel = panel
      addChild(panel)
      panel.view.frame = view.bounds
      panel.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
      view.addSubview(panel.view)
      panel.didMove(toParent: self)
    } catch { showDiagnostic(error.localizedDescription) }
  }

  private func closeKeyboardService() {
    guard let panel = servicePanel else { return }
    servicePanel = nil
    panel.willMove(toParent: nil)
    panel.view.removeFromSuperview()
    panel.removeFromParent()
  }

  private func synchronizePersonalDictionary(force: Bool) {
    guard hasFullAccess, !hasComposition, !isInLocalMode, !synchronizingPersonalDictionary else { return }
    let store = PersonalDictionaryStore()
    do {
      let state = try store.read()
      guard force || state.pendingCount > 0 || state.refreshID != state.completedRefreshID else { return }
      synchronizingPersonalDictionary = true
      defer { synchronizingPersonalDictionary = false }
      try store.synchronize(apply: { request in
        try session.applyPersonalPrevious(request.previous?.bridgeValue, replacement: request.replacement?.bridgeValue,
                                          requestID: request.id)
      }, page: { request in
        let result = try session.personalEntries(atOffset: UInt(request.offset), kind: request.kind,
                                                 query: request.query)
        guard let rows = result["entries"] as? [[String: Any]], let hasMore = result["hasMore"] as? Bool else {
          throw PersonalDictionaryStore.StoreError.invalidState
        }
        return PersonalWordPage(entries: try rows.map { try PersonalWord(bridgeValue: $0) }, hasMore: hasMore)
      }, export: { request in
        try self.session.personalExport(kind: request.kind, format: request.format)
      })
    } catch PersonalDictionaryStore.StoreError.busy {
      // Another process owns this short transaction; the timer retries without interrupting typing.
    } catch {
      showDiagnostic(error.localizedDescription)
    }
  }

  private func synchronizeInputSchemePreference() {
    guard !hasComposition else { return }
    let sharedValue = InputSchemePreference.scheme
    guard sharedValue != inputScheme else { return }
    let source = typingSource
    inputScheme = sharedValue
    let snapshot = applyInputScheme()
    updateSchemeButton()
    updateLanguageModeButton()
    render(snapshot, source: source)
    applyCandidateGlossLayout()
    synchronizeReplyKeyboard()
  }

  // Reads the same table the writer uses, so a scheme can never be persisted under a spelling this
  // side then fails to recognise.
  private static func sharedInputScheme(_ value: String) -> ChineseInputScheme? {
    ChineseInputScheme.scheme(sharedIdentifier: value)
  }

  /// Apply settings written by the Tauri iOS host to the native keyboard's
  /// legacy App Group preferences. Scheme changes are intentionally deferred
  /// while composing so a settings reload cannot interrupt Engine state.
  private func synchronizeSharedTouchPreferences() {
    guard let preferences = session.sharedPreferences else { return }
    // The Tauri host stores learning and frequency settings in the canonical
    // PreferencesStore. Keep the legacy App Group values in sync because the
    // keyboard's native settings and compatibility paths still read them.
    if let learning = preferences["learning"] as? Bool,
       learning != DictionaryLearningPreference.enabled {
      KeyboardFeedbackPreference.defaults.set(learning, forKey: DictionaryLearningPreference.key)
    }
    if let frequency = preferences["frequency"] as? [String: Any] {
      if let rawMode = frequency["mode"] as? String,
         let mode = FrequencyAdjustmentMode(rawValue: rawMode) {
        FrequencyAdjustmentPreference.mode = mode
      }
      // Ignore values outside the shared range rather than replacing a valid setting with a clamped surprise value.
      if let triggerCount = Self.sharedPreferenceInt(frequency["trigger_count"],
                                                      range: FrequencyAdjustmentPreference.countRange) {
        FrequencyAdjustmentPreference.triggerCount = triggerCount
      }
      if let linearStep = Self.sharedPreferenceInt(frequency["linear_step"],
                                                   range: FrequencyAdjustmentPreference.countRange) {
        FrequencyAdjustmentPreference.linearStep = linearStep
      }
    }
    if let glossEnabled = preferences["candidate_english_gloss"] as? Bool,
       glossEnabled != CandidateGlossPreference.enabled {
      CandidateGlossPreference.enabled = glossEnabled
      candidateGlossEpoch &+= 1
      candidateGlossRequestedGeneration = nil
      visibleCandidateGlosses = []
      candidateTargetGlosses = [:]
    }
    if let translationsEnabled = preferences["candidate_translations"] as? Bool {
      CandidateTranslationPreference.onlineEnabled = translationsEnabled
    }
    // The shared Tauri document is the source of truth for mobile settings. Keep the two Wubi
    // switches in the App Group as well because candidate rendering and Engine fallback read
    // these native preferences from the keyboard process.
    if let mixedPinyin = preferences["wubi_mixed_pinyin"] as? Bool {
      WubiMixedPinyinPreference.isEnabled = mixedPinyin
    }
    if let codeHint = preferences["wubi_code_hint"] as? Bool {
      WubiCodeHintPreference.isEnabled = codeHint
    }
    // 五笔版本由会话直接从文档读取；这里只更新方案名和方案卡片角标读的 App Group 镜像。
    WubiProfilePreference.mirror(preferences)
    if let traditional = preferences["traditional_chinese_output"] as? Bool,
       traditional != ChineseOutputPreference.usesTraditional {
      ChineseOutputPreference.usesTraditional = traditional
    }
    if let target = preferences["translation_target_language"] as? String,
       let index = CandidateTranslationPreference.languages.firstIndex(where: { $0.code == target.uppercased() }) {
      CandidateTranslationPreference.primaryIndex = index
    }
    if let secondary = preferences["translation_secondary_language"] as? String {
      let index = CandidateTranslationPreference.languages.firstIndex {
        $0.code == secondary.uppercased()
      } ?? -1
      CandidateTranslationPreference.secondaryIndex = index
    } else if preferences.keys.contains("translation_secondary_language") {
      CandidateTranslationPreference.secondaryIndex = -1
    }
    GlobalThemePreference.mirror(preferences)
    let previousTheme = KeyboardTheme.current
    let skinChanged = KeyboardTheme.reload(preferences) != previousTheme
    if let spacing = (preferences["touch_key_spacing_tenths"] as? NSNumber)?.doubleValue {
      KeyboardLayoutPreference.keySpacing = spacing / 10
    }
    if let spacing = (preferences["touch_row_spacing_tenths"] as? NSNumber)?.doubleValue {
      KeyboardLayoutPreference.rowSpacing = spacing / 10
    }
    if let voice = preferences["touch_voice_shortcut"] as? Bool {
      KeyboardLayoutPreference.voiceShortcutEnabled = voice
    }
    if let adjustment = (preferences["touch_keyboard_height_adjustment"] as? NSNumber)?.doubleValue,
       adjustment.isFinite {
      sharedKeyboardHeightAdjustment = CGFloat(min(48, max(-12, adjustment)))
    }

    var selectedScheme: ChineseInputScheme?
    if let schemes = preferences["touch_keyboard_schemes"] as? [String: Any] {
      let enabled = (schemes["enabled"] as? [String] ?? []).compactMap(Self.sharedInputScheme)
      if !enabled.isEmpty { InputSchemePreference.enabledSchemes = enabled }
      selectedScheme = (schemes["selected"] as? String).flatMap(Self.sharedInputScheme)
    }
    if !hasComposition, let selectedScheme, InputSchemePreference.offeredSchemes.contains(selectedScheme) {
      selectInputScheme(selectedScheme, persistShared: false)
    }
    if skinChanged { applyKeyboardSkin() }
    applyLayoutPreferences()
    updateShortcutButtons()
    updatePreferredKeyboardHeight()
    scheduleCandidateGlosses()
    renderCandidateStrip()
  }

  private static func sharedPreferenceInt(_ value: Any?, range: ClosedRange<Int> = 1...6) -> Int? {
    let integer: Int?
    if let value = value as? Int {
      integer = value
    } else if let value = value as? NSNumber {
      integer = value.intValue
    } else {
      integer = nil
    }
    if let integer, range.contains(integer) { return integer }
    return nil
  }

  // The output script may change in the host app while the keyboard is loaded, so it is re-read on
  // every appearance. Unlike the input scheme it never touches the session, so an active composition
  // only needs its visible candidates redrawn.
  private func synchronizeChineseOutputPreference() {
    let sharedValue = ChineseOutputPreference.usesTraditional
    guard sharedValue != usesTraditionalOutput else { return }

    usesTraditionalOutput = sharedValue
    renderCandidateStrip()
  }

  private func configureStripButton(
    _ button: UIButton, symbol: String, label: String, identifier: String
  ) {
    var configuration = UIButton.Configuration.plain()
    configuration.image = UIImage(systemName: symbol)
    configuration.baseForegroundColor = KeyboardTheme.current.accent
    configuration.contentInsets = NSDirectionalEdgeInsets(
      top: 2, leading: 2, bottom: 2, trailing: 2)
    button.configuration = configuration
    button.accessibilityLabel = label
    button.accessibilityIdentifier = identifier
    button.isHidden = true
    button.widthAnchor.constraint(equalToConstant: 26).isActive = true
  }

  private func showCandidatePanel() {
    closeKeyboardService()
    closeKeyboardPicker()
    playInputClick()
    do {
      let snapshot = try CandidatePanelSnapshot.decode(session.allCandidates())
      guard !snapshot.entries.isEmpty else { return }
      let indexes = snapshot.entries.map(\.index)
      let generation = snapshot.generation
      let panel = KeyboardCandidatePanelView(
        candidates: snapshot.entries.map(\.text), preedit: snapshot.preedit,
        annotations: snapshot.entries.map {
          candidatePanelAnnotation(code: $0.code, gloss: $0.translation, word: $0.text, engine: $0.annotation,
                                   typed: snapshot.preedit)
        },
        markers: snapshot.entries.map { CandidateMarker.markers(source: $0.source, fixedPosition: $0.fixedPosition) },
        candidateScale: candidateFontScale, preeditScale: preeditFontScale, candidateFamilies: candidateFontFamilies,
        display: { [weak self] in self?.chineseOutput($0) ?? $0 },
        menuElements: { [weak self] index in
          guard let self, indexes.indices.contains(index) else { return [] }
          let entry = snapshot.entries[index]
          return candidateMenuElements(
            generation: generation, globalIndex: indexes[index], candidate: entry.text,
            offlineGloss: entry.translation, fixedPosition: entry.fixedPosition)
        },
        onSelect: { [weak self] index in
          guard let self, indexes.indices.contains(index) else { return }
          closeKeyboardPicker()
          playInputClick()
          render(session.selectAnyCandidate(generation: generation, globalIndex: indexes[index]))
        },
        onClose: { [weak self] in self?.closeKeyboardPicker() })
      panel.accessibilityViewIsModal = true
      panel.translatesAutoresizingMaskIntoConstraints = false
      view.addSubview(panel)
      NSLayoutConstraint.activate([
        panel.leadingAnchor.constraint(equalTo: view.leadingAnchor),
        panel.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        panel.topAnchor.constraint(equalTo: view.topAnchor),
        panel.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      ])
      candidatePanel = panel
      candidatePanelGeneration = generation
      UIAccessibility.post(notification: .screenChanged, argument: panel)
    } catch {
      showDiagnostic("候选列表暂不可用")
    }
  }

  private func updateExpandControl() {
    // Offered whenever the strip is not already showing everything. Paging by nine used to be the
    // only way past the ninth candidate, which left the tail of a 351-candidate answer thirty-nine
    // taps away; the panel shows the whole list at once instead.
    expandCandidatesButton.isHidden = visibleCandidatePageCount <= 1 || visibleDiagnostic != nil
  }

  // The engine's local input modes open on a capital carried with a shift-only modifier, which this
  // keyboard has no key for. While nothing is being composed the strip's own name is dead space, so
  // it doubles as the way in; during a composition it goes back to showing the preedit and the menu
  // is withdrawn, because a mode cannot open on top of a composition anyway.
  // `key` is the mode's field in the shared `local_modes` preference.
  private static let localInputModes = [
    (trigger: "U", title: "Unicode 码点", key: "unicode"),
    (trigger: "T", title: "日期时间", key: "date_time"),
    (trigger: "J", title: "超级简拼", key: "super_jianpin"),
    (trigger: "K", title: "快捷短语", key: "quick_phrase"),
    (trigger: "Y", title: "英文补全", key: "temporary_english"),
    (trigger: "E", title: "表情", key: "emoji"),
    (trigger: "M", title: "颜文字", key: "kaomoji"),
    (trigger: "R", title: "临时日语", key: "temporary_japanese"),
  ]

  /// The modes the settings app leaves on. The Engine refuses to open a mode turned off there, so offering one would be a menu entry that does nothing.
  private var enabledLocalInputModes: [(trigger: String, title: String, key: String)] {
    let stored = session.sharedPreferences?["local_modes"] as? [String: Any] ?? [:]
    return Self.localInputModes.filter { stored[$0.key] as? Bool ?? true }
  }

  private func updatePreeditButton() {
    let idle = visiblePreedit.isEmpty
    let modeName = Self.localInputModes.first { $0.trigger == localModeTrigger }?.title
    let title = isInLocalMode && visiblePreedit == localModeTrigger
      ? (modeName ?? visiblePreedit)
      : (idle ? (isChineseMode ? "水杉输入法" : "英文输入") : visiblePreedit)
    // 「候选栏预编辑」 only changes what is drawn: `visiblePreedit` still says a composition is running, which keeps the strip up while a spelling has no candidates yet, and VoiceOver still reads the full title. The setting names the spelling, so a Japanese reading and what an in-place scheme composes are left as they are.
    let style = CandidatePreeditStyle(in: session.sharedPreferences)
    // A caret moved into the spelling is drawn even under 「不显示」: the next key acts at that caret, and a hidden one leaves no way to tell where.
    let drawnTitle = idle || title != visiblePreedit || !inputScheme.hasSpellingCaret
      ? title
      : visibleCaretSpelling.map { visiblePhrasePrefix + $0 } ?? style.title(
        composition: visiblePreedit, phrasePrefix: visiblePhrasePrefix,
        localModeName: isInLocalMode ? modeName : nil)
    if var configuration = preeditButton.configuration {
      configuration.title = drawnTitle
      preeditButton.configuration = configuration
    }

    let modes = enabledLocalInputModes
    let offersModes = idle && supportsLocalTools && !modes.isEmpty
    let spellingMenu = idle ? nil : editableSpelling.map(Self.spellingEditMenu)
    preeditButton.menu =
      offersModes
      ? UIMenu(
        title: "本地输入",
        children: modes.map { mode in
          UIAction(title: mode.title) { [weak self] _ in
            self?.openLocalInputMode(mode.trigger)
          }
        })
      : spellingMenu.map { items in
        UIMenu(title: "编辑拼写", children: items.map { item in
          UIAction(title: item.title, attributes: item.enabled ? [] : .disabled) { [weak self] _ in
            self?.editSpelling(item.edit)
          }
        })
      }
    // Withdrawing the menu is what makes the button inert; disabling it would dim the title, and
    // this is the preedit, which has to keep reading as the text the user is composing.
    preeditButton.accessibilityLabel = offersModes ? "本地输入模式" : title
    preeditButton.accessibilityValue = offersModes ? nil : title
    preeditButton.accessibilityHint = spellingMenu == nil ? nil : "轻点编辑拼写"
    preeditButton.accessibilityTraits = offersModes || spellingMenu != nil ? .button : .staticText
  }

  enum SpellingEdit: Equatable, Sendable { case start, end, deleteForward }

  /// Tapping the spelling offers the Home, End and Delete keys of the Windows composition, which a touch keyboard has no keys for: the space-bar drag walks the caret a letter or a syllable at a time, and these finish the job in one step. An entry that would do nothing where the caret is stays in the menu, dimmed, so the menu keeps its shape.
  nonisolated static func spellingEditMenu(_ spelling: (text: String, caret: Int)) -> [(title: String, edit: SpellingEdit, enabled: Bool)] {
    let atEnd = spelling.caret >= spelling.text.count
    return [
      ("光标移到开头", .start, spelling.caret > 0),
      ("光标移到末尾", .end, !atEnd),
      ("删除光标后的字母", .deleteForward, !atEnd),
    ]
  }

  private func editSpelling(_ edit: SpellingEdit) {
    guard hasComposition else { return }
    playInputClick()
    switch edit {
    case .start: render(session.moveCaretToStart())
    case .end: render(session.moveCaretToEnd())
    case .deleteForward: render(session.deleteForward())
    }
  }

  func openLocalInputMode(_ trigger: String) {
    playInputClick()
    localModeTrigger = trigger
    showsSymbols = false
    render(session.openLocalMode(trigger))
  }

  private func chineseOutput(_ text: String) -> String {
    if !inputScheme.writesChinese || localModeTrigger == "R" { return text }
    return ChineseTextConversion.outputString(text, traditional: usesTraditionalOutput)
  }

  private func updateSchemeButton() {
    // The hints come from the engine's own profile for the scheme the session is running, so they
    // are refreshed wherever the scheme is, and cannot drift from what the keys actually produce.
    shuangpinKeyHints = session.shuangpinKeyHints()
    updateLetterCaseControls()

    var configuration = UIButton.Configuration.plain()
    configuration.image = UIImage(systemName: "keyboard")
    configuration.baseForegroundColor = KeyboardTheme.current.accent
    configuration.contentInsets = NSDirectionalEdgeInsets(
      top: 3, leading: 4, bottom: 3, trailing: 4)
    // The scheme shortcut uses the same unboxed treatment as the other shared shortcuts. The
    // current scheme is already exposed through accessibilityValue and the picker it opens.
    configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
      var attributes = attributes
      attributes.font = .systemFont(ofSize: 16, weight: .medium)
      return attributes
    }
    schemeButton.configuration = configuration
    schemeButton.accessibilityIdentifier = "schemeButton"
    schemeButton.accessibilityLabel = "选择输入方案"
    schemeButton.accessibilityValue = inputScheme.title
    updateKeyboardLayout()
  }

  private func toggleLayout() {
    playInputClick()
    showsSymbols.toggle()
    updateKeyboardLayout()
  }

  private var quickPunctuationSymbols: [String] {
    // Korean and Vietnamese write half-width ASCII marks, the same menu English offers.
    guard isChineseMode, !isInLocalMode, !inputScheme.writesAsciiPunctuation else { return [",", ".", "?", "!", ":", ";", "@"] }
    if inputScheme.isJapanese { return ["、", "。", "？", "！", "「", "」", "・"] }
    return ["，", "。", "？", "！", "、", "；", "："]
  }

  private func updateLetterRowInsets() {
    guard letterRowViews.count > 1, let row = letterRowViews[1] as? UIStackView else { return }
    let inset: CGFloat = KeyboardLayoutPreference.geometry.centeredLetters && inputScheme != .microsoft
      ? max(0, view.bounds.width - 10) * CGFloat(KeyboardLayoutPreference.geometry.letterInsetRatio) : 0
    let margins = UIEdgeInsets(top: 0, left: inset, bottom: 0, right: inset)
    if row.layoutMargins != margins {
      row.isLayoutMarginsRelativeArrangement = true
      row.layoutMargins = margins
    }
  }

  private func applyLayoutPreferences() {
    guard actionRow != nil else { return }
    let layout = KeyboardLayoutPreference.geometry
    updateLetterRowInsets()
    guard appliedLayout != layout else { return }
    appliedLayout = layout
    keyboardRoot?.spacing = layout.rowSpacing
    nineGrid?.spacing = layout.rowSpacing
    nineControls?.spacing = layout.rowSpacing
    nineKeyHeight.constant = layout.rowSpacing * 2
    for row in [numberRowView as UIView?].compactMap({ $0 }) + letterRowViews + zhuyinRowViews + symbolRowViews + nineKeyRows {
      (row as? UIStackView)?.spacing = layout.keySpacing
    }
    actionRow.spacing = layout.keySpacing
    nineKeyContainer.spacing = layout.keySpacing
    if let old = nineSidebarWidth, let sidebar = old.firstItem as? UIView {
      old.isActive = false
      nineSidebarWidth = sidebar.widthAnchor.constraint(equalTo: nineKeyContainer.widthAnchor, multiplier: layout.sidebarRatio)
      nineSidebarWidth?.isActive = true
    }
    nineKeyActionWidths[0].isActive = false
    nineKeyActionWidths[0] = nineKeySymbolsButton.widthAnchor.constraint(equalTo: nineKeyContainer.widthAnchor, multiplier: layout.sidebarRatio)
    standardActionWidths[0].constant = 48.4
    standardActionWidths[1].constant = 44
    standardActionWidths[2].constant = 59.4
    quickPunctuationWidth?.constant = 44
    updateShortcutButtons()
  }

  /// Everything `updateKeyboardLayout` branches on.
  ///
  /// The list has to be complete, because the layout is only rebuilt when this changes. Gating on
  /// a subset is how a nine-key keyboard stayed on its grid after a local mode opened: the value
  /// that moved was not in the signature, so nothing relaid out.
  private struct KeyboardLayoutInputs: Equatable {
    let chinese: Bool
    let scheme: ChineseInputScheme
    let localMode: String
    let symbols: Bool
    let globe: Bool
    let hasSpellings: Bool
    let geometry: KeyboardGeometry
    let formFactor: KeyboardFormFactor
    let fullKeys: Bool
  }

  private var layoutInputs: KeyboardLayoutInputs {
    KeyboardLayoutInputs(
      chinese: isChineseMode, scheme: inputScheme, localMode: currentLocalMode,
      symbols: showsSymbols, globe: needsInputModeSwitchKey,
      hasSpellings: !currentNineKeySpellings.isEmpty,
      geometry: KeyboardLayoutPreference.geometry,
      formFactor: formFactor, fullKeys: KeyboardLayoutPreference.tabletFullKeys)
  }

  /// Rebuild the keyboard only when something it depends on moved.
  ///
  /// This used to run on every keystroke, from `updateSpellingStrip`. Laying the whole keyboard
  /// out costs the same whether or not anything changed, and between two letters of the same word
  /// nothing does.
  private func updateKeyboardLayoutIfNeeded() {
    let inputs = layoutInputs
    guard appliedLayoutInputs != inputs else { return }
    appliedLayoutInputs = inputs
    updateKeyboardLayout()
  }

  private func updateSymbolKeyFaces() {
    // Chinese punctuation only appears in Chinese mode. Local utilities and dedicated English input send the literal ASCII key value, so their labels must follow their insertion path. Korean and Vietnamese write ASCII marks too. Zhuyin's panel writes its marks itself, by the punctuation switch, so its faces follow that switch too.
    let sendsChinesePunctuation = isChineseMode && !isInLocalMode && !inputScheme.writesAsciiPunctuation
      && (!typesZhuyin || zhuyinWritesChinesePunctuation)
    for face in symbolKeyFaces {
      let title = sendsChinesePunctuation ? face.chinese : face.ascii
      guard face.key.configuration?.title != title else { continue }
      face.key.configuration?.title = title
      face.key.accessibilityLabel = "符号 \(title)"
    }
    // 藏文方案下 `=` 键显示它实际发出的 `+`。
    if let key = tibetanPlusKey {
      let title = Self.symbolRowKey("=", tibetan: typesTibetan)
      if key.configuration?.title != title {
        key.configuration?.title = title
        key.accessibilityLabel = "符号 \(title)"
      }
    }
  }

  private func updateKeyboardLayout() {
    applyLayoutPreferences()
    standardRowHeights.forEach { $0.1.isActive = false }
    microsoftFinalKey?.isHidden = !(isChineseMode && inputScheme == .microsoft && !isInLocalMode)
    let kana = isChineseMode && inputScheme == .japaneseNineKey && !isInLocalMode
    japaneseKeys?.isHidden = !kana
    japaneseKeys?.setDigits(showsSymbols)
    japaneseHeight?.constant = KeyboardLayoutPreference.rowSpacing * 3 + 4 * 44
    japaneseHeight?.isActive = kana
    japaneseKeys?.applyLayout()
    actionRow?.isHidden = kana
    japaneseGlobeButton?.isHidden = !needsInputModeSwitchKey
    japaneseKeys?.setModeColumnFull(needsInputModeSwitchKey)
    let nineKey = isChineseMode && inputScheme == .nineKey && !isInLocalMode
    let writes = isChineseMode && inputScheme == .handwriting && !showsSymbols && !isInLocalMode
    if !writes && !handwriting.isHidden { handwriting.deactivate() }
    handwriting.isHidden = !writes
    if writes { handwriting.activate() }
    handwritingActionHeight?.isActive = writes
    // Dachen takes the digit row and four punctuation keys for bopomofo, so Zhuyin draws its own four rows in place of the letter rows and the number row.
    let dachen = isChineseMode && inputScheme.isZhuyin && !isInLocalMode
    zhuyinRowViews.forEach { $0.isHidden = !dachen || showsSymbols }
    letterRowViews.forEach { $0.isHidden = showsSymbols || nineKey || writes || kana || dachen }
    let fullKeys = formFactor.canShowFullKeys && KeyboardLayoutPreference.tabletFullKeys
    numberRowView?.isHidden = !fullKeys || showsSymbols || nineKey || writes || kana || dachen
    tabKey?.isHidden = !fullKeys
    let rowPunctuation = formFactor.showsLetterRowPunctuation
    for key in letterRowPunctuationKeys where key.isHidden == rowPunctuation { key.isHidden = !rowPunctuation }
    // The nine-key digit layer keeps the three-column grid and only changes its legends. This
    // avoids replacing it with the ten-across symbol rows and preserves the user's chosen layout.
    let nineKeyDigits = nineKey && showsSymbols
    nineKeyContainer.isHidden = !nineKey
    nineKeyRows.forEach { $0.isHidden = !nineKey }
    applyNineKeyDigitLayer(nineKeyDigits)
    let hasSpellings = !currentNineKeySpellings.isEmpty
    spellingScrollView.isHidden = !hasSpellings
    punctuationStack.isHidden = hasSpellings
    if actionRow != nil {
      let usesNineKeyLayout = nineKey
      nineKeyHeight.isActive = usesNineKeyLayout
      let globeIndex = usesNineKeyLayout ? 5 : 2
      if actionRow.arrangedSubviews.firstIndex(of: actionGlobeButton) != globeIndex {
        actionRow.removeArrangedSubview(actionGlobeButton)
        actionGlobeButton.removeFromSuperview()
        actionRow.insertArrangedSubview(actionGlobeButton, at: globeIndex)
      }
      NSLayoutConstraint.deactivate(standardActionWidths + nineKeyActionWidths)
      symbolDeleteWidth?.isActive = false
      quickPunctuationWidth?.isActive = false
      globeWidthConstraint?.isActive = false
      actionGlobeButton.isHidden = !needsInputModeSwitchKey
      if needsInputModeSwitchKey {
        globeWidthConstraint = actionGlobeButton.widthAnchor.constraint(equalToConstant: 44)
        globeWidthConstraint?.isActive = true
      }
      let layout = KeyboardLayoutPreference.geometry
      nineKeySymbolsButton.isHidden = !(usesNineKeyLayout || kana || (layout.showsFullKeyboardSymbols && !showsSymbols))
      fullSymbolsWidth?.isActive = !nineKeySymbolsButton.isHidden && !usesNineKeyLayout
      bottomLanguageButton?.isHidden = false
      bottomLanguageWidth?.isActive = true
      quickPunctuationButton.isHidden = usesNineKeyLayout || showsSymbols || writes || kana
      quickPunctuationWidth?.isActive = !quickPunctuationButton.isHidden
      let punctuation = quickPunctuationSymbols
      quickPunctuationButton.configuration?.title = punctuation[0]
      quickPunctuationButton.accessibilityValue = punctuation[0]
      // A long press opens this menu without firing the tap action, so the choice counts the press of the key once, never the mark it types.
      quickPunctuationButton.menu = UIMenu(children: punctuation.map { symbol in
        UIAction(title: symbol) { [weak self] _ in
          self?.countKeyPress(TypingKeyID.quickPunctuation)
          self?.handleSymbol(symbol)
        }
      })
      actionDeleteButton.isHidden = !showsSymbols || kana
      symbolDeleteWidth?.isActive = showsSymbols && !kana
      NSLayoutConstraint.activate(usesNineKeyLayout ? nineKeyActionWidths : standardActionWidths)
    }
    symbolRowViews.forEach { $0.isHidden = !showsSymbols || kana || nineKey }
    updateSymbolKeyFaces()
    for (row, height) in standardRowHeights { height.isActive = !row.isHidden }
    if var configuration = layoutToggleButton?.configuration {
      configuration.title = showsSymbols ? (kana ? "あいう" : (nineKey ? "九键" : "ABC")) : "123"
      layoutToggleButton?.configuration = configuration
    }
    layoutToggleButton?.accessibilityLabel =
      showsSymbols ? "切换到字母" : "切换到数字和符号"
    // The kana layout keeps this key on its own Japanese punctuation menu rather than the symbol
    // panel, which carries no kana marks. Everywhere else the key opens the panel, so the menu has
    // to be taken back off or a stale one would keep answering the tap.
    if kana {
      // The menu is the key's primary action here, so its tap action never runs: the choice stands for the one press of 符.
      nineKeySymbolsButton?.menu = UIMenu(children: quickPunctuationSymbols.map { symbol in
        UIAction(title: symbol) { [weak self] _ in
          self?.countKeyPress(TypingKeyID.symbol)
          self?.handleSymbol(symbol)
        }
      })
      nineKeySymbolsButton?.showsMenuAsPrimaryAction = true
    } else {
      nineKeySymbolsButton?.menu = nil
      nineKeySymbolsButton?.showsMenuAsPrimaryAction = false
    }
    updatePreferredKeyboardHeight()
  }

  /// Whether Tab turns to more candidates while composing, the shared `navigation.tab` (Windows `paging_tab`, on by default).
  nonisolated static func tabShowsMoreCandidates(_ preferences: [String: Any]?) -> Bool {
    KeyboardLayoutPreference.tabShowsMoreCandidates(preferences)
  }

  /// The Engine's `CandidateSource::Fallback`, as it appears in a view candidate's `source`.
  nonisolated static let candidateSourceFallback = 9

  /// Whether Japanese Space arms or steps a conversion. A lone Fallback row is the raw composition the Engine shows when there is nothing to convert (a bare Shift+R prefix, or romaji it cannot read); Windows commits it on the first Space, so Space takes the normal commit path instead.
  nonisolated static func japaneseSpaceConverts(candidateCount: Int, firstSource: Int) -> Bool {
    guard candidateCount > 0 else { return false }
    return !(candidateCount == 1 && firstSource == candidateSourceFallback)
  }

  /// Tab on the full-size iPad keyboard. On the desktop Tab turns the candidate page; the strip here has no pages to turn, only the full candidate panel behind it, so with candidates showing Tab opens that. Otherwise it ends any composition and types a tab, as an unhandled key does.
  private func handleTab() {
    let composing = isChineseMode && (!visiblePreedit.isEmpty || !visibleCandidates.isEmpty)
    if composing, !visibleCandidates.isEmpty, Self.tabShowsMoreCandidates(session.sharedPreferences) {
      showCandidatePanel()
      return
    }
    playInputClick()
    if composing { render(session.finishComposition()) }
    insertOwnText("\t")
    if !isChineseMode { refreshEnglishSuggestions() }
  }

  private func handleBackspace() {
    if !handwriting.isHidden && handwriting.hasInk { handwriting.canvas.undo(); return }
    playInputClick()
    // Deleting document text may take the opening half of a pair with it; a backspace inside a spelling only edits the spelling.
    if !hasComposition { pairedPunctuation.clear() }
    if !isChineseMode {
      deleteOwnBackward()
      refreshEnglishSuggestions()
      return
    }
    let snapshot = session.handleBackspace()
    if !snapshot.isHandled {
      deleteOwnBackward()
    }
    render(snapshot)
  }

  /// Touch down counts the press once; the repeat a held key starts is the same press.
  @objc private func beginBackspacePress() {
    countKeyPress(TypingKeyID.backspace)
    cancelBackspacePress()
    didRepeatBackspace = false

    let timer = Timer(
      timeInterval: 0.075,
      target: self,
      selector: #selector(repeatBackspace),
      userInfo: nil,
      repeats: true)
    timer.fireDate = Date(timeIntervalSinceNow: 0.4)
    RunLoop.main.add(timer, forMode: .common)
    backspaceRepeatTimer = timer
  }

  @objc private func finishBackspacePress() {
    let repeated = didRepeatBackspace
    cancelBackspacePress()
    if !repeated {
      handleBackspace()
    }
  }

  @objc private func cancelBackspacePress() {
    backspaceRepeatTimer?.invalidate()
    backspaceRepeatTimer = nil
    didRepeatBackspace = false
  }

  @objc private func repeatBackspace() {
    // Holding backspace in a spelling takes it apart a syllable at a time, as Ctrl+Backspace does in the Windows composition, so a long spelling can be cut back to the syllable that went wrong instead of being thrown away whole. The hold ends with the composition: the repeat never carries on into text that is already in the document. Nine-key, wubi and the other schemes without lettered syllables keep the hold that clears the whole composition below.
    // A Korean syllable, a Zhuyin conversion and a Vietnamese word are text already, so a held backspace takes them apart a key at a time and carries on into the document, as the system keyboards for those languages do.
    if composesInPlace {
      didRepeatBackspace = true
      handleBackspace()
      return
    }
    if hasComposition && isChineseMode && inputScheme.editsBySyllable {
      didRepeatBackspace = true
      playInputClick()
      render(session.segmentBackspace())
      if !hasComposition {
        backspaceRepeatTimer?.invalidate()
        backspaceRepeatTimer = nil
      }
      return
    }
    if !didRepeatBackspace && hasComposition {
      backspaceRepeatTimer?.invalidate()
      backspaceRepeatTimer = nil
      didRepeatBackspace = true
      playInputClick()
      render(session.cancel())
      return
    }
    didRepeatBackspace = true
    handleBackspace()
  }

  // Space means "commit the leading candidate". The strip no longer pages, so the leading chip is
  // always the engine's first and commitCandidate is that candidate. It also reports itself
  // unhandled when there is nothing to commit, which is what tells the caller to insert its space.
  // Return and the language switch end the whole composition through CompositionBoundaryPolicy instead.
  private func commitVisibleCandidate() -> MetasequoiaInputSnapshot {
    return session.commitCandidate()
  }

  func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
    guard gestureRecognizer.name == "spaceCursorPan", let pan = gestureRecognizer as? UIPanGestureRecognizer else { return true }
    let velocity = pan.velocity(in: view)
    return abs(velocity.x) > abs(velocity.y)
  }

  private func moveCompositionCaret(by offset: Int, bySegment: Bool = false) {
    guard offset != 0 else { return }
    var snapshot: MetasequoiaInputSnapshot?
    for _ in 0..<min(abs(offset), 64) {
      if bySegment {
        snapshot = offset < 0 ? session.moveCaretLeftBySegment() : session.moveCaretRightBySegment()
      } else {
        snapshot = offset < 0 ? session.moveCaretLeft() : session.moveCaretRight()
      }
    }
    if let snapshot { render(snapshot) }
  }

  private func moveCursor(by offset: Int) {
    guard offset != 0 else { return }
    guard let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else { return }
    if hasComposition { render(session.finishComposition()) }
    guard KeyboardHostContext.documentIdentifier(for: textDocumentProxy) == document else { return }
    pairedPunctuation.clear()
    textDocumentProxy.adjustTextPosition(byCharacterOffset: offset)
  }

  @objc private func handleSpacePan(_ pan: UIPanGestureRecognizer) {
    guard let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else {
      cursorMovement.cancel()
      updateSpaceKeyTitle()
      return
    }
    switch pan.state {
    case .began:
      cursorMovement.begin(at: pan.translation(in: view).x, document: document)
      // While spelling, the drag edits the spelling: the caret walks through the pinyin as ← / → do in the Windows composition, and typing or backspace then acts there. A Japanese reading keeps ending the composition, because its conversion owns the space key.
      // A Korean syllable, a Zhuyin conversion and a Vietnamese word have no caret inside them either, so the drag finishes them and moves the document caret.
      spaceDragEditsComposition = hasComposition && inputScheme.hasSpellingCaret
      if hasComposition && !spaceDragEditsComposition { render(session.finishComposition()) }
      spaceButton?.configuration?.title = spaceDragEditsComposition ? "移动拼音光标" : "移动光标"
      if KeyboardFeedbackPreference.hapticsEnabled {
        keyFeedback.impactOccurred(intensity: KeyboardFeedbackPreference.hapticStrength.intensity)
      }
    case .changed:
      let steps = cursorMovement.advance(to: pan.translation(in: view).x, document: document)
      if spaceDragEditsComposition && hasComposition {
        moveCompositionCaret(
          by: steps,
          bySegment: inputScheme.editsBySyllable && SpaceCursorMovement.movesBySegment(velocity: pan.velocity(in: view).x))
      } else {
        moveCursor(by: steps)
      }
      if !cursorMovement.isActive { updateSpaceKeyTitle() }
    case .ended, .cancelled, .failed:
      cursorMovement.cancel()
      updateSpaceKeyTitle()
    default: break
    }
  }

  private func handleSpace() {
    if !handwriting.isHidden && handwriting.hasInk { _ = handwriting.commitFirst(); return }
    playInputClick()
    // A space right after a committed Chinese mark rewrites it as ASCII and is swallowed: the
    // user is correcting the mark they just typed, not typing a mark and then a space. Checked
    // before anything else claims the key, and before the English branch, because the mark it
    // corrects was committed while the keyboard was in Chinese.
    let conversion = session.smartPunctuationDecision(
      character: " ", preceding: precedingCharacter,
      timestampMilliseconds: smartPunctuationNow, editorGeneration: smartPunctuationEditor,
      repeatSnapshot: nil, spaceSnapshot: armedSpaceConversion)
    if let ascii = (conversion["space_ascii"] as? NSNumber)?.uint8Value,
       let scalar = UnicodeScalar(UInt32(ascii)) {
      clearSmartPunctuationArming()
      replacePrecedingCharacter(with: String(Character(scalar)))
      return
    }
    clearSmartPunctuationArming()
    if !isChineseMode {
      insertDirectText(" ")
      refreshEnglishSuggestions()
      return
    }
    if inputScheme.isJapanese && hasComposition
        && Self.japaneseSpaceConverts(candidateCount: visibleCandidates.count,
                                      firstSource: visibleCandidateSources.first ?? -1) {
      let next = (japaneseConversionIndex.map { $0 + 1 } ?? 0) % visibleCandidates.count
      japaneseConversionIndex = next
      renderCandidateStrip()
      updateSpaceKeyTitle()
      updateReturnKey()
      return
    }
    // In Zhuyin Space is a key of the spelling while the list is closed: tone 1 on a pending syllable, or the key that opens the list. With the list open it picks the highlighted row below, like any list.
    if typesZhuyin && hasComposition && !zhuyinListOpen {
      let snapshot = session.handleCharacter(" ")
      render(snapshot)
      if !snapshot.isHandled { insertDirectText(" ") }
      return
    }
    let snapshot = commitVisibleCandidate()
    // An unhandled Space can still carry a commit: the Korean syllable it ended. That goes in first and the space after it.
    render(snapshot)
    if !snapshot.isHandled {
      insertDirectText(" ")
    }
  }

  // Gated the same way as handleSpace: a Return that commits a composition has done its job, and
  // the newline is only the key's own character. Inserting it unconditionally appended a stray
  // newline after every committed word, and in a field whose return key is 发送 or 完成 it also
  // fired that field's primary action. macOS swallows Return during a composition for this reason.
  private func handleReturn() {
    if !handwriting.isHidden && handwriting.hasInk { _ = handwriting.commitFirst(); return }
    playInputClick()
    if inputScheme.isJapanese && hasComposition {
      let index = japaneseConversionIndex
      japaneseConversionIndex = nil
      render(index.map { session.selectCandidate(at: UInt($0)) } ?? session.commitReading())
      return
    }
    let snapshot = endComposition(at: .returnKey)
    // 未处理的回车仍可能带着上屏内容：它结束的韩语音节或越南语单词。先写入上屏内容，再换行。从打开的列表里选的汉字、注音转换结果和藏文音节以已处理返回，不换行。
    render(snapshot)
    if !snapshot.isHandled {
      pairedPunctuation.clear()
      insertOwnText("\n")
    }
  }

  /// Ends an open composition the way `boundary` calls for. An idle session still gets finishComposition, which reports itself unhandled so the caller knows nothing was committed.
  private func endComposition(at boundary: CompositionBoundary) -> MetasequoiaInputSnapshot {
    switch CompositionBoundaryPolicy.action(composing: hasComposition, scheme: inputScheme, boundary: boundary,
                                            koreanHanjaListOpen: koreanHanjaListOpen) {
    case .commitRaw: session.commitRaw()
    case .commitCandidate: session.commitCandidate()
    case .finishComposition, .none: session.finishComposition()
    }
  }

  @objc private func handleInputModeButton(_ sender: UIButton, event: UIEvent) {
    if event.allTouches?.contains(where: { touch in touch.phase == .began }) == true {
      countKeyPress(TypingKeyID.globe)
      render(endComposition(at: .modeSwitch))
    }
    handleInputModeList(from: sender, with: event)
  }

  // Every document mutation the keyboard makes goes through here so textWillChange can tell its own
  // echo apart from a genuine host-initiated change.
  private var typingSource: TypingSource {
    if localModeTrigger == "R" || (isChineseMode && inputScheme.isJapanese) { return .japanese }
    if localModeTrigger != nil { return .local }
    if !isChineseMode { return .english }
    return TypingSource(rawValue: inputScheme.rawValue) ?? .unknown
  }

  /// `replacingComposition` is the runtime's commit, which takes the place of the letters 行内预编辑 marked; anything else inserted mid-composition goes in beside them, and the next render marks the composition again.
  private func insertOwnText(_ text: String, source: TypingSource? = nil, replacingComposition: Bool = false) {
    if replacingComposition && !inlineMarkedText.isEmpty {
      // Turning the marked letters into the committed text and unmarking it is one change to the document, where removing them and then inserting would be two.
      inlineMarkedText = ""
      pendingOwnEdits += 1
      textDocumentProxy.setMarkedText(text, selectedRange: NSRange(location: (text as NSString).length, length: 0))
      textDocumentProxy.unmarkText()
    } else {
      showInlineComposition("")
      pendingOwnEdits += 1
      textDocumentProxy.insertText(text)
    }
    recordTypingStatistics(text, source: source ?? typingSource)
  }

  /// 韩语、越南语和藏文不论全角开关如何都写半角 ASCII，与运行时对它们上屏内容的处理一致。
  private func insertDirectText(_ text: String, source: TypingSource? = nil) {
    insertOwnText(FullWidthInputPolicy.output(text, enabled: fullWidthInput && !typesKorean && !typesCasedLetters), source: source)
  }

  /// The runtime converts what it commits and the keyboard converts what it inserts itself, so both are told together.
  private func setFullWidthInput(_ enabled: Bool) {
    fullWidthInput = enabled
    session.setCharacterWidth(fullwidth: enabled)
  }

  private static func sharedChinesePunctuation(_ preferences: [String: Any]?) -> Bool {
    preferences?["chinese_punctuation"] as? Bool ?? true
  }

  private func setChinesePunctuation(_ enabled: Bool) {
    chinesePunctuation = enabled
    session.setChinesePunctuation(enabled)
    // The Zhuyin panel's faces follow the switch, and it can flip while the panel is on screen.
    if typesZhuyin { updateSymbolKeyFaces() }
  }

  /// Hand the runtime the Keychain key for candidate-bar AI, and take it back when the settings app turns it off or points it somewhere the saved keyboard configuration does not.
  private func synchronizeAICredential() {
    let configuration = KeyboardAIService.configuration()
    guard let configuration,
          AICandidatePreference.credentialEndpoint(session.sharedPreferences,
                                                   configuredEndpoint: configuration.endpoint) != nil
    else {
      session.setAICredential(nil)
      return
    }
    session.setAICredential(try? KeyboardAIService.token(for: configuration))
  }

  /// A value changed in the settings app replaces the card's switch; a document that only changed something else leaves it.
  private func synchronizeChinesePunctuation() {
    let shared = Self.sharedChinesePunctuation(session.sharedPreferences)
    defer { appliedChinesePunctuation = shared }
    guard shared != appliedChinesePunctuation else { return }
    setChinesePunctuation(shared)
  }

  /// A width changed in the settings app replaces the card's switch; a document that only changed something else leaves it.
  private func synchronizeCharacterWidth() {
    let width = CharacterWidthPreference.value(in: session.sharedPreferences)
    defer { appliedCharacterWidth = width }
    guard CharacterWidthPreference.overridesToggle(previous: appliedCharacterWidth, next: width) else { return }
    setFullWidthInput(width == CharacterWidthPreference.fullwidth)
  }

  // Swallowing this left the settings screen showing zeros with nothing to explain them, which is
  // how it reached a bug report rather than the person typing. The reason does not change between
  // keystrokes and a banner on each one would bury the composition, so it is said once per session.
  // The store locks, rewrites and fsyncs per commit, so it runs off main on a serial queue.
  private func recordTypingStatistics(_ text: String, source: TypingSource) {
    guard hasFullAccess else { return }
    let date = Date()
    statisticsQueue.async { [weak self] in
      do {
        try TypingStatisticsStore().record(text, source: source, at: date)
      } catch {
        let message = error.localizedDescription
        DispatchQueue.main.async { [weak self] in
          guard let self, !self.reportedStatisticsFailure else { return }
          self.reportedStatisticsFailure = true
          self.showDiagnostic("统计未能写入：\(message)")
        }
      }
    }
  }

  /// Ask the store whether statistics are on, then flush every 30 seconds while the keyboard is up. Presses before the answer arrives are not counted, which keeps a keyboard whose user turned statistics off from holding a single count.
  private func startCountingKeyPresses() {
    keyPressFlushTimer?.invalidate()
    keyPressFlushTimer = nil
    countsKeyPresses = false
    _ = keyPresses.drain()
    guard hasFullAccess else { return }
    statisticsQueue.async { [weak self] in
      let enabled = (try? TypingStatisticsStore().isEnabled()) ?? false
      DispatchQueue.main.async { [weak self] in
        guard let self else { return }
        countsKeyPresses = enabled
        if !enabled { _ = keyPresses.drain() }
      }
    }
    let timer = Timer(timeInterval: 30, target: self, selector: #selector(flushKeyPresses), userInfo: nil, repeats: true)
    RunLoop.main.add(timer, forMode: .common)
    keyPressFlushTimer = timer
  }

  /// Fields that hold a password or a one-time code, whose key presses are never counted.
  private static let credentialContentTypes: Set<UITextContentType> = [.password, .newPassword, .oneTimeCode]

  /// A secure field, or one marked only by its content type as a password or one-time code. Key presses there are not counted and the cloud clipboard is neither shown nor inserted.
  static func isCredentialField(secure: Bool?, contentType: UITextContentType?) -> Bool {
    if secure == true { return true }
    if let contentType, credentialContentTypes.contains(contentType) { return true }
    return false
  }
  private var isCredentialField: Bool {
    Self.isCredentialField(secure: textDocumentProxy.isSecureTextEntry, contentType: textDocumentProxy.textContentType ?? nil)
  }

  /// One press of a soft key, by its id in `TypingKeyID`; `nil` is a key with no id, which is not counted. Secure and credential fields are skipped as well: iOS normally swaps in the system keyboard for secure ones, and a host that does not, or that marks a field only by its content type, still gets nothing recorded.
  private func countKeyPress(_ id: String?) {
    guard let id, countsKeyPresses, hasFullAccess, !isCredentialField else { return }
    for batch in keyPresses.record(id, day: TypingStatistics.dayKey(Date())) { writeKeyPresses(batch) }
  }

  @objc private func flushKeyPresses() {
    if let batch = keyPresses.drain() { writeKeyPresses(batch) }
  }

  private func writeKeyPresses(_ batch: TypingKeyBatch) {
    statisticsQueue.async { [weak self] in
      do {
        let recorded = try TypingStatisticsStore().recordKeys(batch.keys, day: batch.day)
        // Nothing taken means statistics were turned off since the keyboard last asked.
        guard recorded == 0 else { return }
        DispatchQueue.main.async { [weak self] in
          guard let self else { return }
          countsKeyPresses = false
          _ = keyPresses.drain()
        }
      } catch {
        let message = error.localizedDescription
        DispatchQueue.main.async { [weak self] in
          guard let self, !self.reportedStatisticsFailure else { return }
          self.reportedStatisticsFailure = true
          self.showDiagnostic("统计未能写入：\(message)")
        }
      }
    }
  }

  private func deleteOwnBackward() {
    showInlineComposition("")
    pendingOwnEdits += 1
    textDocumentProxy.deleteBackward()
  }

  /// Bring the host's marked text in line with `text`; an empty `text` takes it out.
  private func showInlineComposition(_ text: String) {
    guard let edit = InlineCompositionPolicy.edit(showing: inlineMarkedText, next: text) else { return }
    inlineMarkedText = text
    pendingOwnEdits += 1
    switch edit {
    case .mark(let marked):
      textDocumentProxy.setMarkedText(marked, selectedRange: NSRange(location: (marked as NSString).length, length: 0))
    case .clear:
      textDocumentProxy.setMarkedText("", selectedRange: NSRange(location: 0, length: 0))
      textDocumentProxy.unmarkText()
    }
  }

  private func render(_ snapshot: MetasequoiaInputSnapshot, source originalSource: TypingSource? = nil) {
    candidateRevision &+= 1
    let source = originalSource ?? typingSource
    if localModeTrigger != nil && !isInLocalMode {
      localModeTrigger = nil
      showsSymbols = false
    }
    currentLocalMode = snapshot.localMode
    currentNineKeySpellings = snapshot.nineKeySpellings
    updateLetterCaseControls()
    if let commitText = snapshot.commitText {
      insertOwnText(source == .japanese ? commitText : chineseOutput(commitText), source: source,
                    replacingComposition: true)
    }
    let wasComposing = hasComposition
    let wasKoreanHanjaListOpen = koreanHanjaListOpen
    hasComposition = !snapshot.preedit.isEmpty
    if !hasComposition || snapshot.commitText != nil { japaneseConversionIndex = nil }
    if inputScheme.isJapanese {
      updateSpaceKeyTitle()
      updateReturnKey()
    } else if hasComposition != wasComposing {
      // With inline preedit off nothing reaches the field, so textDidChange never fires; return switches to 确认 and the accent here, when a composition starts or ends (dc.html L2217).
      updateReturnKey()
    }
    if !hasComposition { applyLearningPreferences() }
    showDiagnostic(snapshot.diagnosticText)
    // 已选的那一段领在读音前面，与来源把 word_for_creating_word 拼在读音前面是同一件事。行内预编辑关闭时（默认）编辑框里没有组字，候选条这一行就是用户唯一能看见它的地方；打开后按所选样式（原始按键或拼音分词）也作为标记文本写进编辑框。
    let composing = snapshot.phrasePrefix
      + (inputScheme.isJapanese && !snapshot.reading.isEmpty ? snapshot.reading : snapshot.preedit)
    visiblePhrasePrefix = snapshot.phrasePrefix
    // The `editing_text` of an in-place scheme is the keys of what it composes, not a spelling with a caret to move.
    visibleCaretSpelling = inputScheme.hasSpellingCaret ? snapshot.editingTextWithCaret : nil
    editableSpelling = hasComposition && inputScheme.hasSpellingCaret && !snapshot.isInLocalMode
      && !snapshot.editingText.isEmpty && snapshot.editingText.allSatisfy(\.isASCII)
      ? (snapshot.editingText, snapshot.caretPosition) : nil
    let japaneseReading = inputScheme.isJapanese && !snapshot.reading.isEmpty ? snapshot.reading : nil
    showInlineComposition(hasComposition
      ? InlineCompositionPolicy.markedText(
        inPlace: isChineseMode && inputScheme.composesInPlace, style: InlinePreeditPreference.style,
        phrasePrefix: snapshot.phrasePrefix, preedit: snapshot.preedit,
        editingText: snapshot.editingText, japaneseReading: japaneseReading)
      : "")
    updateCandidateStrip(
                         preedit: composing,
                         candidates: snapshot.candidates,
                         candidateCodes: snapshot.candidateCodes,
                         candidateGlosses: snapshot.candidateGlosses,
                         candidateAnnotations: snapshot.candidateAnnotations,
                         candidateSources: snapshot.candidateSources,
                         candidateFixedPositions: snapshot.candidateFixedPositions,
                         candidatePageCount: snapshot.candidatePageCount,
                         answeredByPinyinFallback: snapshot.answeredByPinyinFallback)
    // The Hanja list opens and closes while the syllable keeps composing, and Return chooses a Hanja only while it is open.
    if !inputScheme.isJapanese && koreanHanjaListOpen != wasKoreanHanjaListOpen { updateReturnKey() }
    refreshCandidatePanelAnnotations()
    updateSpellingStrip()
    scheduleCandidateGlosses()
    onlineCandidates.refresh(allowed: hasFullAccess && hasComposition && !isInLocalMode)
  }

  // A diagnostic means the key was handled but something behind it failed, so input keeps working
  // and the message is transient. It replaces the candidate strip, which is empty in exactly the
  // cases that produce one, and clears itself on the next key or after a few seconds.
  private func showDiagnostic(_ diagnostic: String?) {
    diagnosticDismissTimer?.invalidate()
    diagnosticDismissTimer = nil
    visibleDiagnostic = diagnostic
    guard diagnostic != nil else { return }

    let timer = Timer(timeInterval: 4, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated {
        guard let self else { return }
        self.visibleDiagnostic = nil
        self.diagnosticDismissTimer = nil
        self.renderCandidateStrip()
      }
    }
    RunLoop.main.add(timer, forMode: .common)
    diagnosticDismissTimer = timer
  }

  private func updateCandidateStrip(preedit: String, candidates: [String],
                                    candidateCodes: [String] = [], candidateGlosses: [String] = [],
                                    candidateAnnotations: [String] = [], candidateSources: [Int] = [],
                                    candidateFixedPositions: [Int] = [], candidatePageCount: Int = 0,
                                    answeredByPinyinFallback: Bool = false) {
    if visibleCandidates != candidates {
      candidateGlossRequestedGeneration = nil
    }
    visiblePreedit = preedit
    visibleCandidates = candidates
    visibleCandidateCodes = candidateCodes
    visibleCandidateGlosses = candidateGlosses
    visibleCandidateAnnotations = candidateAnnotations
    visibleCandidateSources = candidateSources
    visibleCandidateFixedPositions = candidateFixedPositions
    visibleCandidatePageCount = candidatePageCount
    visibleCandidatesAnsweredByPinyinFallback = answeredByPinyinFallback
    requestCandidateTranslations()
    // Any new candidate list is a different composition or a different set of matches, so the page
    // it was showing no longer describes anything.
    // A horizontal offset belongs to the previous matches, just like the page index.
    // Cancel deceleration as well so it cannot hide the new leading candidate.
    candidateScrollView.setContentOffset(.zero, animated: false)
    renderCandidateStrip()
  }

  private func renderCandidateStrip() {
    exitLocalModeButton.isHidden = !isInLocalMode
    updateHanjaButton()
    let showsCandidates = isInLocalMode || !visiblePreedit.isEmpty || !visibleCandidates.isEmpty || visibleDiagnostic != nil
    shortcutBar.isHidden = showsCandidates
    candidateContent?.isHidden = !showsCandidates
    updatePreeditButton()
    let page = Array(visibleCandidates.prefix(candidatePageSize))
    while candidateStack.arrangedSubviews.count < page.count {
      let index = candidateStack.arrangedSubviews.count
      candidateStack.addArrangedSubview(makeCandidateButton(index: index))
    }
    for (offset, view) in candidateStack.arrangedSubviews.enumerated() {
      guard let chip = view as? KeyboardKeyButton else { continue }
      chip.isHidden = offset >= page.count
      guard offset < page.count else { continue }
      // A Korean Hanja's 훈음 goes on the first line under it rather than after it, so it is not read as part of the candidate.
      let annotation = candidateAnnotation(at: offset).text
      updateCandidateButton(
        chip, candidate: page[offset], hint: typesKorean ? "" : annotation,
        reading: typesKorean ? annotation : nil,
        glosses: candidateGlosses(at: offset), markers: candidateMarkers(at: offset), number: offset + 1,
        converting: japaneseConversionIndex == offset)
    }
    updateExpandControl()

    diagnosticLabel.text = visibleDiagnostic
    diagnosticLabel.accessibilityLabel = visibleDiagnostic.map { "提示：\($0)" }
    diagnosticLabel.isHidden = visibleDiagnostic == nil
    candidateScrollView.isHidden = visibleCandidates.isEmpty || visibleDiagnostic != nil
    candidateEmptySpacer.isHidden = !visibleCandidates.isEmpty || visibleDiagnostic != nil
  }

  /// 漢 shows while a Korean syllable composes, its list open or not, so a lone jamo is left to the Engine, which declines it, and the keyboard needs no jamo table. The tinted face says the list is open.
  private func updateHanjaButton() {
    let offersHanja = typesKorean && hasComposition
    let listOpen = koreanHanjaListOpen
    hanjaButton.isHidden = !offersHanja
    guard offersHanja, var configuration = hanjaButton.configuration else { return }
    let accent = candidatePalette?.accent ?? KeyboardTheme.current.accent
    configuration.baseForegroundColor = accent
    configuration.background.backgroundColor = listOpen ? accent.withAlphaComponent(0.22) : .clear
    hanjaButton.configuration = configuration
    hanjaButton.accessibilityLabel = listOpen ? "关闭汉字列表" : "转换为汉字"
    hanjaButton.accessibilityTraits = listOpen ? [.button, .selected] : .button
  }

  /// The 漢 button: MSIME_CONVERT_HANJA lists the Hanja of the composing syllable, or closes the open list and keeps the syllable composing. A lone jamo has no Hanja and the Engine leaves it alone, so the strip says why nothing opened.
  private func convertKoreanSyllableToHanja() {
    guard typesKorean, hasComposition else { return }
    playInputClick()
    let snapshot = session.convertHanja()
    render(snapshot)
    if !snapshot.isHandled {
      showDiagnostic("单个字母没有对应的汉字")
      renderCandidateStrip()
    }
  }

  private func candidateMarkers(at index: Int) -> [CandidateMarker] {
    CandidateMarker.markers(
      source: visibleCandidateSources.indices.contains(index) ? visibleCandidateSources[index] : 0,
      fixedPosition: visibleCandidateFixedPositions.indices.contains(index) ? visibleCandidateFixedPositions[index] : 0)
  }

  private func candidateAnnotation(at index: Int) -> KeyboardCandidateAnnotation {
    let code = visibleCandidateCodes.indices.contains(index) ? visibleCandidateCodes[index] : ""
    let engine = visibleCandidateAnnotations.indices.contains(index) ? visibleCandidateAnnotations[index] : ""
    let (hint, description) = codeHint(code: code, engine: engine, typed: visiblePreedit)
    return hint.isEmpty ? .none : KeyboardCandidateAnnotation(text: hint, accessibilityDescription: description)
  }

  /// The code shown under a candidate: the Wubi keys still to type, or else the Engine's own suffix - the candidate's helpcode when the scheme shows helpcodes, or the spelling a typo correction replaced. The Engine fills its suffix only when there is one to show, so the helpcode setting needs no second check here.
  private func codeHint(code: String, engine: String, typed: String) -> (String, String) {
    let wubi = wubiCodeHint(code: code, typed: typed)
    if !wubi.isEmpty { return (wubi, "还需输入 \(wubi)") }
    // A Hanja carries its 훈음 (meaning and reading) as the Engine's annotation; its code is only the key letters, which is not drawn (msime_client.h).
    if typesKorean { return engine.isEmpty ? ("", "") : (engine, "训音 \(engine)") }
    // Pinyin schemes only: that is where the Engine puts helpcodes and corrections, and what other schemes carry there is not something these settings govern.
    guard !engine.isEmpty, !isInLocalMode, inputScheme == .quanpin || usesShuangpin else { return ("", "") }
    // The Engine brackets its suffix for desktop windows that append it after the word; here it sits under the word like the Wubi hint, which is bare.
    let bare = engine.hasPrefix("(") && engine.hasSuffix(")") && engine.count > 2
      ? String(engine.dropFirst().dropLast()) : engine
    return (bare, "编码提示 \(bare)")
  }

  /// The gloss and translation lines under a strip candidate. A Korean Hanja's 훈음 is not one of them: it is drawn above them by the strip and never offered for insertion.
  private func candidateGlosses(at index: Int) -> [String] {
    guard CandidateGlossPreference.enabled, visibleCandidates.indices.contains(index) else { return [] }
    let word = visibleCandidates[index]
    var languages = [CandidateTranslationPreference.primary]
    if let secondary = CandidateTranslationPreference.secondary { languages.append(secondary) }
    return languages
      .filter {
        Self.canFillGloss($0, fullAccess: hasFullAccess, onlineRoute: translationRoute != .none,
                          offline: offlineGlossLanguages)
      }
      .map {
        gloss(word: word, language: $0,
              offline: visibleCandidateGlosses.indices.contains(index) ? visibleCandidateGlosses[index] : "")
          ?? Self.pendingGlossPlaceholder
      }
  }

  static let pendingGlossPlaceholder = " "

  private func glosses(word: String, offline: String) -> [String] {
    var lines: [String] = []
    if let value = gloss(word: word, language: CandidateTranslationPreference.primary, offline: offline) { lines.append(value) }
    if let secondary = CandidateTranslationPreference.secondary,
       let value = gloss(word: word, language: secondary, offline: "") { lines.append(value) }
    return lines
  }

  private func gloss(word: String, language: CandidateTranslationLanguage, at index: Int) -> String? {
    gloss(word: word, language: language,
          offline: visibleCandidateGlosses.indices.contains(index) ? visibleCandidateGlosses[index] : "")
  }

  private func gloss(word: String, language: CandidateTranslationLanguage, offline: String) -> String? {
    if !CandidateTranslationPreference.needsNetwork(language), glossesCandidates,
       !offline.isEmpty, offline != word { return offline }
    let target = !glossesCandidates ? nil : candidateTargetGlosses[language.code]?[word]
    let online = CandidateTranslationPreference.onlineEnabled ? translations.gloss(word: word, code: language.code) : nil
    return Self.preferredGloss(offline: target, online: online, route: translationRoute)
  }

  /// A non-English gloss, as on macOS: a service of the user's own outranks the offline dictionary, which outranks the 水杉 account.
  static func preferredGloss(offline: String?, online: String?, route: TranslationRoute) -> String? {
    switch route {
    case .account, .none: return offline ?? online
    case .niutrans, .tencent, .custom: return online ?? offline
    }
  }

  private func candidatePanelAnnotation(code: String, gloss: String, word: String, engine: String,
                                        typed: String) -> KeyboardCandidateAnnotation {
    let (hint, hintDescription) = codeHint(code: code, engine: engine, typed: typed)
    let lines = glosses(word: word, offline: gloss)
    let text = ([hint] + lines).filter { !$0.isEmpty }.joined(separator: "\n")
    guard !text.isEmpty else { return .none }
    let description = ([hintDescription, lines.isEmpty ? "" : "英文释义：\(lines.joined(separator: "，"))"])
      .filter { !$0.isEmpty }.joined(separator: "，")
    return KeyboardCandidateAnnotation(text: text, accessibilityDescription: description)
  }

  private func synchronizeTranslationRoute() {
    let route = TranslationProviderPreference.route(in: session.sharedPreferences)
    guard route != translationRoute || route.cacheScope != translations.scope else { return }
    translationRoute = route
    translations.use(route == .account ? BackendCandidateTranslationService() : ProviderCandidateTranslationService(route: route),
                     scope: route.cacheScope)
    DiagnosticLog.shared.write("translation_route provider=\(route.provider?.rawValue ?? "none")")
    // Rows reserved for network-only languages follow whether any service is chosen.
    applyCandidateGlossLayout()
  }

  private func requestCandidateTranslations() {
    guard CandidateGlossPreference.enabled, CandidateTranslationPreference.onlineEnabled,
          translationRoute != .none, hasFullAccess, glossesCandidates, !isInLocalMode,
          !visibleCandidates.isEmpty else { translations.cancel(); return }
    var codes = [CandidateTranslationPreference.primary.code]
    if let secondary = CandidateTranslationPreference.secondary { codes.append(secondary.code) }
    translations.refresh(words: Array(visibleCandidates.prefix(candidatePageSize)), codes: codes)
  }

  private func wubiCodeHint(code: String, typed: String) -> String {
    guard inputScheme == .wubi, !isInLocalMode,
          WubiCodeHintPreference.isEnabled else { return "" }
    return WubiCodeHintPreference.hint(
      code: code, typed: typed,
      answeredByPinyinFallback: visibleCandidatesAnsweredByPinyinFallback)
  }

  private func refreshCandidatePanelAnnotations() {
    guard let panel = candidatePanel, let generation = candidatePanelGeneration else { return }
    guard let value = try? session.allCandidates(),
          let snapshot = try? CandidatePanelSnapshot.decode(value),
          snapshot.generation == generation else {
      closeKeyboardPicker()
      return
    }
    panel.updateAnnotations(snapshot.entries.map {
      candidatePanelAnnotation(code: $0.code, gloss: $0.translation, word: $0.text, engine: $0.annotation,
                               typed: snapshot.preedit)
    })
  }

  /// Candidate gloss lookup is session-free disk work. Copy the complete candidate generation on
  /// the keyboard thread, then resolve it off-thread and apply only if the same composition is
  /// still visible. A failed or missing dictionary is intentionally silent.
  private func scheduleCandidateGlosses() {
    guard CandidateGlossPreference.enabled, glossesCandidates,
          !isInLocalMode, !visibleCandidates.isEmpty,
          let resources = session.candidateGlossResources(), !resources.isEmpty else {
      let hadVisibleGlosses = !visibleCandidateGlosses.isEmpty || !candidateTargetGlosses.isEmpty
      if candidateGlossRequestedGeneration != nil || hadVisibleGlosses {
        candidateGlossEpoch &+= 1
        candidateGlossRequestedGeneration = nil
        visibleCandidateGlosses = []
        candidateTargetGlosses = [:]
        if hadVisibleGlosses { renderCandidateStrip() }
      }
      refreshCandidatePanelAnnotations()
      candidateGlossTimer?.invalidate()
      candidateGlossTimer = nil
      return
    }
    // Ask once the typing pauses, not once per key.
    //
    // The request needs every candidate the query has, and reading them costs in proportion:
    // `yi` answers with hundreds, and that one call measured 3.5ms - more than the whole rest of
    // a keystroke. The answer is applied asynchronously and guarded by generation, so the glosses
    // for the compositions a fast typist passes through are fetched and then thrown away. Nobody
    // ever saw them.
    candidateGlossTimer?.invalidate()
    let timer = Timer(timeInterval: 0.12, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated {
        self?.candidateGlossTimer = nil
        self?.requestCandidateGlosses()
      }
    }
    RunLoop.main.add(timer, forMode: .common)
    candidateGlossTimer = timer
  }

  private func requestCandidateGlosses() {
    guard CandidateGlossPreference.enabled, glossesCandidates,
          !isInLocalMode, !visibleCandidates.isEmpty,
          let resources = session.candidateGlossResources(), !resources.isEmpty else { return }
    do {
      let allCandidates = try session.allCandidates()
      guard let value = allCandidates["generation"] as? NSNumber else { return }
      let generation = value.uint64Value
      if candidateGlossRequestedGeneration == generation { return }
      guard let candidates = allCandidates["candidates"] as? [[String: Any]] else { return }
      let request = try CandidateGlossModel.request(generation: generation, candidates: candidates)
      candidateGlossRequestedGeneration = generation
      let targetEpoch = candidateGlossEpoch
      let targetResources = resources
      let queue = candidateGlossQueue
      let applyOnMain: (UInt64, Data) -> Void = { [weak self] responseGeneration, translations in
        DispatchQueue.main.async { [weak self] in
          guard let self, self.candidateGlossEpoch == targetEpoch,
                CandidateGlossPreference.enabled,
                self.candidateGlossRequestedGeneration == responseGeneration else { return }
          do {
            let applied = try self.session.applyTranslations(
              generation: responseGeneration, translations: translations)
            guard applied["applied"] as? Bool == true else { return }
            let snapshot = try self.session.snapshot(from: applied)
            self.render(snapshot)
          } catch {
            // Optional display metadata must never interrupt input.
          }
        }
      }
      queue.async {
        do {
          let response = try MetasequoiaInputSessionBridge.candidateGlosses(
            request: request, resources: targetResources)
          let decoded = try CandidateGlossModel.decode(response)
          guard decoded.generation == generation else { return }
          applyOnMain(decoded.generation, decoded.translations)
        } catch {
          // Optional display metadata must never interrupt input.
        }
      }
      requestCandidateTargetGlosses(generation: generation, candidates: candidates, resources: resources)
    } catch {
      // Optional display metadata must never interrupt input.
    }
  }

  /// The non-English targets with an installed offline dictionary, each asked on the same queue as English. These do not go through `applyTranslations`: the session keeps one gloss per candidate, and that one is English.
  private func requestCandidateTargetGlosses(generation: UInt64, candidates: [[String: Any]], resources: String) {
    var languages = [CandidateTranslationPreference.primary]
    if let secondary = CandidateTranslationPreference.secondary { languages.append(secondary) }
    let installed = offlineGlossLanguages
    let targetEpoch = candidateGlossEpoch
    for code in languages.map(\.code) where installed.contains(code) {
      guard let request = try? CandidateGlossModel.request(generation: generation, candidates: candidates,
                                                          targetLanguage: code) else { continue }
      candidateGlossQueue.async { [weak self] in
        guard let response = try? MetasequoiaInputSessionBridge.candidateGlosses(request: request, resources: resources),
              let decoded = try? CandidateGlossModel.decode(response), decoded.generation == generation,
              let entries = try? JSONSerialization.jsonObject(with: decoded.translations) as? [[String: String]]
        else { return }
        var glosses: [String: String] = [:]
        for entry in entries {
          if let text = entry["text"], let translation = entry["translation"] { glosses[text] = translation }
        }
        DispatchQueue.main.async { [weak self] in
          guard let self, self.candidateGlossEpoch == targetEpoch, CandidateGlossPreference.enabled,
                self.candidateGlossRequestedGeneration == generation else { return }
          self.candidateTargetGlosses[code] = glosses
          self.renderCandidateStrip()
          self.refreshCandidatePanelAnnotations()
        }
      }
    }
  }

  /// The source logo is white-backed and has no alpha, so use its luminance as a mask before tinting it with the current skin accent. This prevents a white square on dark skins. The expanded candidate panel leads its header with the same template.
  static func brandTemplate() -> UIImage? {
    guard let path = Bundle(for: KeyboardViewController.self).path(forResource: "KeyboardBrand", ofType: "png"),
          let source = UIImage(contentsOfFile: path),
          let cgImage = source.cgImage else { return nil }
    let input = CIImage(cgImage: cgImage)
    guard let inverted = CIFilter(name: "CIColorInvert", parameters: [kCIInputImageKey: input])?.outputImage,
          let masked = CIFilter(name: "CIMaskToAlpha", parameters: [kCIInputImageKey: inverted])?.outputImage,
          let output = CIContext().createCGImage(masked, from: masked.extent) else { return nil }
    return UIImage(cgImage: output).withRenderingMode(.alwaysTemplate)
  }

  // A touch keyboard has no number row to answer with, so the ordinal is spoken rather than drawn;
  // the index is the engine position the chip selects. The expand panel already showed bare text.
  private func makeCandidateButton(index: Int) -> KeyboardKeyButton {
    var configuration = UIButton.Configuration.plain()
    configuration.titleLineBreakMode = .byTruncatingTail
    configuration.baseForegroundColor = KeyboardTheme.current.keyForeground
    configuration.contentInsets = NSDirectionalEdgeInsets(
      top: 4, leading: 9, bottom: 4, trailing: 9)
    // Candidates sit on the keyboard with no chip of their own (dc.html `mobCands`); the fill only shows while converting or with the desktop palette.
    configuration.background.backgroundColor = .clear
    configuration.background.strokeWidth = 0
    configuration.background.cornerRadius = 9

    let button = KeyboardKeyButton(
      configuration: configuration,
      primaryAction: UIAction { [weak self] _ in
        guard let self else { return }
        self.playInputClick()
        self.selectCandidate(at: index)
      })
    button.setContentCompressionResistancePriority(.required, for: .horizontal)
    button.accessibilityIdentifier = "candidate-\(index + 1)"
    button.menu = UIMenu(children: [
      UIDeferredMenuElement.uncached { [weak self] completion in
        completion(self?.candidateMenuElements(at: index) ?? [])
      }
    ])
    // Key feedback without a key surface: a candidate is text, not a key.
    button.addTarget(self, action: #selector(prepareKeyFeedback), for: .touchDown)
    return button
  }

  private func selectCandidate(at index: Int) {
    guard visibleCandidates.indices.contains(index) else { return }
    if inputScheme == .handwriting, !handwritingResults.isEmpty {
      if handwriting.use(at: index) { handwritingResults = [] }
      return
    }
    if !isChineseMode {
      useEnglishSuggestion(at: index)
      return
    }
    render(session.selectCandidate(at: UInt(index)))
  }

  private func candidateColumnWidth() -> CGFloat {
    KeyboardKeyButton.glossColumnWidth(
      visible: candidateScrollView.bounds.width, spacing: candidateStack.spacing,
      insets: NSDirectionalEdgeInsets(top: 4, leading: 9, bottom: 4, trailing: 9))
  }

  /// `leadLine` is the width of a line under the candidate that must not be cut, a Korean Hanja's 훈음, which widens the chip as the candidate itself would.
  private func pinCandidateWidth(
    of button: KeyboardKeyButton, firstLine title: AttributedString?, glossLines: Int, leadLine: CGFloat = 0
  ) {
    let existing = button.constraints.first { $0.identifier == "candidateChipWidth" }
    guard glossLines > 0, let title else {
      existing?.isActive = false
      return
    }
    let text = NSAttributedString(title)
    let separator = (text.string as NSString).range(of: "\n")
    let head = separator.location == NSNotFound
      ? NSRange(location: 0, length: text.length)
      : NSRange(location: 0, length: separator.location)
    let width = KeyboardKeyButton.chipWidth(
      titleLine: max(text.attributedSubstring(from: head).size().width, leadLine),
      glossLines: glossLines, column: candidateColumnWidth(),
      insets: button.configuration?.contentInsets ?? .zero)
    if let existing {
      existing.constant = width
      existing.isActive = true
    } else {
      let constraint = button.widthAnchor.constraint(equalToConstant: width)
      constraint.identifier = "candidateChipWidth"
      constraint.isActive = true
    }
  }

  private func updateCandidateButton(
    _ button: KeyboardKeyButton, candidate: String, hint: String, reading: String? = nil, glosses: [String],
    markers: [CandidateMarker] = [], number: Int, converting: Bool
  ) {
    let display = chineseOutput(candidate)
    guard var configuration = button.configuration else { return }
    let skin = KeyboardTheme.current
    let annotationColor = candidatePalette?.secondary ?? skin.secondary
    if let palette = candidatePalette {
      // Drawn flat like the desktop candidate window: the first candidate, the one space commits, carries the skin's highlight.
      configuration.background.customView = nil
      configuration.background.backgroundColor = converting ? palette.selected.withAlphaComponent(0.35)
        : number == 1 ? palette.hover : palette.surface
      configuration.background.cornerRadius = 9
      configuration.background.strokeWidth = 1
      configuration.background.strokeColor = palette.border
      configuration.baseForegroundColor = palette.text
      button.layer.shadowOpacity = 0
    } else {
      configuration.background.customView = nil
      configuration.background.backgroundColor = converting ? skin.accent.withAlphaComponent(0.22) : .clear
      configuration.background.cornerRadius = 9
      configuration.background.strokeWidth = 0
      button.layer.shadowOpacity = 0
    }
    let annotation = hint
    // A Korean Hanja's 훈음 leads the lines under it, ahead of any gloss; one with none keeps the line so the glosses stay aligned across chips.
    let lines = (reading.map { [$0.isEmpty ? Self.pendingGlossPlaceholder : $0] } ?? []) + glosses
    // On the keyboard's own colours the first candidate, the one space commits, is the selected one: the accent at weight 600 while the rest stay in the key text colour at 400 (dc.html `mobCands`). The desktop palette marks it with its fill instead.
    let selected = candidatePalette == nil && number == 1
    let weight: UIFont.Weight = selected ? .semibold : .regular
    // A pinned word takes the accent colour, as it does in the Windows candidate window; chips are reused, so every other word is set back.
    let pinned = markers.contains { $0.symbol == "pin.fill" }
    configuration.baseForegroundColor = pinned || selected
      ? candidatePalette?.accent ?? skin.accent
      : candidatePalette?.text ?? skin.keyForeground
    // The 훈음 is drawn whole, like an inline hint was, so the chip widens to it rather than cutting it to the gloss column; the glosses under it fit that width.
    var readingWidth: CGFloat = 0
    if annotation.isEmpty && lines.isEmpty && markers.isEmpty {
      configuration.titleLineBreakMode = .byTruncatingTail
      configuration.attributedTitle = nil
      configuration.titleTextAttributesTransformer = Self.fontTransformer(
        .body, scale: candidateFontScale, families: candidateFontFamilies, weight: weight)
      configuration.title = display
    } else {
      configuration.titleLineBreakMode = .byWordWrapping
      // The runs below carry their own fonts; a transformer would flatten the gloss lines to the candidate size.
      configuration.titleTextAttributesTransformer = nil
      let paragraph = NSMutableParagraphStyle()
      paragraph.alignment = .natural
      paragraph.lineBreakMode = .byTruncatingTail
      var title = AttributedString(display, attributes: AttributeContainer([
        .font: Self.weighted(CandidateFontPreference.font(.body, scale: candidateFontScale, families: candidateFontFamilies), weight),
        .paragraphStyle: paragraph,
      ]))
      title += Self.markerRun(markers, color: annotationColor, scale: candidateFontScale)
      if !annotation.isEmpty {
        title += AttributedString(" " + annotation, attributes: AttributeContainer([
          .font: CandidateFontPreference.font(.caption1, scale: candidateFontScale), .paragraphStyle: paragraph,
          .foregroundColor: annotationColor,
        ]))
      }
      let caption = UIFont.preferredFont(forTextStyle: .caption2)
      readingWidth = reading.map { ceil(($0 as NSString).size(withAttributes: [.font: caption]).width) } ?? 0
      let content = KeyboardKeyButton.chipContentWidth(
        titleLine: max(NSAttributedString(title).size().width, readingWidth), glossLines: lines.count,
        column: candidateColumnWidth())
      for gloss in lines {
        let fitted = KeyboardKeyButton.fittedGloss(gloss, font: caption, width: content)
        title += AttributedString("\n" + fitted.text, attributes: AttributeContainer([
          .font: fitted.font, .paragraphStyle: paragraph,
          .foregroundColor: annotationColor,
        ]))
      }
      configuration.attributedTitle = title
    }
    button.configuration = configuration
    button.titleLineCount = 1 + lines.count
    pinCandidateWidth(of: button, firstLine: configuration.attributedTitle, glossLines: lines.count,
                      leadLine: readingWidth)
    button.accessibilityLabel = annotation.isEmpty
      ? "候选词 \(number)：\(display)"
      : "候选词 \(number)：\(display)，还需输入 \(annotation)"
    // A Korean Hanja's 훈음 is its meaning and reading, not keys still to type.
    if let reading, !reading.isEmpty { button.accessibilityLabel? += "，训音 \(reading)" }
    for marker in markers { button.accessibilityLabel? += "，\(marker.spoken)" }
    let spoken = glosses.filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
    if !spoken.isEmpty {
      button.accessibilityLabel? += "，释义 " + spoken.joined(separator: "，")
    }
    // A Hanja's long press offers nothing (candidateMenuElements(at:)), so its glosses are read but not offered for insertion.
    button.accessibilityHint = spoken.isEmpty || reading != nil ? nil : "轻点输入，长按可输入释义"
  }

  /// The markers as symbol attachments after the word, at the annotation's size and colour so they read as a suffix rather than as part of the candidate.
  static func markerRun(_ markers: [CandidateMarker], color: UIColor, scale: CGFloat) -> AttributedString {
    let font = CandidateFontPreference.font(.caption1, scale: scale)
    var run = AttributedString()
    for marker in markers {
      guard let image = UIImage(systemName: marker.symbol, withConfiguration: UIImage.SymbolConfiguration(font: font))?
        .withTintColor(color, renderingMode: .alwaysOriginal) else { continue }
      run += AttributedString(" ")
      run += AttributedString(NSAttributedString(attachment: NSTextAttachment(image: image)))
    }
    return run
  }

  /// What a long press on a candidate's gloss offers. The action is deferred until the menu opens,
  /// so a chip reused for another composition cannot insert a stale translation.
  private func glossMenuElements(
    _ glosses: [String], isCurrent: @escaping () -> Bool
  ) -> [UIMenuElement] {
    glosses
      .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
      .filter { !$0.isEmpty }
      .map { gloss in
        UIAction(title: gloss, image: UIImage(systemName: "character.bubble")) { [weak self] _ in
          guard let self, isCurrent() else { return }
          playInputClick()
          insertOwnText(gloss, source: .local)
          render(session.cancel())
          UIAccessibility.post(notification: .announcement, argument: "已输入 \(gloss)")
        }
      }
  }

  private func glossMenuElements(at index: Int) -> [UIMenuElement] {
    guard visibleCandidates.indices.contains(index) else { return [] }
    let candidate = visibleCandidates[index]
    let revision = candidateRevision
    return glossMenuElements(candidateGlosses(at: index)) { [weak self] in
      guard let self, candidateRevision == revision,
            visibleCandidates.indices.contains(index), visibleCandidates[index] == candidate else {
        return false
      }
      return true
    }
  }

  /// What a long press on a strip candidate offers.
  ///
  /// The expanded panel shares this menu through the overload below. It built its own chips and
  /// gave them none, so the press that managed an entry on the strip did nothing once the list was
  /// expanded -- and the expanded list is exactly where a rarely used entry is reached.
  func candidateMenuElements(at index: Int) -> [UIMenuElement] {
    guard isChineseMode, inputScheme.writesChinese, !isInLocalMode,
          visibleCandidates.indices.contains(index) else { return [] }
    let candidate = visibleCandidates[index]
    let revision = candidateRevision
    let glosses = glossMenuElements(at: index)
    let fixedPosition = visibleCandidateFixedPositions.indices.contains(index) ? visibleCandidateFixedPositions[index] : 0
    let management = candidateMenuElements(candidate: candidate, fixedPosition: fixedPosition) { [weak self] operation in
      guard let self, candidateRevision == revision,
            visibleCandidates.indices.contains(index), visibleCandidates[index] == candidate else { return nil }
      return session.editCandidate(at: UInt(index), expectedWord: candidate, action: operation)
    }
    let edges = wordCharacterMenuElements(candidate: candidate) { [weak self] last in
      guard let self, candidateRevision == revision,
            visibleCandidates.indices.contains(index), visibleCandidates[index] == candidate else { return nil }
      return session.selectCandidateEdge(at: UInt(index), last: last)
    }
    return edges + glosses + management
  }

  /// The same menu for a candidate identified the way the expanded panel holds it. Panel positions
  /// index the engine's whole answer, not the visible strip, so they cannot use the visible index.
  func candidateMenuElements(generation: UInt64, globalIndex: UInt64) -> [UIMenuElement] {
    guard isChineseMode, inputScheme.writesChinese, !isInLocalMode else { return [] }
    let entry = (try? CandidatePanelSnapshot.decode(session.allCandidates()))
      .flatMap { $0.generation == generation ? $0.entries.first { $0.index == globalIndex } : nil }
    return candidateMenuElements(candidate: entry?.text, fixedPosition: entry?.fixedPosition ?? 0) { [weak self] operation in
      self?.session.editCandidate(generation: generation, globalIndex: globalIndex, action: operation)
    }
  }

  private func candidateMenuElements(
    generation: UInt64, globalIndex: UInt64, candidate: String, offlineGloss: String, fixedPosition: Int
  ) -> [UIMenuElement] {
    guard isChineseMode, inputScheme.writesChinese, !isInLocalMode else { return [] }
    let glosses = glossMenuElements(glosses(word: candidate, offline: offlineGloss)) { [weak self] in
      guard let self else { return false }
      guard let snapshot = try? session.allCandidates(),
            let current = try? CandidatePanelSnapshot.decode(snapshot),
            current.generation == generation,
            current.entries.contains(where: { $0.index == globalIndex && $0.text == candidate }) else {
        return false
      }
      return true
    }
    guard session.isOnCurrentPage(generation: generation, globalIndex: globalIndex) else { return glosses }
    let management = candidateMenuElements(candidate: candidate, fixedPosition: fixedPosition) { [weak self] operation in
      self?.session.editCandidate(generation: generation, globalIndex: globalIndex, action: operation)
    }
    let edges = wordCharacterMenuElements(candidate: candidate) { [weak self] last in
      guard let self else { return nil }
      closeKeyboardPicker()
      return session.selectCandidateEdge(generation: generation, globalIndex: globalIndex, last: last)
    }
    return edges + glosses + management
  }

  /// 以词定字: commit just the first or the last Han character of a longer candidate. Desktop hosts bind it to [ ] or - =; a keyboard extension receives no hardware keys, even on an iPad with one attached, so here it lives in the candidate's long-press menu and follows only the on/off half of the shared `word_character` preference.
  private func wordCharacterMenuElements(
    candidate: String, select: @escaping (_ last: Bool) -> MetasequoiaInputSnapshot?
  ) -> [UIMenuElement] {
    let stored = session.sharedPreferences?["word_character"] as? [String: Any] ?? [:]
    let han = candidate.filter { $0.unicodeScalars.first?.properties.isUnifiedIdeograph == true }
    guard stored["enabled"] as? Bool ?? true, han.count > 1, let first = han.first, let last = han.last else { return [] }
    func action(_ title: String, _ character: Character, last: Bool) -> UIAction {
      UIAction(title: "\(title)「\(chineseOutput(String(character)))」", image: UIImage(systemName: last ? "text.insert" : "text.append")) { [weak self] _ in
        guard let self, let result = select(last) else { return }
        playInputClick()
        render(result)
        if !result.isHandled { showDiagnostic("当前候选不支持以词定字") }
      }
    }
    return [UIMenu(title: "以词定字", options: .displayInline, children: [
      action("只上屏首字", first, last: false), action("只上屏末字", last, last: true),
    ])]
  }

  /// `fixedPosition` is the slot the word is pinned to, zero when it is not: the menu checks that slot and offers 取消固定 only then. Deleting is not offered for a single character, the same as the Windows candidate menu.
  private func candidateMenuElements(
    candidate: String?, fixedPosition: Int,
    edit: @escaping (MetasequoiaCandidateAction) -> MetasequoiaInputSnapshot?
  ) -> [UIMenuElement] {
    func action(_ title: String, _ symbol: String?, _ operation: MetasequoiaCandidateAction,
                destructive: Bool = false, announcement: String? = nil) -> UIAction {
      UIAction(title: title, image: symbol.flatMap { UIImage(systemName: $0) },
               attributes: destructive ? .destructive : []) { [weak self] _ in
        guard let self, let result = edit(operation) else { return }
        render(result)
        if !result.isHandled { showDiagnostic("当前候选不支持此操作") }
        else if result.diagnosticText == nil {
          playInputClick()
          UIAccessibility.post(notification: .announcement, argument: announcement ?? "已\(title)")
        }
      }
    }
    // The desktop candidate menu fixes a word at any of the first five slots, not only the first; a submenu keeps the five choices out of the top level, where a phone has room for few rows.
    let positions = (UInt8(1)...5).map { position in
      let item = action("第 \(position) 位", nil, .fix(position: position), announcement: "已固定到第 \(position) 位")
      item.state = Int(position) == fixedPosition ? .on : .off
      return item
    }
    var elements: [UIMenuElement] = [
      action("优先显示", "arrow.up", .promote),
      UIMenu(title: "固定排位", image: UIImage(systemName: "pin"), children: positions),
    ]
    if fixedPosition > 0 { elements.append(action("取消固定", "pin.slash", .clearPosition)) }
    if candidate.map({ $0.unicodeScalars.count != 1 }) ?? true {
      elements.append(UIMenu(title: "删除词条…", image: UIImage(systemName: "trash"), options: .destructive, children: [
        action("确认删除此词条", "trash", .remove, destructive: true),
      ]))
    }
    return elements
  }

  private func makeSymbolKey(
    symbol: String, accessibilityLabel: String, action: (() -> Void)? = nil
  ) -> UIButton {
    var configuration = UIButton.Configuration.plain()
    configuration.image = UIImage(systemName: symbol)
    configuration.baseForegroundColor = KeyboardTheme.current.keyForeground
    configuration.background.backgroundColor = KeyboardTheme.current.functionKeyBackground
    configuration.background.cornerRadius = 8
    let button = KeyboardKeyButton(configuration: configuration)
    button.isFunctionKey = true
    if let action {
      button.addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
    }
    button.accessibilityLabel = accessibilityLabel
    decorateKey(button)
    return button
  }

  private func makeKey(
    title: String,
    accessibilityLabel: String,
    emphasized: Bool = false,
    function: Bool = false,
    action: @escaping () -> Void
  ) -> UIButton {
    var configuration = UIButton.Configuration.plain()
    configuration.title = title
    configuration.titleLineBreakMode = .byClipping
    configuration.baseForegroundColor = emphasized ? KeyboardTheme.current.actionForeground : KeyboardTheme.current.keyForeground
    configuration.background.backgroundColor =
      emphasized
      ? KeyboardTheme.current.actionBackground
      : function ? KeyboardTheme.current.functionKeyBackground : KeyboardTheme.current.keyBackground
    configuration.background.cornerRadius = 8
    configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
      attributes in
      var attributes = attributes
      attributes.font = KeyboardTheme.current.usesMonospacedFont
        ? .monospacedSystemFont(ofSize: UIFont.preferredFont(forTextStyle: .title3).pointSize, weight: .medium)
        : .preferredFont(forTextStyle: .title3)
      return attributes
    }
    let button = KeyboardKeyButton(configuration: configuration, primaryAction: UIAction { _ in action() })
    button.isFunctionKey = function
    button.accessibilityLabel = accessibilityLabel
    decorateKey(button)
    return button
  }

  private func decorateKey(_ button: UIButton) {
    button.addTarget(self, action: #selector(prepareKeyFeedback), for: .touchDown)
    let skin = KeyboardTheme.current
    guard var configuration = button.configuration else { return }
    if let design = skin.design {
      let surface = SkinKeySurfaceView()
      surface.design = design
      surface.fillColor = configuration.background.backgroundColor ?? skin.keyBackground
      configuration.background.customView = surface
      configuration.background.backgroundColor = .clear
      configuration.background.cornerRadius = 0
      configuration.background.strokeWidth = 0
      button.configuration = configuration
      button.layer.shadowOpacity = 0
      return
    }
    configuration.background.customView = nil
    configuration.background.cornerRadius = skin.cornerRadius
    configuration.background.strokeWidth = skin.borderWidth
    configuration.background.strokeColor = skin.borderColor
    button.configuration = configuration
    button.layer.shadowColor = skin.shadowColor.resolvedColor(with: button.traitCollection).cgColor
    button.layer.shadowOpacity = skin.hasShadow ? 1 : 0
    button.layer.shadowRadius = skin.shadowRadius
    button.layer.shadowOffset = CGSize(width: 0, height: skin.shadowOffset)
  }

  override func viewDidLayoutSubviews() {
    super.viewDidLayoutSubviews()
    let column = candidateColumnWidth()
    if abs(column - appliedCandidateColumnWidth) > 0.5 {
      appliedCandidateColumnWidth = column
      if !visibleCandidates.isEmpty { renderCandidateStrip() }
    }
    updateLetterRowInsets()
    if let globe = actionGlobeButton, globe.isHidden != !needsInputModeSwitchKey {
      updateKeyboardLayout()
    }
    updatePreferredKeyboardHeight()
    func updateShadows(_ node: UIView) {
      if let button = node as? UIButton, button.layer.shadowOpacity > 0 {
        button.layer.shadowPath = UIBezierPath(roundedRect: button.bounds,
          cornerRadius: KeyboardTheme.current.cornerRadius).cgPath
      }
      node.subviews.forEach { updateShadows($0) }
    }
    updateShadows(view)
  }

  private func updatePreferredKeyboardHeight() {
    let landscape = view.window?.windowScene?.interfaceOrientation.isLandscape
      ?? (traitCollection.verticalSizeClass == .compact)
    // The composition line added a row to the candidate strip; the keyboard grew by it rather than
    // taking the space out of the keys.
    let extra = currentStripExtraHeight
    // Handwriting shares the candidate strip and therefore the common portrait height.
    let height = formFactor.baseHeight(
      landscape: landscape, handwriting: !handwriting.isHidden,
      numberRow: numberRowView?.isHidden == false || zhuyinRowViews.first?.isHidden == false) + extra
    let adjustedHeight = height + sharedKeyboardHeightAdjustment
    if keyboardHeightConstraint?.constant != adjustedHeight { keyboardHeightConstraint?.constant = adjustedHeight }
  }

  private func showClipboardHistory() {
    closeKeyboardService()
    closeKeyboardPicker()
    // No 云端 tab at all in a password or one-time-code field, so nothing is fetched there either.
    let cloud = isCredentialField ? nil : KeyboardCloudClipboard(hasFullAccess: hasFullAccess)
    cloud?.fieldAllowsCloud = { [weak self] in self.map { !$0.isCredentialField } ?? false }
    let panel = KeyboardClipboardView(hasFullAccess: hasFullAccess, cloud: cloud, onInsert: { [weak self] text in
      guard let self else { return }
      render(session.finishComposition())
      insertOwnText(text)
      closeKeyboardPicker()
    }, onClose: { [weak self] in self?.closeKeyboardPicker() })
    panel.accessibilityViewIsModal = true
    panel.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(panel)
    NSLayoutConstraint.activate([
      panel.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      panel.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      panel.topAnchor.constraint(equalTo: view.topAnchor),
      panel.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
    clipboardPanel = panel
    UIAccessibility.post(notification: .screenChanged, argument: panel)
  }

  private func showLayoutPicker() {
    closeKeyboardService()
    closeKeyboardPicker()
    // The keyboard has to stay visible while it is being adjusted, so the shortcut bar stays too --
    // it sits under the toolbar and is out of the way. Hiding it was for the opaque slider panel.
    let picker = KeyboardLayoutPickerView(
      keySpacing: KeyboardLayoutPreference.keySpacing,
      rowSpacing: KeyboardLayoutPreference.rowSpacing,
      height: KeyboardLayoutPreference.heightAdjustment,
      onKeySpacing: { [weak self] spacing in
        KeyboardLayoutPreference.keySpacing = spacing
        self?.applyLayoutPreferences()
        self?.persistTouchKeyboardGeometry()
      },
      onRowSpacing: { [weak self] spacing in
        KeyboardLayoutPreference.rowSpacing = spacing
        self?.applyLayoutPreferences()
        self?.persistTouchKeyboardGeometry()
      },
      onHeight: { [weak self] adjustment in
        KeyboardLayoutPreference.heightAdjustment = adjustment
        self?.sharedKeyboardHeightAdjustment = CGFloat(adjustment)
        self?.updatePreferredKeyboardHeight()
        self?.persistTouchKeyboardGeometry()
      },
      onCommit: { [weak self] in self?.commitTouchKeyboardGeometry() },
      onReset: { [weak self] in
        guard let self else { return }
        KeyboardLayoutPreference.resetToDefaults()
        sharedKeyboardHeightAdjustment = 0
        applyLayoutPreferences()
        updatePreferredKeyboardHeight()
        updateShortcutButtons()
        _ = session.resetTouchKeyboardGeometry()
        showLayoutPicker()
      },
      onClose: { [weak self] in
        guard let self else { return }
        updateKeyboardLayout()
        closeKeyboardPicker()
      })
    picker.accessibilityIdentifier = "keyboardLayoutPicker"
    picker.accessibilityViewIsModal = true
    picker.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(picker)
    NSLayoutConstraint.activate([
      picker.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      picker.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      picker.topAnchor.constraint(equalTo: view.topAnchor),
      picker.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
    layoutPicker = picker
    UIAccessibility.post(notification: .screenChanged, argument: picker)
  }

  private func persistTouchKeyboardGeometry() {
    _ = session.setTouchKeyboardGeometry(
      keySpacing: KeyboardLayoutPreference.keySpacing,
      rowSpacing: KeyboardLayoutPreference.rowSpacing,
      heightAdjustment: KeyboardLayoutPreference.heightAdjustment,
      voiceEnabled: KeyboardLayoutPreference.voiceShortcutEnabled)
  }

  // The drags report on every gesture frame; the shared document is written once the value has
  // settled, so a single adjustment does not take a file lock a hundred times.
  private func commitTouchKeyboardGeometry() {
    _ = session.persistTouchKeyboardGeometry(
      keySpacing: KeyboardLayoutPreference.keySpacing,
      rowSpacing: KeyboardLayoutPreference.rowSpacing,
      heightAdjustment: KeyboardLayoutPreference.heightAdjustment,
      voiceEnabled: KeyboardLayoutPreference.voiceShortcutEnabled)
  }

  private func showMorePicker() {
    closeKeyboardService()
    closeKeyboardPicker()
    updateShortcutButtons()
    moreToolsPage = .root
    if moreTools.isEmpty { moreTools = makeToolSections() }
    let picker = KeyboardMorePickerView(sections: moreTools,
      onClose: { [weak self] in self?.closeKeyboardPicker() })
    picker.accessibilityViewIsModal = true
    picker.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(picker)
    NSLayoutConstraint.activate([
      picker.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      picker.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      picker.topAnchor.constraint(equalTo: view.topAnchor),
      picker.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
    morePicker = picker
    UIAccessibility.post(notification: .screenChanged, argument: picker)
  }

  private func selectTraditionalOutput(_ traditional: Bool) {
    usesTraditionalOutput = traditional
    ChineseOutputPreference.usesTraditional = traditional
    _ = session.setTraditionalChineseOutput(traditional)
    renderCandidateStrip()
    updateShortcutButtons()
  }

  private func showEmojiPicker() {
    closeKeyboardService()
    closeKeyboardPicker()
    guard let resources = session.candidateGlossResources(), !resources.isEmpty else {
      showDiagnostic("表情目录尚未就绪")
      return
    }
    // Browsing is a separate input surface. Finish any pinyin first so selecting an Emoji cannot
    // reorder it ahead of text that was already composed.
    render(session.finishComposition())
    let picker = KeyboardEmojiPickerView(
      resources: resources,
      onInsert: { [weak self] emoji in self?.insertOwnText(emoji, source: .local) },
      onDelete: { [weak self] in
        self?.countKeyPress(TypingKeyID.backspace)
        self?.deleteOwnBackward()
      },
      onClose: { [weak self] in self?.closeKeyboardPicker() })
    picker.accessibilityViewIsModal = true
    picker.overrideUserInterfaceStyle = KeyboardAppearancePreference.style(KeyboardAppearancePreference.emojiKey, in: session.sharedPreferences)
    picker.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(picker)
    NSLayoutConstraint.activate([
      picker.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      picker.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      picker.topAnchor.constraint(equalTo: view.topAnchor),
      picker.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
    emojiPicker = picker
    UIAccessibility.post(notification: .screenChanged, argument: picker)
  }

  /// Replace the keyboard with the categorized symbol surface, finishing any active composition
  /// before direct local insertion can occur.
  private func showSymbolPanel() {
    closeKeyboardService()
    closeKeyboardPicker()
    playInputClick()
    render(session.finishComposition())
    // Without staged resources the phone categories still work; only the Engine catalog's categories are missing.
    let catalog = session.candidateGlossResources().flatMap { $0.isEmpty ? nil : KeyboardSymbolPanelView.Catalog.engine(resources: $0) }
    let panel = KeyboardSymbolPanelView(catalog: catalog, recents: KeyboardSymbolRecents.stored, onInsert: { [weak self] symbol in
      self?.countKeyPress(TypingKeyID.character(symbol))
      self?.playInputClick()
      self?.insertOwnText(symbol, source: .local)
      KeyboardSymbolRecents.record(symbol)
    }, onDelete: { [weak self] in
      self?.countKeyPress(TypingKeyID.backspace)
      self?.deleteOwnBackward()
    }, onClose: { [weak self] in self?.closeKeyboardPicker() })
    panel.accessibilityViewIsModal = true
    panel.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(panel)
    NSLayoutConstraint.activate([
      panel.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      panel.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      panel.topAnchor.constraint(equalTo: view.topAnchor),
      panel.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
    symbolPanel = panel
    UIAccessibility.post(notification: .screenChanged, argument: panel)
  }

  private func showSchemePicker() {
    closeKeyboardService()
    closeKeyboardPicker()
    let picker = KeyboardSchemePickerView(selected: inputScheme, isChineseMode: isChineseMode, onSelect: { [weak self] scheme in
      guard let self else { return }
      closeKeyboardPicker()
      if !isChineseMode { toggleInputMode() }
      selectInputScheme(scheme)
    }, onSelectEnglish: { [weak self] in
      guard let self else { return }
      closeKeyboardPicker()
      if isChineseMode { toggleInputMode() }
    }, onSettings: { [weak self] in
      self?.showMorePicker()
    }, onClose: { [weak self] in self?.closeKeyboardPicker() })
    picker.accessibilityViewIsModal = true
    picker.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(picker)
    NSLayoutConstraint.activate([
      picker.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      picker.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      picker.topAnchor.constraint(equalTo: view.topAnchor),
      picker.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
    schemePicker = picker
    UIAccessibility.post(notification: .screenChanged, argument: picker)
  }

  private func showSkinPicker() {
    guard skinPicker == nil else { return }
    closeKeyboardService()
    closeKeyboardPicker()
    let picker = KeyboardSkinPickerView(selected: KeyboardTheme.current.id, document: session.sharedPreferences, onSelect: { [weak self] id in
      self?.selectTheme(GlobalThemePreference.selecting(id))
    }, onSelectDesign: { [weak self] design in
      guard let mapping = GlobalThemePreference.applyingDesign(design) else { return }
      self?.selectTheme(mapping)
    }, onClose: { [weak self] in self?.closeKeyboardPicker() })
    picker.accessibilityViewIsModal = true
    picker.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(picker)
    NSLayoutConstraint.activate([
      picker.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      picker.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      picker.topAnchor.constraint(equalTo: view.topAnchor),
      picker.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
    skinPicker = picker
    UIAccessibility.post(notification: .screenChanged, argument: picker)
  }

  /// Write a theme choice made in the keyboard to the shared document, then draw it. The App Group copy follows the document, as it does when the app writes the choice.
  private func selectTheme(_ mapping: @escaping (inout [String: Any]) -> Void) {
    if session.updateTheme(mapping), let document = session.sharedPreferences { GlobalThemePreference.mirror(document) }
    KeyboardTheme.reload(session.sharedPreferences)
    closeKeyboardPicker()
    applyKeyboardAppearance()
    applyKeyboardSkin()
    playInputClick()
  }

  private func closeKeyboardPicker() {
    dismissNineKeyHoldOptions()
    if let panel = candidatePanel {
      panel.removeFromSuperview()
      candidatePanel = nil
      candidatePanelGeneration = nil
      UIAccessibility.post(notification: .screenChanged, argument: expandCandidatesButton)
    }
    if let picker = layoutPicker {
      picker.removeFromSuperview()
      layoutPicker = nil
      shortcutBar.isHidden = false
      UIAccessibility.post(notification: .screenChanged, argument: layoutShortcut)
    }
    if let picker = morePicker {
      picker.removeFromSuperview()
      morePicker = nil
      UIAccessibility.post(notification: .screenChanged, argument: moreShortcut)
    }
    if let picker = schemePicker {
      picker.removeFromSuperview()
      schemePicker = nil
      UIAccessibility.post(notification: .screenChanged, argument: schemeButton)
    }
    if let panel = clipboardPanel {
      panel.removeFromSuperview()
      clipboardPanel = nil
      UIAccessibility.post(notification: .screenChanged, argument: moreShortcut)
    }
    if let picker = emojiPicker {
      picker.removeFromSuperview()
      emojiPicker = nil
      UIAccessibility.post(notification: .screenChanged, argument: emojiShortcut)
    }
    if let panel = symbolPanel {
      panel.removeFromSuperview()
      symbolPanel = nil
      UIAccessibility.post(notification: .screenChanged, argument: nineKeySymbolsButton)
    }
    if let picker = skinPicker {
      picker.removeFromSuperview()
      skinPicker = nil
      UIAccessibility.post(notification: .screenChanged, argument: skinShortcut)
    }
  }

  /// The top row sits straight on the keyboard in the design (dc.html `mobCands`, the 48px row); the desktop palette brings its own surface, and a keyboard design keeps a veil of its key fill so the candidates stay legible over a photo or pattern.
  static func stripBackground(_ skin: KeyboardTheme, palette: CandidatePalette?) -> UIColor {
    if let palette { return palette.surface }
    return skin.design == nil ? .clear : skin.keyBackground.withAlphaComponent(0.6)
  }

  private func applyKeyboardSkin() {
    handwriting.canvas.setNeedsDisplay()
    replyModel.objectWillChange.send()
    let skin = KeyboardTheme.current
    candidatePalette = currentCandidatePalette()
    view.backgroundColor = skin.drawsNativeBackground ? .clear : skin.background
    skinBackdrop.skin = skin
    // Only here, not in KeyboardSkinBackgroundView: the App's skin previews have no system backdrop behind them.
    if skin.drawsNativeBackground { skinBackdrop.backgroundColor = .clear }
    func recolor(_ node: UIView) {
      if let button = node as? UIButton, var configuration = button.configuration {
        if configuration.background.customView is SkinKeySurfaceView || (configuration.background.backgroundColor?.cgColor.alpha ?? 0) > 0 {
          configuration.background.backgroundColor = (button as? KeyboardKeyButton)?.isFunctionKey == true
            ? skin.functionKeyBackground : skin.keyBackground
          configuration.baseForegroundColor = skin.keyForeground
        }
        if configuration.background.strokeWidth > 0 {
          configuration.background.strokeColor = skin.accent.withAlphaComponent(0.3)
        }
        button.configuration = configuration
        if let color = configuration.background.backgroundColor, color.cgColor.alpha > 0 { decorateKey(button) }
      }
      if node.accessibilityIdentifier == "nineKeySidebar" || node.accessibilityIdentifier == "candidateStrip" {
        node.backgroundColor = node.accessibilityIdentifier == "candidateStrip"
          ? Self.stripBackground(skin, palette: candidatePalette)
          : skin.keyBackground.withAlphaComponent(0.6)
      }
      if let label = node as? UILabel, label.accessibilityIdentifier == "keyNumberHint" { label.textColor = skin.accent }
      node.subviews.forEach { recolor($0) }
    }
    recolor(view)
    if var configuration = enterButton?.configuration {
      // A design draws return in its action colour; every other theme styles it from the composition state below.
      configuration.background.backgroundColor = skin.design == nil ? skin.functionKeyBackground : skin.actionBackground
      configuration.baseForegroundColor = skin.design == nil ? skin.keyForeground : skin.actionForeground
      enterButton?.configuration = configuration
      if let enterButton { decorateKey(enterButton) }
      styleReturnKey()
    }
    updateLanguageModeButton()
    updateSchemeButton()
    updateShortcutButtons()
    renderCandidateStrip()
    updateSpellingStrip()
    exitLocalModeButton.configuration?.baseForegroundColor = candidatePalette?.accent ?? skin.accent
    preeditButton.configuration?.baseForegroundColor = candidatePalette?.accent ?? skin.accent
    expandCandidatesButton.configuration?.baseForegroundColor = candidatePalette?.accent ?? skin.accent
    for (_, _, hint) in letterButtons { hint.textColor = skin.accent }
    view.tintColor = skin.accent
  }

  /// Draw the keyboard and its panels in the light or dark form the shared themes ask for (see KeyboardAppearancePreference). A changed style reaches `traitCollectionDidChange`, which redraws the skin.
  private func applyKeyboardAppearance() {
    let preferences = session.sharedPreferences
    // A built-in theme draws its surfaces in its own mode (dc.html L1551-1558); only `system` and a custom theme without a base follow the keyboard's mode setting.
    let keyboard = KeyboardTheme.current.appearance
      ?? KeyboardAppearancePreference.style(KeyboardAppearancePreference.keyboardKey, in: preferences)
    if overrideUserInterfaceStyle != keyboard { overrideUserInterfaceStyle = keyboard }
    handwriting.overrideUserInterfaceStyle = KeyboardAppearancePreference.style(KeyboardAppearancePreference.handwritingKey, in: preferences)
    emojiPicker?.overrideUserInterfaceStyle = KeyboardAppearancePreference.style(KeyboardAppearancePreference.emojiKey, in: preferences)
  }

  private func currentCandidatePalette() -> CandidatePalette? {
    CandidatePalette.active(in: session.sharedPreferences, systemDark: traitCollection.userInterfaceStyle == .dark)
  }

  /// Redraw the strip when the resolved candidate palette changed; switching it off goes back through the keyboard skin so the chips get the skin's shape again.
  private func refreshCandidatePalette() {
    let palette = currentCandidatePalette()
    guard palette != candidatePalette else { return }
    if palette == nil { applyKeyboardSkin(); return }
    candidatePalette = palette
    compositionContainer?.backgroundColor = palette?.surface
    for button in [exitLocalModeButton, preeditButton, expandCandidatesButton] {
      button.configuration?.baseForegroundColor = palette?.accent
    }
    renderCandidateStrip()
  }

  override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
    super.traitCollectionDidChange(previousTraitCollection)
    if isViewLoaded, actionRow != nil,
       previousTraitCollection?.userInterfaceStyle != traitCollection.userInterfaceStyle {
      applyKeyboardSkin()
    }
    // Docking or undocking the iPad keyboard changes the width class, and with it the form factor.
    if isViewLoaded, actionRow != nil,
       previousTraitCollection?.horizontalSizeClass != traitCollection.horizontalSizeClass {
      updateKeyboardLayoutIfNeeded()
      // The form factor also sets how large a synced candidate size may be drawn.
      applyCandidateGlossLayout()
    }
  }

  @objc private func prepareKeyFeedback() {
    if KeyboardFeedbackPreference.hapticsEnabled { keyFeedback.prepare() }
  }

  private func playInputClick() {
    if KeyboardFeedbackPreference.soundEnabled {
      UIDevice.current.playInputClick()
    }
    if KeyboardFeedbackPreference.hapticsEnabled {
      keyFeedback.impactOccurred(intensity: KeyboardFeedbackPreference.hapticStrength.intensity)
      keyFeedback.prepare()
    }
  }
}
