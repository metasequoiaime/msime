import SwiftUI
import CoreImage
import CoreFoundation
import UIKit

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
  /// 候选词块与展开箭头之间那条 1×22 的细线。
  private let expandDivider = UIView()
  private let candidateStack = UIStackView()
  private let candidateEmptySpacer = UIView()
  /// 输入方式：始终在工具栏里，在键区打开方案选择。
  private let schemeButton = KeyboardToolbarButton(icon: .toolbarScheme, accessibilityLabel: "选择输入方案")
  private let shortcutBar = UIStackView()
  private var candidateContent: UIStackView?
  private let scriptShortcut = UIButton()
  /// 高情商回复已移进功能菜单；按钮仍排在栏里但保持隐藏，按标识符查找它的地方不会找到别的控件。
  private let replyShortcut = UIButton()
  private let emojiShortcut = KeyboardToolbarButton(icon: .toolbarEmoji, accessibilityLabel: "表情")
  /// 常用语：与 Android 一样始终在工具栏里，因为 `touch_toolbar` 没有它的开关。
  private let phrasesShortcut = KeyboardToolbarButton(icon: .toolbarPhrase, accessibilityLabel: "常用语")
  private let skinShortcut = KeyboardToolbarButton(icon: .toolbarSkin, accessibilityLabel: "切换皮肤")
  /// 「工具栏按钮」（`touch_toolbar`）里的可选按钮，用户没有固定时隐藏；每一个在功能菜单里也有。
  private let clipboardShortcut = KeyboardToolbarButton(icon: .toolbarClipboard, accessibilityLabel: "剪贴板历史")
  private let aiShortcut = UIButton()
  private let characterSetShortcut = UIButton()
  private let fullwidthShortcut = UIButton()
  private let punctuationShortcut = UIButton()
  private var clipboardPanel: KeyboardClipboardView?
  private var skinPicker: KeyboardSkinPickerView?
  private var schemePicker: KeyboardSchemePickerView?
  private var phrasesPanel: KeyboardPhrasesView?
  /// 正在进行的常用语读取；其间打开或关闭面板会清掉它，回来时令牌已经变了的读取结果直接丢弃。
  private var phrasesRequest: UUID?
  /// 顶栏下方键区里的面板（`installPanel`）：功能菜单、表情、常用语、剪贴板、皮肤、方案或候选网格。面板打开期间各排按键隐藏。
  private var keyAreaPanel: UIView?
  private let moreShortcut = KeyboardBrandMarkButton()
  private var morePicker: KeyboardMorePickerView?
  /// 键盘高度：占据工具栏位置的内联调节条（此时按键照常可用），以及按「取消」时要恢复的调整值。
  private var inlineHeightBar: InlineHeightBar?
  private var inlineHeightSnapshot: CGFloat = 0
  /// 隐私模式，在键盘出现时和点它的磁贴时读取，按键时不读 App Group。
  private var incognito = false
  /// 当前输入框下各类记录是否允许（`KeyboardPrivacyGate`）：隐私模式或凭据输入框里统计、剪贴板历史、云剪贴板和诊断日志一律不记。
  private var privacyGate: KeyboardPrivacyGate {
    KeyboardPrivacyGate(incognito: incognito, credentialField: isCredentialField)
  }
  /// 「长按空格语音输入」（`KeyboardLayoutPreference.spaceVoice`）的开关，每次键盘出现时读一次。
  private var spaceVoiceEnabled = true
  /// 「滑动输入符号」（`KeyboardLayoutPreference.swipeSymbols`）的开关，每次键盘出现时读一次，再交给每个字母键。
  private var swipeSymbolsEnabled = true
  /// 上次配置诊断日志时闸门是否挡着它，`textDidChange` 据此判断要不要重新配置。
  private var diagnosticLogSuppressed = false
  /// 存储的单手模式，在键盘出现时读取，由它的磁贴和侧栏修改。
  private var oneHandedMode: KeyboardOneHandedMode = .off
  /// 本机的工具栏设置（`TouchToolbarLocalPreference`），与 Android 的 `applyLocalSettings` 一样在键盘出现时读取：常用语和输入方式按钮，以及显示方式「隐藏」。
  private var toolbarPhrases = true
  private var toolbarScheme = true
  private var toolbarHidden = false
  /// 显示方式为「隐藏」且顶栏上没有内容：顶栏不占空间，它下面的间距也一并去掉（`renderCandidateStrip`）。
  private var topRowCollapsed = false
  /// 按键行当前按哪种单手模式布局（`applyOneHanded`）；第一次布局之前为 nil。
  private var appliedOneHanded: KeyboardOneHandedMode?
  /// 顶栏下面的那一行：各排按键组成的列，单手模式下还有它旁边的侧栏。
  private let oneHandRow = UIStackView()
  /// 所有按键行，依次叠在顶栏下方。
  private let keyColumn = UIStackView()
  private let oneHandGutter = KeyboardOneHandGutterView()
  private var oneHandGutterWidth: NSLayoutConstraint?
  /// 长按空格键时在键区打开的语音面板。
  private var voicePanel: KeyboardVoicePanelView?
  private let handwriting = HandwritingInputView()
  /// 手写区：标点列、书写卡片和 ⌫ / 重写列，与 Android 的 rebuildHandwritingRows 布局一致。
  private let handwritingPad = UIStackView()
  /// 手写板的两列按键，间距与按键行相同。
  private var handwritingColumns: [UIStackView] = []
  private var handwritingResults: [String] = []
  private var handwritingActionHeight: NSLayoutConstraint?
  private var candidatePanel: KeyboardCandidatePanelView?
  /// 面板正在显示的那一代候选。选择和长按菜单都按它里面的代次和全局序号找候选；九键面板换代时整份换掉。
  private var candidatePanelSnapshot: CandidatePanelSnapshot?
  /// 全拼九键展开面板的左右两栏；不为 nil 时面板是三栏布局，换代时不关而是重建。
  private var nineKeyPanelColumns: KeyboardNineKeyPanelColumns?
  /// 九键面板左栏正显示笔画键。只是面板自己的状态，收起面板就回到拼音。
  private var nineKeyPanelStrokeMode = false
  private var emojiPicker: KeyboardEmojiPickerView?
  private var symbolPanel: KeyboardSymbolPanelView?
  private var nineKeyHoldPopup: UIView?
  private struct NineKeyGridKey {
    let button: UIButton
    let digit: Int
    /// 在 3×3 网格里的位置，从 0 起。数字层按 `numberKeypadOrder` 由位置算出显示和输入的数字，字母层始终是 `digit`。
    let row: Int
    let column: Int
    let letters: String?
    let numberHint: UILabel?
    /// 2–9 键按住弹出数字与字母的菜单；26 键的九宫格数字层上停用，那里没有字母可选。
    let hold: UILongPressGestureRecognizer?
  }
  private var nineKeyGridKeys: [NineKeyGridKey] = []
  /// 九键外框的 ，。？！ 四个标点键和它们的中文标点，键面与输入见 `nineKeyMark`。
  private var nineKeyMarkKeys: [(button: UIButton, mark: String)] = []
  /// 九键数字层的排列，键盘出现时从共享文档同步（`synchronizeSharedTouchPreferences`），按键时不再读 App Group。
  private var numberKeypadOrder = KeyboardLayoutPreference.numberKeypadOrder
  /// 手机 26 键按 123 时出一行数字符号页还是九键数字层，与 `numberKeypadOrder` 一起从共享文档同步。
  private var twentySixKeyNumberLayout = KeyboardLayoutPreference.twentySixKeyNumberLayout
  /// 上一次 `updateKeyboardLayout` 是否把九键数字层当作 26 键的 123 页画了出来（`opensNineKeyDigitPad`）。
  private var nineKeyDigitPadShown = false
  /// 26 键双拼画不画键位提示（共享偏好 `touch_shuangpin_key_hints`），键盘出现时从共享文档同步，绘制键面时不再读 App Group。
  private var shuangpinKeyHintsEnabled = KeyboardLayoutPreference.shuangpinKeyHints
  private enum MoreToolsPage { case root, localInput }
  private var moreToolsPage: MoreToolsPage = .root
  private let dismissShortcut = KeyboardToolbarButton(icon: .collapse, accessibilityLabel: "收起键盘")
  /// 每个字母键及其两个提示：`hint` 是沿底边显示的双拼韵母，`corner` 是下滑时输入的符号（`LetterHintTable`），画在右上角。
  private var letterButtons: [(button: UIButton, lowercase: String, hint: UILabel, corner: UILabel)] = []
  /// 滑行输入（`KeyboardLayoutPreference.glideTyping`）的开关，每次键盘出现时读一次，按键时不再读 App Group。
  private var glideTypingEnabled = false
  private let glideGesture = GlideTypingGestureRecognizer()
  private let glideTrail = GlideTrailView()
  /// a..z 二十六个字母键，按字母顺序；滑行请求里的键中心按这个顺序给出。
  private var glideLetterKeys: [UIButton] = []
  private var microsoftFinalKey: UIButton?
  /// Comma and full stop at the end of the third letter row; shown only on the tablet keyboard.
  private var letterRowPunctuationKeys: [UIButton] = []
  private var formFactor: KeyboardFormFactor { .resolve(traitCollection) }
  private var letterRowViews: [UIView] = []
  /// The iPad digit row above the letters and the Tab key before Q; see `KeyboardLayoutPreference.tabletFullKeys`.
  private var numberRowView: UIStackView?
  private var tabKey: UIButton?
  /// 横屏分离式键盘每一排中间的中缝（见 `KeyboardSplitLayout`）。不分离时隐藏，stack view 不给它宽度也不给它两侧的键距；分离时它是一块普通视图，落在上面的触摸由它接住后丢掉，不会被 `KeyAreaStackView` 当成键距交给旁边的键。
  private var splitGaps: [UIView] = []
  private var splitGapWidths: [NSLayoutConstraint] = []
  /// 分离时右半最里面的第二个空格键，和 `spaceButton` 做同样的事。
  private weak var splitSpaceButton: UIButton?
  /// 上一次 `updateKeyboardLayout` 画的是不是分离式键盘；旋转或改设置后与 `wantsSplitKeyboard` 不同就重新布局。
  private var splitKeyboardShown = false
  /// 共用的每排十键符号行，现在只有大千布局还拿它做符号层（`drawsSymbolLayer`）。
  private var symbolRowViews: [UIView] = []
  /// 设计稿 123 / #+= 层的三排按键（`SymbolLayerLayout`）；底行是为该层排好的功能行。
  private var symbolLayerRowViews: [UIStackView] = []
  /// 这几排里的字符键，依次为十、十、五个，键面跟随屏幕上显示的层。
  private var symbolLayerKeys: [[UIButton]] = []
  /// 123 层上是 `#+=`，#+= 层上是 `123`。
  private var symbolLayerToggle: UIButton?
  /// 123 层的表情键，在底行里紧挨回到字母的那个键。
  private var layerEmojiButton: UIButton!
  /// 九键外框底行的 0，位于空格键和回车之间。
  private var nineKeyZeroButton: UIButton!
  /// 九键外框右列的中间键：拼音网格上是分词，其数字层上是句点，笔画键旁边是重输。
  private var nineKeyMiddleButton: UIButton?
  /// The four Dachen rows, shown instead of the letter rows while the Zhuyin scheme is active.
  private var zhuyinRowViews: [UIView] = []
  // Symbol keys show the punctuation they actually emit in Chinese mode.
  private var symbolKeyFaces: [(key: UIButton, ascii: String, chinese: String)] = []
  /// 符号页第三排的 `=` 键：藏文方案下换成威利叠写用的 `+`（见 `symbolRowKey`）。
  private weak var tibetanPlusKey: UIButton?
  private var layoutToggleButton: UIButton?
  private weak var shiftButton: UIButton?
  private weak var enterButton: UIButton?
  /// iPad 键盘专有的键（`TabletLetterLayout`）：第一排字母末尾的 ⌫、第二排末尾的回车，以及 ，。 之后的第二个 ⇧，同时隐藏手机第三排的 ⌫。在手机上这些键都留在各自的行里，处于隐藏状态。
  private weak var tabletDeleteKey: UIButton?
  private weak var tabletReturnKey: UIButton?
  private weak var rightShiftButton: UIButton?
  private weak var letterDeleteKey: UIButton?
  /// 左侧 ⇧ 的宽度：手机上 44pt，iPad 上按设计稿为 1.4 个键宽。
  private var phoneShiftWidth: NSLayoutConstraint?
  private var tabletShiftWidth: NSLayoutConstraint?
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
  private var inputScheme: ChineseInputScheme = .editionFallback
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
  /// Whether the stroke keys are on screen and feed the Stroke editor: their faces are the stroke glyphs 一丨丿丶乛＊, and each sends the letter the Engine reads for it.
  private var typesStroke: Bool { isChineseMode && inputScheme.isStroke && !isInLocalMode }
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
  /// 笔画方案在九键外框里替换 3×3 网格的 2×3 笔画键。
  private var strokeKeys: StrokeKeypadView!
  private weak var japaneseGlobeButton: UIButton?
  private weak var japaneseSpaceButton: UIButton?
  private weak var japaneseReturnButton: UIButton?
  private var japaneseHeight: NSLayoutConstraint!
  private var nineKeyHeight: NSLayoutConstraint!
  private var nineKeySymbolsButton: UIButton!
  private let punctuationStack = UIStackView()
  private var quickPunctuationButton: UIButton!
  private var quickPunctuationWidth: NSLayoutConstraint?
  /// 手机 26 键底行空格键右边的 。；， 是 `quickPunctuationButton`。
  private var periodKey: UIButton!
  /// 分离式键盘底行的中缝，留着引用是为了重排这一行时它仍紧挨空格键。
  private var actionSplitGap: UIView?
  private var symbolDeleteWidth: NSLayoutConstraint?
  private var standardActionWidths: [NSLayoutConstraint] = []
  /// 九键外框底行按设计稿的弹性权重，以空格键的 4.2 为基准：123 1.25、地球键 1、0 1.05、中 1.05、回车 1.6。
  private var nineKeyActionWidths: [NSLayoutConstraint] = []
  /// 123 / #+= 层的底行，以空格键的 6 为基准（`SymbolLayerLayout`）。
  private var layerActionWidths: [NSLayoutConstraint] = []
  /// 设计稿的手机底行弹性权重，以空格键的 4 为基准：123 1.25、中 1.05、地球键 1、， 1、。 1、回车 1.9。
  private var phoneActionWidths: [NSLayoutConstraint] = []
  /// iPad 26 键底行，以空格键的 6.4 为基准（`TabletLetterLayout`）：123 1.5、中 1.2、地球键 1、123 1.5、⌄ 1.2。
  private var tabletActionWidths: [NSLayoutConstraint] = []
  /// iPad 底行的第二个 123 和收起键盘的 ⌄；其他底行上都隐藏。
  private var tabletLayerButton: UIButton!
  private var dismissKeyButton: UIButton!
  /// 底行是否按手机 26 键底行布局（`usesPhoneBottomRow`），是的话 ， 跟随标点键面，而不是快捷标点菜单的第一个标点。
  private var phoneBottomRowShown = false
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
  private var currentNineKeySingleCharacter = false
  private var currentNineKeyStrokes = ""
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
  private var showsSymbols = false {
    didSet { if !showsSymbols { showsMoreSymbols = false } }
  }
  /// 符号层当前是否在 #+= 页而不是 123 页。关闭符号层时清掉它，所以符号层总是从 123 页打开，与 Android 的 `moreSymbols` 一致。
  private var showsMoreSymbols = false
  private var letterCaseState = LetterCaseState.lowercase
  private var isAutomaticShift = false
  private var lastShiftTapTime: TimeInterval?
  /// UIKit 也为键盘自己的改动回调 `textWillChange`，而它会结束组字：没认出回声，一次留着剩余组字的上屏会在一轮之后把组字毁掉，下一键刚开始的组字也会被前一键迟到的回声结束。怎么认见 `OwnEditEchoWindow`。
  private var ownEditEcho = OwnEditEchoWindow()
  /// What 行内预编辑 last wrote into the host as marked text; empty when nothing is marked.
  private var inlineMarkedText = ""

  /// The chips the strip numbers, the page size the session was given (see CandidatePageSizePreference), so a digit picks the chip carrying its number. Everything past it is in the expanded panel.
  private var candidatePageSize: Int {
    CandidatePageSizePreference.clamped(session.sharedPreferences?["candidate_page_size"] as? Int)
  }
  // 顶栏（dc.html：一行 50pt）空闲时是工具栏；组字时由读音行和候选行占据同一位置，与 Android 一致（`ImeToolbar`）。不管显示什么，它的高度都取组字布局的高度，所以组字开始或结束时按键不会移动。
  // 不是 private：高度断言从这些值推出，而不是把总和再写一遍。
  /// 设计稿的顶栏高度，顶栏不会低于它。
  static let topRowMinimumHeight: CGFloat = 50
  /// 候选上方的读音行：按设计稿以 12pt 单独一行显示拼写。
  static let readingRowHeight: CGFloat = 14
  /// 读音行上的拼写（dc.html：12px、kb.sub、letter-spacing .02em）。
  static let preeditFontSize: CGFloat = 12
  private static let candidateRowHeight: CGFloat = 38
  /// 带释义的候选行：第一行释义和候选字同在这一行里，18pt 候选（21）加 10pt 释义（13）再加上下内边距各 4 正好 42，与 Android 的 `ImeToolbar.CANDIDATE_LINE_DP` 一致。原先释义另加一整行 14pt，开着释义时顶栏是 66pt，比 Android 高 10pt，空闲时工具栏也一直按这个高度留着。
  static let glossedCandidateRowHeight: CGFloat = 42
  /// 第二行释义起每行再加的高度（Android 的 `EXTRA_GLOSS_ROW_DP`）。
  static let glossLineHeight: CGFloat = 14
  static func glossHeight(lines: Int) -> CGFloat { CGFloat(max(lines, 0)) * glossLineHeight }
  // 候选或编码字号调大时所在行跟着变高，调小时行高不会低于默认值：这一行同时也是触摸目标。
  static func readingRowHeight(preeditScale: CGFloat) -> CGFloat {
    max(readingRowHeight, ceil(readingRowHeight * preeditScale))
  }
  static func candidateRowHeight(candidateScale: CGFloat, glossed: Bool = false) -> CGFloat {
    let base = glossed ? glossedCandidateRowHeight : candidateRowHeight
    return max(base, ceil(base * candidateScale))
  }
  /// 顶栏：读音行在上，候选行在下，第一行释义在候选行里，其余释义行在它下面，高度不低于设计稿的 50pt。
  static func topRowHeight(
    glossLines: Int, candidateScale: CGFloat = 1, preeditScale: CGFloat = 1
  ) -> CGFloat {
    max(topRowMinimumHeight, readingRowHeight(preeditScale: preeditScale)
      + candidateRowHeight(candidateScale: candidateScale, glossed: glossLines > 0)
      + glossHeight(lines: glossLines - 1))
  }
  /// 用户调整之前的键盘高度：按默认候选字号和存储的行距计算，每个候选下带 `glossLines` 行释义（默认取配置的行数）。
  /// Tests and host layout consumers use this contract so the default gloss row stays accounted for.
  static func keyboardHeight(
    _ formFactor: KeyboardFormFactor = .phone, landscape: Bool = false, handwriting: Bool = false,
    numberRow: Bool = false, glossLines: Int? = nil
  ) -> CGFloat {
    let lines = glossLines ?? configuredGlossLines(fullAccess: false, onlineRoute: false)
    return formFactor.keyboardHeight(
      topRow: topRowHeight(glossLines: lines), rowSpacing: CGFloat(KeyboardLayoutPreference.rowSpacing),
      landscape: landscape, handwriting: handwriting, numberRow: numberRow)
  }
  /// 手机竖屏键盘：`keyboardHeight()`。
  static var defaultKeyboardHeight: CGFloat { keyboardHeight() }

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
  private var topRowHeightConstraint: NSLayoutConstraint?
  private var readingRowHeightConstraint: NSLayoutConstraint?
  /// 顶栏的读音行；只在候选行显示时才显示。
  private let readingRow = UIView()
  /// 根视图的内边距（`KeyboardFormFactor.padding`）：上、前、后、下。
  private var keyboardPaddingConstraints: [NSLayoutConstraint] = []

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
    // 方案以共享文档为准。会话在本控制器创建时已经同步读过一次文档，先把其中记的方案抄进 App Group 镜像再按镜像行事，不额外读盘；否则在 `viewWillAppear` 的后台重载回来之前，键盘会先按镜像里的旧方案（比如双拼）画出来并开始接收按键。
    InputSchemePreference.mirror(session.sharedPreferences)
    inputScheme = InputSchemePreference.scheme
    isChineseMode = ImeModeMemoryPreference.startsInChinese(fallback: Self.startsInChinese(session.sharedPreferences))
    appliedCharacterWidth = CharacterWidthPreference.value(in: session.sharedPreferences)
    setFullWidthInput(CharacterWidthPreference.startsFullwidth(in: session.sharedPreferences))
    appliedChinesePunctuation = Self.sharedChinesePunctuation(session.sharedPreferences)
    chinesePunctuation = appliedChinesePunctuation
    synchronizeAICredential()
    applyKeyboardAppearance()
    // 隐私模式要在诊断日志配置之前读出来：开着时日志一行也不写，包括下面这行。
    incognito = KeyboardPrivacyPreference.incognito
    readGesturePreferences()
    configureDiagnosticLog()
    DiagnosticLog.shared.write("keyboard_loaded full_access=\(hasFullAccess ? 1 : 0) idiom=\(UIDevice.current.userInterfaceIdiom == .pad ? "pad" : "phone")")
    if session.initializationFailed { DiagnosticLog.shared.write("runtime_initialization_failed") }
    glossLineCount = currentGlossLines()
    // 简繁与方案同理以共享文档为准：先把会话创建时读到的文档里记的字形抄进镜像，否则在后台重载回来之前（文档没有更新时它根本不会抄），键盘按镜像里的旧字形转换上屏文字。
    ChineseOutputPreference.mirror(session.sharedPreferences)
    usesTraditionalOutput = ChineseOutputPreference.usesTraditional
    oneHandedMode = KeyboardLayoutPreference.oneHanded
    readToolbarPreferences()
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
      preferredKeyboardHeight + CGFloat(KeyboardLayoutPreference.heightAdjustment))
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
    // 进程可能跨过换季或应用主题的改动；在下面任何地方解析皮肤之前先刷新。
    KeyboardTheme.refreshSeason()
    incognito = KeyboardPrivacyPreference.incognito
    oneHandedMode = KeyboardLayoutPreference.oneHanded
    readToolbarPreferences()
    readGesturePreferences()
    applyOneHanded()
    // 隐私模式和输入框都可能在两次出现之间变了，日志开不开要重新判断。
    configureDiagnosticLog()
    DiagnosticLog.shared.write("focus_in")
    KeyboardUsageReporting.presented(fullAccess: hasFullAccess)
    do { try session.resumeDictionarySession() }
    catch {
      DiagnosticLog.shared.write("dictionary_resume_failed")
      showDiagnostic(error.localizedDescription)
    }
    // 新的编辑会话不欠任何回调，上一次出现时留下的回声窗口不能延续到这里。
    ownEditEcho.reset()
    inlineMarkedText = ""
    if schemePicker != nil { closeKeyboardPicker() }
    synchronizeInputContext()
    synchronizeInputSchemePreference()
    synchronizeChineseOutputPreference()
    applyLearningPreferences()
    synchronizeTranslationRoute()
    glideTypingEnabled = KeyboardLayoutPreference.glideTyping
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
      let translationSettingsChanged = self.synchronizeSharedTouchPreferences()
      self.synchronizeCharacterWidth()
      self.synchronizeChinesePunctuation()
      self.synchronizeAICredential()
      self.synchronizeChineseOutputPreference()
      self.applyLearningPreferences()
      let translationRouteChanged = self.synchronizeTranslationRoute()
      if translationSettingsChanged || translationRouteChanged {
        self.translations.cancel()
        self.requestCandidateTranslations()
      }
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
    // Keyboard extensions are terminated shortly after a warning if they keep their optional
    // panels, translation results, or Engine candidate caches. Drop everything that can be
    // recreated while keeping the active composition and session alive.
    closeKeyboardPicker()
    closeKeyboardService()
    handwriting.deactivate()
    snapshotWorker.stop()
    candidateGlossEpoch &+= 1
    candidateGlossRequestedGeneration = nil
    translations.clearCacheForMemoryPressure()
    onlineCandidates.cancel()
    session.resetCache()
    ResolvedTheme.clearCacheForMemoryPressure()
  }

  /// `diagnostic_log.server` 开着时让诊断日志写到共享目录，关掉后停写；隐私模式和凭据输入框里同样停写（`KeyboardPrivacyGate` 的 `diagnosticLog`）。
  private func configureDiagnosticLog() {
    let preferences = session.sharedPreferences ?? MetasequoiaInputSessionBridge.loadSharedPreferences()
    diagnosticLogSuppressed = !privacyGate.allows(.diagnosticLog)
    DiagnosticLog.shared.configure(directory: session.stateDirectory ?? MetasequoiaInputSessionBridge.sharedStateDirectory,
                                   enabled: DiagnosticLog.isEnabled(in: preferences) && !diagnosticLogSuppressed)
  }

  /// 读本机的两项手势开关：长按空格语音和滑动输入符号。后者交给每个字母键，按键时不再读 App Group。
  private func readGesturePreferences() {
    spaceVoiceEnabled = KeyboardLayoutPreference.spaceVoice
    swipeSymbolsEnabled = KeyboardLayoutPreference.swipeSymbols
    for letter in letterButtons { (letter.button as? KeyboardKeyButton)?.swipesCornerHint = swipeSymbolsEnabled }
    for space in spaceKeys {
      space.accessibilityHint = Self.spaceAccessibilityHint(voice: spaceVoiceEnabled)
      space.accessibilityCustomActions = spaceAccessibilityActions()
    }
  }

  /// 空格键的 VoiceOver 提示：关掉长按空格语音后不再说长按能打开语音。
  private static func spaceAccessibilityHint(voice: Bool) -> String {
    voice ? "轻点输入空格或选词，左右滑动移动光标，长按打开语音输入" : "轻点输入空格或选词，左右滑动移动光标"
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
    // 自己改动的回声：那次改动之后的组字仍是当前的组字。
    if ownEditEcho.isEcho(at: ProcessInfo.processInfo.systemUptime) { return }
    // With 行内预编辑 the host already holds the letters: moving the caret out of marked text makes them ordinary text, and clearing the field removes them. Committing the composition as well would write it a second time, so the engine lets it go instead.
    if !inlineMarkedText.isEmpty {
      inlineMarkedText = ""
      textDocumentProxy.unmarkText()
      noteOwnEdit()
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
    // 宿主可能把焦点换到了凭据输入框，诊断日志跟着当前输入框开关；只在闸门的判断变了时重新配置，不必每次按键都去读偏好。
    if privacyGate.allows(.diagnosticLog) == diagnosticLogSuppressed { configureDiagnosticLog() }
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
    // 键盘收起时还在调整高度就当作取消：没点「完成」的预览不保存，与 Android onFinishInputView 一致。
    closeInlineHeight(commit: false)
    cursorMovement.cancel()
    endGlide()
    updateSpaceKeyTitle()
    // Putting the keyboard away used to drop whatever was composed. macOS commits in
    // prepareForDeactivation: for the same reason: the user typed those letters and never asked to
    // throw them away.
    render(session.finishComposition())
    _ = session.suspendDictionarySession()
    ownEditEcho.reset()
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
    let root = KeyAreaStackView()
    keyboardRoot = root
    root.axis = .vertical
    root.spacing = 7
    root.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(root)

    // 这些常量是设备形态的内边距，由 `applyKeyboardMetrics` 设置。
    keyboardPaddingConstraints = [
      root.topAnchor.constraint(equalTo: view.topAnchor),
      root.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      root.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      root.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ]
    NSLayoutConstraint.activate(keyboardPaddingConstraints)

    let candidateStrip = makeCandidateStrip()
    root.addArrangedSubview(candidateStrip)
    // 候选栏和手写区不是键：落在它们里面的触摸照旧，它们的按钮也不来接键距里的触摸。
    root.gapRoutingExclusions = [candidateStrip, handwriting]
    installOneHandRow(in: root)
    let numberRow = makeNumberRow()
    numberRowView = numberRow
    keyColumn.addArrangedSubview(numberRow)
    for (index, row) in letterRows.enumerated() {
      let rowView = makeLetterRow(row, includesShift: index == letterRows.count - 1)
      letterRowViews.append(rowView)
      keyColumn.addArrangedSubview(rowView)
    }
    for (index, row) in ZhuyinKeyLayout.rows.enumerated() {
      let rowView = makeZhuyinRow(row, includesDelete: index == ZhuyinKeyLayout.rows.count - 1)
      rowView.isHidden = true
      zhuyinRowViews.append(rowView)
      keyColumn.addArrangedSubview(rowView)
    }
    keyColumn.addArrangedSubview(makeNineKeyLayout())
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
    keyColumn.addArrangedSubview(japaneseKeys)
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
    keyColumn.addArrangedSubview(makeHandwritingPad())
    for row in symbolRows {
      let rowView = makeSymbolRow(row)
      rowView.isHidden = true
      symbolRowViews.append(rowView)
      keyColumn.addArrangedSubview(rowView)
    }
    for index in 0..<3 {
      let rowView = makeSymbolLayerRow(index)
      rowView.isHidden = true
      symbolLayerRowViews.append(rowView)
      keyColumn.addArrangedSubview(rowView)
    }
    actionRow = makeActionRow()
    keyColumn.addArrangedSubview(actionRow)
    // 中/英紧挨回车的底行（手机、九键），回车把键帽左边几 pt 让给中/英；别的排法里两者不相邻，不起作用。
    root.yieldingKey = enterButton
    root.yieldReceiver = bottomLanguageButton
    installSplitGaps(in: root)
    standardRowHeights = ([numberRow] + letterRowViews + zhuyinRowViews + symbolRowViews + symbolLayerRowViews).map {
      ($0, $0.heightAnchor.constraint(equalTo: actionRow.heightAnchor))
    }
    // Keep the three keypad rows the same height as the bottom controls.
    nineKeyHeight = nineKeyContainer.heightAnchor.constraint(
      equalTo: actionRow.heightAnchor, multiplier: 3, constant: 14)
    // The kana surface now has a dedicated punctuation row beneath the three kana rows.
    japaneseHeight = japaneseKeys.heightAnchor.constraint(equalToConstant: Self.japaneseKeyBlockHeight)
    // Extra handwriting space belongs to the canvas, not enlarged Space/Return keys.
    handwritingActionHeight = actionRow.heightAnchor.constraint(equalToConstant: 44)
    installGlideTyping(on: root)
    updateKeyboardLayout()
  }

  /// 手写区，对应 Android 的 rebuildHandwritingRows：左边竖排 ，。？！，中间是书写卡片，右边竖排 ⌫ 和重写，三者按 Android 的 .7 / 3 / .8 分配一行的宽度。卡片上有笔迹时 ⌫ 撤回最后一笔，卡片为空时删除光标前的字符（`handleBackspace`）；重写清空卡片。下面的底行是所有 26 键界面共用的手机底行。
  private func makeHandwritingPad() -> UIStackView {
    handwritingPad.axis = .horizontal
    handwritingPad.alignment = .fill
    handwritingPad.distribution = .fill
    handwritingPad.spacing = 6
    handwritingPad.accessibilityIdentifier = "handwritingPad"
    handwritingPad.isHidden = true
    let punctuation = UIStackView()
    punctuation.accessibilityIdentifier = "handwritingPunctuation"
    for symbol in Self.handwritingPunctuation {
      let key = makeKey(title: symbol, accessibilityLabel: "符号 \(symbol)", function: true) { [weak self] in
        self?.countKeyPress(TypingKeyID.punctuation)
        self?.handleSymbol(symbol)
      }
      key.configuration?.contentInsets = .zero
      key.accessibilityIdentifier = "handwritingPunctuation\(symbol)"
      punctuation.addArrangedSubview(key)
    }
    let tools = UIStackView()
    tools.accessibilityIdentifier = "handwritingTools"
    let delete = makeDeleteKey()
    delete.accessibilityIdentifier = "handwritingDelete"
    tools.addArrangedSubview(delete)
    let rewrite = makeKey(title: "重写", accessibilityLabel: "清空手写", function: true) { [weak self] in
      self?.handwriting.clear()
    }
    rewrite.configuration?.contentInsets = .zero
    rewrite.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    rewrite.accessibilityIdentifier = "handwritingRewrite"
    tools.addArrangedSubview(rewrite)
    for column in [punctuation, tools] {
      column.axis = .vertical
      column.alignment = .fill
      column.distribution = .fillEqually
      column.spacing = CGFloat(KeyboardLayoutPreference.rowSpacing)
    }
    handwritingColumns = [punctuation, tools]
    handwritingPad.addArrangedSubview(punctuation)
    handwritingPad.addArrangedSubview(handwriting)
    handwritingPad.addArrangedSubview(tools)
    NSLayoutConstraint.activate([
      punctuation.widthAnchor.constraint(equalTo: handwriting.widthAnchor, multiplier: Self.handwritingPunctuationShare / Self.handwritingCardShare),
      tools.widthAnchor.constraint(equalTo: handwriting.widthAnchor, multiplier: Self.handwritingToolsShare / Self.handwritingCardShare),
    ])
    return handwritingPad
  }

  /// 手写板的标点列，对应 Android 的 `NineKeyLayout.punctuation()`。
  static let handwritingPunctuation = ["，", "。", "？", "！"]
  /// 手写板在一行里的宽度份额，取 Android 的布局权重：标点 .7、书写卡片 3、工具 .8。
  static let handwritingPunctuationShare: CGFloat = 0.7
  static let handwritingCardShare: CGFloat = 3
  static let handwritingToolsShare: CGFloat = 0.8

  /// 顶栏下面的那一行：填满它的按键列，以及单手模式的侧栏；侧栏默认隐藏，由 `applyOneHanded` 显示在按键的另一侧。这一行总是从左到右排列，因为存储的模式记的就是按键靠哪一侧。
  private func installOneHandRow(in root: UIStackView) {
    keyColumn.axis = .vertical
    keyColumn.spacing = root.spacing
    oneHandRow.axis = .horizontal
    oneHandRow.alignment = .fill
    oneHandRow.distribution = .fill
    oneHandRow.spacing = KeyboardOneHandLayout.gap
    oneHandRow.semanticContentAttribute = .forceLeftToRight
    oneHandRow.addArrangedSubview(keyColumn)
    oneHandGutter.isHidden = true
    oneHandRow.addArrangedSubview(oneHandGutter)
    // 设为 required，且只在侧栏显示时生效：底行隐藏的键会打破它们可选的宽度权重，侧栏宽度如果也只是可选的，布局引擎会拿它和那些约束权衡，把按键压窄。
    oneHandGutterWidth = oneHandGutter.widthAnchor.constraint(equalTo: oneHandRow.widthAnchor, multiplier: KeyboardOneHandLayout.gutterRatio)
    oneHandGutter.onSwap = { [weak self] in
      guard let self else { return }
      setOneHanded(oneHandedMode.toggled(swapSide: true))
    }
    oneHandGutter.onExit = { [weak self] in
      guard let self, oneHandedMode != .off else { return }
      setOneHanded(oneHandedMode.toggled(swapSide: false))
    }
    root.addArrangedSubview(oneHandRow)
  }

  /// 按单手模式布局按键行：按键收窄到侧栏留下的宽度，靠在存储的那一侧，侧栏在另一侧。只有手机键盘会画它（`KeyboardOneHandLayout.effective`）；存储的值无论如何都保留。工具栏、候选和盖在按键上的面板保持全宽。
  private func applyOneHanded() {
    // 方案在按键建好之前就已应用，那一步已经请求过布局。
    guard keyColumn.superview === oneHandRow else { return }
    let mode = KeyboardOneHandLayout.effective(oneHandedMode, formFactor: formFactor)
    guard mode != appliedOneHanded else { return }
    appliedOneHanded = mode
    let on = mode != .off
    oneHandGutter.setKeysOnRight(mode == .right)
    let index = KeyboardOneHandLayout.gutterLeads(mode) ? 0 : 1
    if oneHandRow.arrangedSubviews.firstIndex(of: oneHandGutter) != index {
      oneHandRow.removeArrangedSubview(oneHandGutter)
      oneHandRow.insertArrangedSubview(oneHandGutter, at: index)
    }
    // 宽度约束在侧栏隐藏之前停用、在显示之后启用，这样它不会碰上 stack view 给隐藏视图的零宽约束。
    if on {
      oneHandGutter.isHidden = false
      oneHandGutterWidth?.isActive = true
    } else {
      oneHandGutterWidth?.isActive = false
      oneHandGutter.isHidden = true
    }
    updateLetterRowInsets()
  }

  /// 存储并画出新的单手模式，与 Android 的 `toggleOneHanded` 从磁贴和侧栏触发时一样。功能菜单开着时重画它的磁贴。
  private func setOneHanded(_ mode: KeyboardOneHandedMode) {
    oneHandedMode = mode
    KeyboardLayoutPreference.oneHanded = mode
    applyOneHanded()
    updateShortcutButtons()
  }

  /// 滑行输入的手势和轨迹都挂在键区上：手势看得到键区里的每一根手指，轨迹盖在键的上面。
  private func installGlideTyping(on root: KeyAreaStackView) {
    glideLetterKeys = GlideTyping.letters.compactMap { letter in
      letterButtons.first { $0.lowercase == String(letter) }?.button
    }
    glideTrail.frame = root.bounds
    glideTrail.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    root.addSubview(glideTrail)
    glideGesture.isArmed = { [weak self] in self?.glideTypingArmed ?? false }
    glideGesture.letterIndex = { [weak self, weak root] view in
      guard let self, let root else { return nil }
      var node: UIView? = view
      while let current = node, current !== root {
        if let index = glideLetterKeys.firstIndex(where: { $0 === current }) { return index }
        node = current.superview
      }
      return nil
    }
    glideGesture.keyFrames = { [weak self, weak root] in
      guard let self, let root else { return [] }
      return glideLetterKeys.map { KeyAreaStackView.layoutFrame(of: $0, in: root) }
    }
    glideGesture.onBegan = { [weak self, weak root] in
      guard let self, let root else { return }
      root.suppressesKeyHits = true
      root.bringSubviewToFront(glideTrail)
    }
    glideGesture.onMoved = { [weak self] samples in
      guard let self else { return }
      glideTrail.draw(samples, color: KeyboardTheme.current.accent)
    }
    glideGesture.onEnded = { [weak self] samples, frames in
      guard let self else { return }
      endGlide(fading: true)
      typeGlide(samples, frames: frames)
    }
    glideGesture.onCancelled = { [weak self] in self?.endGlide() }
    root.addGestureRecognizer(glideGesture)
  }

  /// 此刻按下的字母键能否开始滑行：开关打开；显示的是全拼 26 键的字母层，不是符号层、九键、手写、笔画、注音或韩文键帽；中文模式而不是英文；没有本地输入模式；Shift 也没有把下一个字母变成辅助码。
  private var glideTypingArmed: Bool {
    glideTypingEnabled && isChineseMode && inputScheme == .quanpin && !isInLocalMode && !showsSymbols
      && !replyKeyboardShown && !entersHelpcode && glideLetterKeys.count == GlideTyping.letters.count
      && letterRowViews.allSatisfy { !$0.isHidden }
  }

  /// 滑行结束：键区重新接受新的手指，轨迹抬手时淡出，被打断时直接清空。
  private func endGlide(fading: Bool = false) {
    (keyboardRoot as? KeyAreaStackView)?.suppressesKeyHits = false
    if fading { glideTrail.fadeOut() } else { glideTrail.clear() }
  }

  /// 抬手：把这一笔交给 Engine，回应像轻点字母键那样渲染。Engine 不处理（方案或模式不对、解不出音节）时丢掉这一笔，一个键也不输入。
  private func typeGlide(_ samples: [GlideTyping.Sample], frames: [CGRect]) {
    synchronizeInputSchemePreference()
    guard glideTypingArmed, let request = GlideTyping.request(frames: frames, samples: samples) else { return }
    let snapshot = session.glide(request)
    guard snapshot.isHandled else { return }
    render(snapshot)
  }

  /// 给数字行、三排字母、三排符号和底部功能键那一排各插入一段中缝，默认隐藏，分离时由 `updateKeyboardLayout` 显示。
  ///
  /// 中缝要有自己的宽度，所以原来 `.fillEqually` 的排改成 `.fill` 再给字符键加上等宽约束，不分离时排出来和原来一样。字符键的中缝位置按 `KeyboardSplitLayout.leftKeyCount` 定，是固定的，不随 `;` 等键的显隐移动；底部那一排的中缝在两个空格键之间。
  private func installSplitGaps(in root: UIStackView) {
    let rows = ([numberRowView as UIView?].compactMap { $0 } + letterRowViews + symbolRowViews + symbolLayerRowViews).compactMap { $0 as? UIStackView }
    for row in rows {
      let characterKeys = row.arrangedSubviews.filter { ($0 as? KeyboardKeyButton)?.isFunctionKey != true }
      guard !characterKeys.isEmpty else { continue }
      if row.distribution == .fillEqually {
        row.distribution = .fill
        for key in characterKeys.dropFirst() {
          let width = key.widthAnchor.constraint(equalTo: characterKeys[0].widthAnchor)
          // 微软双拼的 `;` 只在该方案下显示：等宽约束低于 required，让 stack view 隐藏它时的零宽约束胜出。
          if key === microsoftFinalKey { width.priority = .init(999) }
          width.isActive = true
        }
      }
      insertSplitGap(into: row, after: characterKeys[KeyboardSplitLayout.leftKeyCount(characterKeys.count) - 1], root: root)
    }
    if let space = spaceButton {
      insertSplitGap(into: actionRow, after: space, root: root)
      actionSplitGap = splitGaps.last
    }
  }

  private func insertSplitGap(into row: UIStackView, after key: UIView, root: UIStackView) {
    guard let index = row.arrangedSubviews.firstIndex(of: key) else { return }
    let gap = UIView()
    gap.accessibilityIdentifier = "splitKeyboardGap"
    gap.isAccessibilityElement = false
    gap.isHidden = true
    row.insertArrangedSubview(gap, at: index + 1)
    // 中缝两侧各还有一个键距，宽度减去它们，两半最里面的键之间正好空出 `gapRatio`（见 `KeyboardSplitLayout.gapViewWidth`）。低于 required，隐藏时让 stack view 的零宽约束胜出。
    let width = gap.widthAnchor.constraint(
      equalTo: root.widthAnchor, multiplier: KeyboardSplitLayout.gapRatio,
      constant: -2 * CGFloat(KeyboardLayoutPreference.keySpacing))
    width.priority = .init(999)
    width.isActive = true
    splitGaps.append(gap)
    splitGapWidths.append(width)
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
    // ，。？ 在左边竖排，与网格的三行对齐；！ 在右列底部，与 Android 的 rebuildNineKeyRows 一致。
    for symbol in Self.nineKeySidebarMarks {
      let button = makeKey(title: symbol, accessibilityLabel: "符号 \(symbol)") { [weak self] in
        guard let self else { return }
        countKeyPress(TypingKeyID.punctuation)
        handleSymbol(nineKeyMark(symbol))
      }
      Self.drawBareInSidebar(button)
      punctuationStack.addArrangedSubview(button)
      nineKeyMarkKeys.append((button, symbol))
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
    // 笔画键与九键网格占同一格：笔画方案沿用九键的标点栏、删除列和动作行，只换中间这块。
    strokeKeys = StrokeKeypadView(makeKey: { [unowned self] title, label, action in
      makeKey(title: title, accessibilityLabel: label, action: action)
    })
    strokeKeys.isHidden = true
    strokeKeys.onStroke = { [weak self] key in
      self?.countKeyPress(TypingKeyID.character(key))
      self?.handleStrokeKey(key)
    }
    nineKeyContainer.addArrangedSubview(strokeKeys)
    // 2-9 键与下面的长按菜单共用键面文字。1 键是设计稿的 @#，打开符号面板而不交给 Engine（Engine 的九键编辑器不收 1），所以没有长按选项，也没有数字提示；它原来承担的音节分隔符改由右列的中间键承担，与 Android 一致。
    for rowIndex in 0..<3 {
      let row = makeRow()
      for column in 0..<3 {
        let digit = rowIndex * 3 + column + 1
        let letters = Self.nineKeyLetters[digit]
        let button = makeKey(
          title: letters ?? Self.nineKeySymbolsFace,
          accessibilityLabel: letters.map { "\(digit) \($0)" } ?? "符号"
        ) { [weak self] in
          guard let self else { return }
          let face = String(self.numberKeypadOrder.digit(row: rowIndex, column: column))
          // 26 键的九宫格数字层按输入的数字计数，与一行的 123 页记到同一个键上，统计页不会因此多出一块九宫格。
          self.countKeyPress(self.nineKeyDigitPadShown ? TypingKeyID.character(face) : TypingKeyID.nineKey(digit))
          if self.showsSymbols {
            self.handleSymbol(face)
          }
          else if letters == nil { self.showSymbolPanel() }
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
        var holdGesture: UILongPressGestureRecognizer?
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
          // 0.5 秒与 UIKit 自己的默认值、HarmonyOS `LongPressGesture` 的默认 500ms 一致；Android 这里用系统长按时长 `ViewConfiguration.getLongPressTimeout()`，12 起默认 400ms、之前 500ms。原来的 0.3 秒比哪一端都短，主线程稍一卡顿，一次普通的点按就会被当成长按、弹出菜单而不出字。
          hold.minimumPressDuration = Self.nineKeyHoldDuration
          button.addGestureRecognizer(hold)
          holdGesture = hold
          button.accessibilityHint = "长按输入 \(digit) 或 \(letters)"
        }
        nineKeyGridKeys.append(
          NineKeyGridKey(button: button, digit: digit, row: rowIndex, column: column, letters: letters,
                         numberHint: numberHint, hold: holdGesture))
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
    // ⌫、中间键和 ！ 各占这一列的三分之一，与网格的三行对齐。组字时按住 ⌫ 会丢掉整个组字（`repeatBackspace`），这就是设计稿拼音网格上的重输；Android 也是这样取舍的，这个位置改放分隔符。
    let delete = makeDeleteKey()
    delete.accessibilityIdentifier = "nineKeyDelete"
    controls.addArrangedSubview(delete)
    let middle = makeKey(title: "", accessibilityLabel: "", function: true) { [weak self] in
      self?.handleNineKeyMiddleKey()
    }
    middle.configuration?.contentInsets = .zero
    middle.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    middle.accessibilityIdentifier = "nineKeyMiddleKey"
    nineKeyMiddleButton = middle
    controls.addArrangedSubview(middle)
    let closing = Self.nineKeyClosingMark
    let exclamation = makeKey(title: closing, accessibilityLabel: "符号 \(closing)", function: true) { [weak self] in
      guard let self else { return }
      countKeyPress(TypingKeyID.punctuation)
      handleSymbol(nineKeyMark(closing))
    }
    exclamation.configuration?.contentInsets = .zero
    exclamation.accessibilityIdentifier = "nineKeyClosingMark"
    nineKeyMarkKeys.append((exclamation, closing))
    controls.addArrangedSubview(exclamation)
    nineKeyContainer.addArrangedSubview(controls)
    controls.widthAnchor.constraint(equalTo: sidebar.widthAnchor).isActive = true
    return nineKeyContainer
  }

  private func applyNineKeyDigitLayer(_ digits: Bool) {
    for key in nineKeyGridKeys {
      let face = numberKeypadOrder.digit(row: key.row, column: key.column)
      key.button.configuration?.title = digits ? String(face) : (key.letters ?? Self.nineKeySymbolsFace)
      key.button.accessibilityLabel = digits
        ? "数字 \(face)"
        : (key.letters.map { "\(key.digit) \($0)" } ?? "符号")
      key.numberHint?.isHidden = digits
      key.button.accessibilityHint = digits ? nil : key.letters.map { "长按输入 \(key.digit) 或 \($0)" }
      key.hold?.isEnabled = !nineKeyDigitPadShown
    }
    for key in nineKeyMarkKeys {
      let face = nineKeyMark(key.mark)
      guard key.button.configuration?.title != face else { continue }
      key.button.configuration?.title = face
      key.button.accessibilityLabel = "符号 \(face)"
    }
  }

  /// 九键外框的 ，。？！ 此刻画并输入的标点。26 键的九宫格数字层在英文、本地输入和写 ASCII 标点的方案（韩语、越南语、藏文）下与一行的 123 页一样用 ASCII 标点，其余情况是中文标点，交给 Engine 的标点路由。
  private func nineKeyMark(_ mark: String) -> String {
    guard nineKeyDigitPadShown, !symbolLayerIsChinese else { return mark }
    return Self.chineseSymbolFaces.first { $0.value == mark }?.key ?? mark
  }

  /// 26 键的 123 是否换成九键的数字层：「26 键数字键盘」选了九宫格、手机形态（含 iPad 的浮动键盘和窄窗口），而且字母层画的是 26 键字母（全拼、双拼、五笔、英文、日文罗马字、韩文等，本地输入模式也算）。九键网格有自己的数字层，笔画、手写、假名和大千保留各自的符号层；iPad 全尺寸键盘本来就有数字行，3×3 拉到整个键盘宽也不顺手，所以保留一行的 123 页，分离式键盘也就不受影响。不是 private：布局测试会固定它。
  static func opensNineKeyDigitPad(layout: KeyboardLayoutPreference.TwentySixKeyNumberLayout,
                                   formFactor: KeyboardFormFactor, letterKeys: Bool) -> Bool {
    layout == .nineKey && formFactor == .phone && letterKeys
  }

  /// 九键外框的左列；！ 是右列的最后一个键（`nineKeyClosingMark`）。
  static let nineKeySidebarMarks = ["，", "。", "？"]
  static let nineKeyClosingMark = "！"
  /// 拼音网格的 1 键，用来打开符号面板。
  static let nineKeySymbolsFace = "@#"

  /// 九键外框里当前界面右列中间键的用途。不是 private：布局测试会固定它。
  enum NineKeyMiddleKey: Equatable {
    /// 拼音网格：音节分隔符 `'`，只定音节在哪里结束，不定它的拼写。
    case separator
    /// 网格的数字层：句点，用于小数和版本号。
    case period
    /// 笔画键：丢掉已输入的笔画（即 Android 那里的重输）。
    case rewrite

    static func resolve(stroke: Bool, digits: Bool) -> NineKeyMiddleKey {
      stroke ? .rewrite : digits ? .period : .separator
    }

    var face: String {
      switch self {
      case .separator: return "分词"
      case .period: return "."
      case .rewrite: return "重输"
      }
    }

    var accessibilityLabel: String {
      switch self {
      case .separator: return "拼音分词"
      case .period: return "句点"
      case .rewrite: return "重新输入笔画"
      }
    }
  }

  private var nineKeyMiddleKey: NineKeyMiddleKey {
    NineKeyMiddleKey.resolve(stroke: typesStroke && !showsSymbols, digits: showsSymbols)
  }

  private func updateNineKeyMiddleKey() {
    guard let button = nineKeyMiddleButton else { return }
    let key = nineKeyMiddleKey
    if button.configuration?.title != key.face { button.configuration?.title = key.face }
    button.accessibilityLabel = key.accessibilityLabel
  }

  private func handleNineKeyMiddleKey() {
    switch nineKeyMiddleKey {
    case .separator:
      countKeyPress(TypingKeyID.character("'"))
      handleCharacter("'")
    case .period:
      countKeyPress(TypingKeyID.character("."))
      handleSymbol(".")
    case .rewrite:
      playInputClick()
      if hasComposition { render(discardComposition()) }
    }
  }

  /// 九键 2–9 按住多久弹出数字与字母菜单，理由见 `makeNineKeyLayout` 里设置它的地方。
  static let nineKeyHoldDuration: TimeInterval = 0.5

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

    // The composition gets its own line. Sharing the candidate row cost it up to 28% of the width
    // and left the candidates that much narrower, on the one row where width is worth most.
    let compositionRow = readingRow
    compositionRow.accessibilityIdentifier = "compositionRow"
    compositionRow.translatesAutoresizingMaskIntoConstraints = false
    container.addSubview(compositionRow)

    var preeditConfiguration = UIButton.Configuration.plain()
    // 按设计稿，拼写比候选列向内缩进 10pt（dc.html `padding-left: 10px`）。
    preeditConfiguration.contentInsets = NSDirectionalEdgeInsets(
      top: 0, leading: 10, bottom: 0, trailing: 8)
    // Truncate the tail. The head of a spelling is what tells the typist where a long composition
    // went wrong, so dropping it is dropping the useful half.
    preeditConfiguration.titleLineBreakMode = .byTruncatingTail
    preeditConfiguration.baseForegroundColor = KeyboardTheme.current.secondary
    preeditConfiguration.titleTextAttributesTransformer = Self.preeditTransformer(scale: preeditFontScale)
    preeditButton.configuration = preeditConfiguration
    preeditButton.setContentCompressionResistancePriority(.defaultHigh, for: .horizontal)
    preeditButton.showsMenuAsPrimaryAction = true
    preeditButton.accessibilityIdentifier = "preeditButton"

    updateLanguageModeButton()


    updateSchemeButton()


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

    configureExpandButton()
    expandCandidatesButton.addAction(
      UIAction { [weak self] _ in self?.toggleCandidatePanel() },
      for: .primaryActionTriggered)

    preeditButton.translatesAutoresizingMaskIntoConstraints = false
    compositionRow.addSubview(preeditButton)

    let content = UIStackView(arrangedSubviews: [
      candidateScrollView, diagnosticLabel,
      candidateEmptySpacer, expandDivider, expandCandidatesButton, exitLocalModeButton, hanjaButton,
    ])
    content.axis = .horizontal
    content.alignment = .center
    content.spacing = 12
    // 箭头紧贴它的分隔线，对应设计稿里紧跟细线的那个 40pt 按钮。
    content.setCustomSpacing(0, after: expandDivider)
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
      equalToConstant: shownTopRowHeight)
    topRowHeightConstraint = stripHeight
    let compositionHeight = compositionRow.heightAnchor.constraint(
      equalToConstant: Self.readingRowHeight(preeditScale: preeditFontScale))
    readingRowHeightConstraint = compositionHeight
    // 略低于 required：收起的顶栏（显示方式「隐藏」）比读音行还矮，这时由隐藏的候选行让步，而不是破坏这一行的高度。
    let contentBottom = content.bottomAnchor.constraint(equalTo: container.bottomAnchor)
    contentBottom.priority = .init(999)
    // 设计稿里这一行两端各有 2pt 内边距（dc.html `padding: 0 2px`），工具栏和候选列都一样。
    NSLayoutConstraint.activate([
      stripHeight,
      compositionRow.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 2),
      compositionRow.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -2),
      compositionRow.topAnchor.constraint(equalTo: container.topAnchor),
      compositionHeight,
      preeditButton.leadingAnchor.constraint(equalTo: compositionRow.leadingAnchor),
      preeditButton.trailingAnchor.constraint(lessThanOrEqualTo: compositionRow.trailingAnchor),
      preeditButton.centerYAnchor.constraint(equalTo: compositionRow.centerYAnchor),
      content.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 2),
      content.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -2),
      content.topAnchor.constraint(equalTo: compositionRow.bottomAnchor),
      contentBottom,
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
    // 设计稿的工具栏是等分网格：无论固定了哪些，每个可见工具都占同样宽的格子。
    shortcutBar.distribution = .fillEqually
    shortcutBar.alignment = .fill
    shortcutBar.spacing = 0
    shortcutBar.accessibilityIdentifier = "keyboardShortcutBar"
    shortcutBar.translatesAutoresizingMaskIntoConstraints = false
    moreShortcut.accessibilityIdentifier = "moreShortcut"
    emojiShortcut.accessibilityIdentifier = "emojiShortcut"
    phrasesShortcut.accessibilityIdentifier = "phrasesShortcut"
    clipboardShortcut.accessibilityIdentifier = "clipboardShortcut"
    skinShortcut.accessibilityIdentifier = "skinShortcut"
    schemeButton.accessibilityIdentifier = "schemeButton"
    dismissShortcut.accessibilityIdentifier = "dismissShortcut"
    // 先按设计稿手机工具栏的顺序，再排「工具栏按钮」可以固定的 iOS 额外按钮，语音入口和收起放在最后。
    let shortcuts: [UIButton] = [
      moreShortcut, emojiShortcut, phrasesShortcut, clipboardShortcut, skinShortcut, schemeButton,
      aiShortcut, characterSetShortcut, fullwidthShortcut, punctuationShortcut,
      replyShortcut, scriptShortcut, dismissShortcut,
    ]
    for button in shortcuts {
      shortcutBar.addArrangedSubview(button)
    }
    container.addSubview(shortcutBar)
    // 与候选行相同：顶栏收起时由隐藏的工具栏让步。
    let shortcutBarBottom = shortcutBar.bottomAnchor.constraint(equalTo: container.bottomAnchor)
    shortcutBarBottom.priority = .init(999)
    // 空闲时顶栏只显示工具栏，图标在整行里居中；组字时在同一位置换成读音行和候选行，所以组字开始时什么都不会移位。
    NSLayoutConstraint.activate([
      shortcutBar.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 2),
      shortcutBar.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -2),
      shortcutBar.topAnchor.constraint(equalTo: container.topAnchor),
      shortcutBarBottom,
    ])
    scriptShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      countKeyPress(TypingKeyID.voice)
      showKeyboardVoice()
    }, for: .primaryActionTriggered)
    replyShortcut.addAction(UIAction { [weak self] _ in self?.toggleReplyKeyboard() }, for: .primaryActionTriggered)
    // 工具栏图标对应的面板已打开时，再点就关掉；否则在键区打开它的面板。
    emojiShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      if emojiPicker != nil { closeKeyboardPicker(); return }
      countKeyPress(TypingKeyID.emoji)
      showEmojiPicker()
    }, for: .primaryActionTriggered)
    phrasesShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      if phrasesPanel != nil { closeKeyboardPicker() } else { showPhrasesPanel() }
    }, for: .primaryActionTriggered)
    skinShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      if skinPicker != nil { closeKeyboardPicker() } else { showSkinPicker() }
    }, for: .primaryActionTriggered)
    clipboardShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      if clipboardPanel != nil { closeKeyboardPicker() } else { showClipboardHistory() }
    }, for: .primaryActionTriggered)
    schemeButton.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      if schemePicker != nil { closeKeyboardPicker() } else { showSchemePicker() }
    }, for: .primaryActionTriggered)
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
    moreShortcut.addAction(UIAction { [weak self] _ in self?.toggleMorePicker() }, for: .primaryActionTriggered)
    dismissShortcut.addAction(UIAction { [weak self] _ in self?.dismissOrReturnToKeys() }, for: .primaryActionTriggered)
    updateShortcutButtons()
  }

  private func updateShortcutButtons() {
    let skin = KeyboardTheme.current
    // 固定的额外按钮保留 SF Symbols 或文字键面；它们和旁边设计稿的线框图标一样用按键前景色绘制。
    func configure(_ button: UIButton, title: String?, symbol: String?, label: String, id: String) {
      var configuration = UIButton.Configuration.plain()
      configuration.title = title
      configuration.image = symbol.flatMap { UIImage(systemName: $0) }
      configuration.preferredSymbolConfigurationForImage = .init(pointSize: 18, weight: .regular)
      configuration.baseForegroundColor = skin.keyForeground
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
    for button in [emojiShortcut, phrasesShortcut, clipboardShortcut, skinShortcut, schemeButton, dismissShortcut] {
      button.apply(skin: skin)
    }
    moreShortcut.apply(skin: skin)
    moreShortcut.accessibilityLabel = "功能"
    // 语音入口只在「顶部语音入口」打开时显示；简繁是设一次就很少再动的开关，和其他开关一起放在功能菜单里。
    configure(scriptShortcut, title: nil, symbol: "waveform", label: "语音结果", id: "layoutVoiceShortcut")
    scriptShortcut.isEnabled = true
    scriptShortcut.accessibilityValue = nil
    scriptShortcut.isHidden = !KeyboardLayoutPreference.voiceShortcutEnabled
    configure(replyShortcut, title: nil, symbol: "bubble.left.and.text.bubble.right",
      label: "高情商回复", id: "replyShortcut")
    replyShortcut.accessibilityValue = replyPanel != nil ? "已打开" : nil
    replyShortcut.isHidden = true
    skinShortcut.accessibilityValue = skin.title
    schemeButton.accessibilityValue = inputScheme.title
    // The switches show their state as a character, the way the Windows floating toolbar does: 简/繁, 全/半, and a Chinese or ASCII comma.
    let pinned = TouchToolbarPreference(in: session.sharedPreferences)
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
    punctuationShortcut.isEnabled = chinesePunctuationSwitchApplies
    emojiShortcut.isHidden = !pinned.emoji
    phrasesShortcut.isHidden = !toolbarPhrases
    schemeButton.isHidden = !toolbarScheme
    skinShortcut.isHidden = !pinned.skin
    clipboardShortcut.isHidden = !pinned.clipboard
    aiShortcut.isHidden = !pinned.ai
    characterSetShortcut.isHidden = !pinned.characterSet
    fullwidthShortcut.isHidden = !pinned.fullwidth
    punctuationShortcut.isHidden = !pinned.punctuation
    updateToolbarActiveState()
    updateMorePickerPage()
  }

  /// 工具栏里高亮的按钮：面板已打开的那个图标，以及任何面板、候选网格或回复面板盖住按键时的品牌键。
  private func updateToolbarActiveState() {
    emojiShortcut.isActive = emojiPicker != nil
    phrasesShortcut.isActive = phrasesPanel != nil
    clipboardShortcut.isActive = clipboardPanel != nil
    skinShortcut.isActive = skinPicker != nil
    schemeButton.isActive = schemePicker != nil
    moreShortcut.isActive = keyAreaPanel != nil || replyKeyboardShown
    moreShortcut.accessibilityValue = morePicker != nil ? "已打开" : nil
    dismissShortcut.accessibilityLabel = keyAreaPanel != nil || replyKeyboardShown ? "返回键盘" : "收起键盘"
  }

  /// 锁定的标点设置自己说了算，英文模式无论如何都输入 ASCII 标点，所以「中文标点」开关只在「跟随中英文」下的中文模式里有意义，而且只对写中文标点的方案有意义：标点走 Engine 标点路由的方案，以及注音，它的符号面板也按同一个开关选标点。
  private var chinesePunctuationSwitchApplies: Bool {
    isChineseMode && (inputScheme.writesChinese || inputScheme.isCantonese || inputScheme.isZhuyin || inputScheme.isStroke)
      && (session.sharedPreferences?["punctuation_lock"] as? String ?? "follow") == "follow"
  }

  /// 模糊音是否适用：中文模式下的拼音方案，即 Engine 会按模糊规则读取其拼写的那些方案。
  private var fuzzyPinyinApplies: Bool {
    isChineseMode && inputScheme.writesChinese && inputScheme != .wubi && inputScheme != .handwriting
  }

  /// 功能菜单（功能），按 Android 的 `FunctionPanelModel` 顺序：先是设计稿里的项，再是没有别处可放的 iOS 工具。
  ///
  /// 开关类的项让菜单停在当前页；会跳到别处的项先关掉菜单。振动相关的项只在有 Taptic Engine 的设备上出现。
  private func makeTools() -> [KeyboardTool] {
    var tools: [KeyboardTool] = [
      KeyboardTool(id: "fullWidth", title: "全角", accessibilityLabel: "全角输入", face: .glyph("全"),
                   isToggle: true, selected: fullWidthInput) { [weak self] in
        guard let self else { return }
        setFullWidthInput(!fullWidthInput)
        updateShortcutButtons()
      },
      KeyboardTool(id: "chinesePunctuation", title: "中文标点", face: .glyph("，"), isToggle: true,
                   selected: chinesePunctuation, enabled: chinesePunctuationSwitchApplies) { [weak self] in
        guard let self else { return }
        setChinesePunctuation(!chinesePunctuation)
        updateShortcutButtons()
      },
      KeyboardTool(id: "fuzzyPinyin", title: "模糊音", face: .glyph("≈"), isToggle: true,
                   selected: FuzzyPinyinPreference.settings(in: session.sharedPreferences)?.enabled == true,
                   enabled: fuzzyPinyinApplies) { [weak self] in
        guard let self else { return }
        let enabled = FuzzyPinyinPreference.settings(in: session.sharedPreferences)?.enabled == true
        session.setFuzzyPinyinEnabled(!enabled)
        applyLearningPreferences()
        updateShortcutButtons()
      },
      // 用一个开关而不是一对卡片：输出字形只是一个布尔值。
      KeyboardTool(id: "traditional", title: "繁体", accessibilityLabel: "繁体输出", face: .glyph("繁"), isToggle: true,
                   selected: usesTraditionalOutput, enabled: !(isChineseMode && !inputScheme.writesChinese)) { [weak self] in
        guard let self else { return }
        selectTraditionalOutput(!usesTraditionalOutput)
      },
      KeyboardTool(id: "handwriting", title: "手写", face: .icon(.handwriting),
                   selected: isChineseMode && inputScheme == .handwriting,
                   enabled: InputSchemePreference.offeredSchemes.contains(.handwriting)) { [weak self] in
        guard let self else { return }
        // 与在方案选择里选手写的步骤相同。
        closeKeyboardPicker()
        if !isChineseMode { toggleInputMode() }
        selectInputScheme(.handwriting)
      },
      KeyboardTool(id: "dictionary", title: "词库", face: .icon(.lexicon)) { [weak self] in
        self?.openApp(KeyboardAppLauncher.dictionaryURL)
      },
      KeyboardTool(id: "keyboardHeight", title: "键盘高度", face: .icon(.keyboardHeight)) { [weak self] in
        self?.showInlineHeight()
      },
      // 键盘只改这个菜单里有的设置；其余设置都在 app 里，正在打字的人没有别的路可以过去。
      KeyboardTool(id: "settings", title: "设置", face: .icon(.settings)) { [weak self] in
        self?.openApp(KeyboardAppLauncher.inputSettingsURL)
      },
      KeyboardTool(id: "keySound", title: "按键音", face: .icon(.keySound), isToggle: true,
                   selected: KeyboardFeedbackPreference.soundEnabled) { [weak self] in
        KeyboardFeedbackPreference.defaults.set(!KeyboardFeedbackPreference.soundEnabled,
                                                 forKey: KeyboardFeedbackPreference.soundKey)
        if KeyboardFeedbackPreference.soundEnabled { UIDevice.current.playInputClick() }
        self?.updateShortcutButtons()
      },
    ]
    if KeyboardFeedbackPreference.hapticsAvailable {
      tools.append(KeyboardTool(id: "vibration", title: "振动", accessibilityLabel: "按键振动", face: .icon(.vibration),
                                isToggle: true, selected: KeyboardFeedbackPreference.hapticsEnabled) { [weak self] in
        KeyboardFeedbackPreference.defaults.set(!KeyboardFeedbackPreference.hapticsEnabled,
                                                 forKey: KeyboardFeedbackPreference.hapticsKey)
        if KeyboardFeedbackPreference.hapticsEnabled {
          if let self { KeyboardFeedbackPreference.hapticStrength.impact(self.keyFeedback) }
          self?.prepareKeyFeedback()
        }
        self?.updateShortcutButtons()
      })
    }
    tools += [
      // 与 Android 的磁贴一样：轻点在关闭和靠右之间切换，长按换边。磁贴显示存储的模式，在全宽的 iPad 键盘上变暗，因为那里从不画单手模式。
      KeyboardTool(id: "oneHand", title: "单手模式", face: .icon(.oneHand), isToggle: true, selected: oneHandedMode != .off,
                   enabled: formFactor == .phone) { [weak self] in
        guard let self else { return }
        setOneHanded(oneHandedMode.toggled(swapSide: false))
      } longPress: { [weak self] in
        guard let self else { return }
        setOneHanded(oneHandedMode.toggled(swapSide: true))
      },
      // 隐私模式打开期间停止学习和统计；磁贴保持选中状态来表明这一点。
      KeyboardTool(id: "incognito", title: "隐私模式", face: .icon(.incognito), isToggle: true, selected: incognito) { [weak self] in
        guard let self else { return }
        incognito.toggle()
        KeyboardPrivacyPreference.incognito = incognito
        applyLearningPreferences()
        configureDiagnosticLog()
        updateShortcutButtons()
      },
      KeyboardTool(id: "feedback", title: "反馈", face: .icon(.feedback)) { [weak self] in
        self?.openApp(KeyboardAppLauncher.feedbackURL)
      },
      KeyboardTool(id: "about", title: "关于", face: .icon(.about)) { [weak self] in
        self?.openApp(KeyboardAppLauncher.aboutURL)
      },
      KeyboardTool(id: "aiAssist", title: "AI 润色", face: .icon(.aiAssist)) { [weak self] in
        self?.closeKeyboardPicker(); self?.showKeyboardAI()
      },
      KeyboardTool(id: "reply", title: "高情商回复", face: .glyph("回"), selected: replyKeyboardShown) { [weak self] in
        self?.closeKeyboardPicker(); self?.toggleReplyKeyboard()
      },
      KeyboardTool(id: "localInput", title: "本地输入", face: .icon(.localInput),
                   enabled: supportsLocalTools && !enabledLocalInputModes.isEmpty) { [weak self] in
        self?.showMoreToolsPage(.localInput)
      },
      KeyboardTool(id: "voiceResult", title: "语音结果", face: .icon(.voiceResult)) { [weak self] in
        self?.closeKeyboardPicker(); self?.showKeyboardVoice()
      },
    ]
    if KeyboardFeedbackPreference.hapticsAvailable {
      let strength = KeyboardFeedbackPreference.hapticStrength
      tools.append(KeyboardTool(id: "vibrationStrength", title: "振动强度 \(strength.title)", accessibilityLabel: "振动强度",
                                face: .icon(.vibrationStrength), enabled: KeyboardFeedbackPreference.hapticsEnabled,
                                caption: strength.title) { [weak self] in
        guard let self else { return }
        let strengths = KeyboardHapticStrength.allCases
        let current = strengths.firstIndex(of: KeyboardFeedbackPreference.hapticStrength) ?? 0
        let next = strengths[(current + 1) % strengths.count]
        KeyboardFeedbackPreference.defaults.set(next.rawValue, forKey: KeyboardFeedbackPreference.strengthKey)
        next.impact(keyFeedback)
        prepareKeyFeedback()
        updateShortcutButtons()
      })
    }
    tools += [
      KeyboardTool(id: "emoji", title: "表情", face: .icon(.toolbarEmoji)) { [weak self] in
        self?.countKeyPress(TypingKeyID.emoji)
        self?.showEmojiPicker()
      },
      KeyboardTool(id: "clipboardHistory", title: "剪贴板历史", face: .icon(.clipboardHistory)) { [weak self] in
        self?.showClipboardHistory()
      },
      // Windows 用 Ctrl+Shift+Alt+C 清除候选缓存。iOS 不把硬件按键交给第三方键盘，这里按不出这个组合键，所以菜单把它做成一个磁贴。
      KeyboardTool(id: "clearCandidateCache", title: "清除候选缓存", face: .glyph("清")) { [weak self] in
        self?.closeKeyboardPicker(); self?.resetCandidateCache()
      },
    ]
    return tools
  }

  /// 关掉面板并打开 app 的某个页面；app 在前台时面板没有可显示的内容。
  private func openApp(_ url: URL) {
    closeKeyboardPicker()
    KeyboardAppLauncher.open(url, from: self)
  }

  private func resetCandidateCache() {
    let snapshot = session.resetCache()
    render(snapshot)
    guard snapshot.diagnosticText == nil else { return }
    showDiagnostic("已清除候选缓存")
    renderCandidateStrip()
  }

  /// 本地输入的各个模式以磁贴列出，前面是回到菜单的「返回工具」磁贴。每个磁贴标出该模式的触发字母，即在硬件键盘上打开它的那个键。
  private func makeLocalModeTools() -> [KeyboardTool] {
    [KeyboardTool(id: "backToTools", title: "返回工具", face: .glyph("‹")) { [weak self] in
      self?.showMoreToolsPage(.root)
    }] + enabledLocalInputModes.map { mode in
      KeyboardTool(id: "localMode-" + mode.trigger, title: mode.title, face: .glyph(mode.trigger),
                   enabled: supportsLocalTools) { [weak self] in
        self?.closeKeyboardPicker()
        self?.openLocalInputMode(mode.trigger)
      }
    }
  }

  private func showMoreToolsPage(_ page: MoreToolsPage) {
    moreToolsPage = page
    updateMorePickerPage()
    morePicker?.resetToFirstPage()
  }

  /// 按当前开关状态重画打开着的菜单；工具只在菜单显示时才构建。
  private func updateMorePickerPage() {
    guard let morePicker else { return }
    switch moreToolsPage {
    case .root: morePicker.update(tools: makeTools())
    case .localInput: morePicker.update(tools: makeLocalModeTools())
    }
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
      button.accessibilityLabel = KeyboardNineKeyPanelColumns.spellingAccessibilityLabel(spelling)
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
      let button = makeIconKey(Self.shiftIcon, accessibilityLabel: "大写") { [weak self] in
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
      // 下滑或长按会像符号层按键那样输入角标提示的符号，并记作这个字母键的一次按键，与 Android 的 commitNineKeyLiteral 路径一致。
      (button as? KeyboardKeyButton)?.onCornerHint = { [weak self] hint in
        self?.countKeyPress(TypingKeyID.character(text))
        self?.insertSymbolLayerLiteral(hint)
      }
      applyLetterFont(to: button)
      letterButtons.append((button: button, lowercase: text, hint: attachHintLabel(to: button),
                            corner: attachCornerHintLabel(to: button)))
      row.addArrangedSubview(button)
    }
    if includesShift {
      let delete = makeDeleteKey()
      delete.accessibilityIdentifier = "letterDeleteKey"
      letterDeleteKey = delete
      row.addArrangedSubview(delete)
      row.distribution = .fill
      // 让 Shift 和删除键容易按到；七个字母在两者之间均分。iPad 键盘把 ⌫ 移到第一排，所以它的宽度约束低于 required，让 stack view 能在这里把它隐藏。
      let shift = row.arrangedSubviews[0]
      let keys = Array(row.arrangedSubviews.dropFirst().dropLast())
      let deleteWidth = delete.widthAnchor.constraint(equalToConstant: Self.phoneLetterEdgeWidth)
      deleteWidth.priority = .init(999)
      let shiftWidth = shift.widthAnchor.constraint(equalToConstant: Self.phoneLetterEdgeWidth)
      phoneShiftWidth = shiftWidth
      tabletShiftWidth = shift.widthAnchor.constraint(equalTo: keys[0].widthAnchor, multiplier: TabletLetterLayout.shiftWeight)
      NSLayoutConstraint.activate([shiftWidth, deleteWidth]
        + keys.dropFirst().map { $0.widthAnchor.constraint(equalTo: keys[0].widthAnchor) })
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
      // iPad 键盘在 ，。 之后的第二个 ⇧，手机在这个位置是 ⌫。两个 Shift 是同一个键。
      let rightShift = makeIconKey(Self.shiftIcon, accessibilityLabel: "大写") { [weak self] in
        self?.countKeyPress(TypingKeyID.shiftRight)
        self?.toggleLetterCase()
      }
      rightShift.accessibilityIdentifier = "rightShiftButton"
      rightShift.isHidden = true
      rightShiftButton = rightShift
      row.addArrangedSubview(rightShift)
      let rightShiftWidth = rightShift.widthAnchor.constraint(equalTo: keys[0].widthAnchor, multiplier: TabletLetterLayout.shiftWeight)
      rightShiftWidth.priority = .init(999)
      rightShiftWidth.isActive = true
    }
    if letters == letterRows[0] {
      // iPad 键盘的 ⌫ 在第一排末尾；手机仍放在第三排。
      let delete = makeDeleteKey()
      delete.accessibilityIdentifier = "tabletDeleteKey"
      delete.isHidden = true
      tabletDeleteKey = delete
      row.addArrangedSubview(delete)
      let tab = makeSymbolKey(symbol: "arrow.right.to.line", accessibilityLabel: "Tab") { [weak self] in
        self?.countKeyPress(TypingKeyID.tab)
        self?.handleTab()
      }
      tab.accessibilityIdentifier = "tabKey"
      tab.isHidden = true
      tabKey = tab
      row.insertArrangedSubview(tab, at: 0)
      row.distribution = .fill
      // 字母保持等宽；Tab 宽一个半键，⌫ 宽 1.3 个键，都低于 required，Tab 或 ⌫ 隐藏时由字母填满这一排。
      let keys = row.arrangedSubviews.filter { $0 !== tab && $0 !== delete }
      NSLayoutConstraint.activate(keys.dropFirst().map { $0.widthAnchor.constraint(equalTo: keys[0].widthAnchor) })
      let width = tab.widthAnchor.constraint(equalTo: keys[0].widthAnchor, multiplier: 1.5)
      width.priority = .init(999)
      width.isActive = true
      let deleteWidth = delete.widthAnchor.constraint(equalTo: keys[0].widthAnchor, multiplier: TabletLetterLayout.deleteWeight)
      deleteWidth.priority = .init(999)
      deleteWidth.isActive = true
    }
    if letters == letterRows[1] {
      let key = makeKey(title: ";", accessibilityLabel: "微软双拼 ing") { [weak self] in
        self?.countKeyPress(TypingKeyID.character(";"))
        self?.handleCharacter(";")
      }
      key.accessibilityIdentifier = "microsoftFinalKey"
      (key as? KeyboardKeyButton)?.showsPressPreview = true
      applyLetterFont(to: key)
      microsoftFinalKey = key
      letterButtons.append((button: key, lowercase: ";", hint: attachHintLabel(to: key), corner: attachCornerHintLabel(to: key)))
      row.addArrangedSubview(key)
      // iPad 键盘的回车在第二排末尾，底行没有回车。它的外观和行为都与底行的回车相同（`returnKeys`）。
      let enter = makeKey(title: "换行", accessibilityLabel: "换行", function: true) { [weak self] in
        self?.countKeyPress(TypingKeyID.enter)
        self?.handleReturn()
      }
      enter.accessibilityIdentifier = "tabletReturnKey"
      enter.titleLabel?.adjustsFontSizeToFitWidth = true
      enter.titleLabel?.minimumScaleFactor = 0.65
      enter.isHidden = true
      tabletReturnKey = enter
      row.addArrangedSubview(enter)
      row.distribution = .fill
      // 字母保持等宽，微软双拼的 `;` 以低于 required 的约束与它们等宽，隐藏时 stack view 的零宽约束胜出；回车宽 1.75 个键，同样低于 required，让手机能把它隐藏。
      let keys = row.arrangedSubviews.filter { $0 !== enter }
      for other in keys.dropFirst() {
        let width = other.widthAnchor.constraint(equalTo: keys[0].widthAnchor)
        if other === key { width.priority = .init(999) }
        width.isActive = true
      }
      let enterWidth = enter.widthAnchor.constraint(equalTo: keys[0].widthAnchor, multiplier: TabletLetterLayout.returnWeight)
      enterWidth.priority = .init(999)
      enterWidth.isActive = true
    }
    return row
  }

  /// 手机第三排字母上的 ⇧ 和 ⌫。
  private static let phoneLetterEdgeWidth: CGFloat = 44

  /// 设计稿的字母键面：手机 22pt，iPad 键盘 20pt，固定字号而不跟随动态字体，免得大字设置把字母挤出按键。
  static func letterFontSize(_ formFactor: KeyboardFormFactor) -> CGFloat { formFactor == .tablet ? 20 : 22 }

  private func applyLetterFont(to button: UIButton) {
    button.configuration?.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { [weak self] attributes in
      var attributes = attributes
      let size = Self.letterFontSize(self?.formFactor ?? .phone)
      attributes.font = KeyboardTheme.current.usesMonospacedFont
        ? .monospacedSystemFont(ofSize: size, weight: .regular) : .systemFont(ofSize: size)
      return attributes
    }
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

  /// 设计稿的符号提示：10pt，用皮肤的次要颜色，距按键上边 3pt、右边 5pt，与 Android 的 KeyHintButton 画法一致。它只是画上去的，所以按键的无障碍标签仍是字母。
  private func attachCornerHintLabel(to button: UIButton) -> UILabel {
    let label = UILabel()
    label.font = .systemFont(ofSize: 10, weight: .regular)
    label.textColor = KeyboardTheme.current.secondary
    label.textAlignment = .right
    label.numberOfLines = 1
    label.isHidden = true
    label.isAccessibilityElement = false
    label.accessibilityIdentifier = "letterCornerHint"
    label.translatesAutoresizingMaskIntoConstraints = false
    button.addSubview(label)
    NSLayoutConstraint.activate([
      label.topAnchor.constraint(equalTo: button.topAnchor, constant: 3),
      label.trailingAnchor.constraint(equalTo: button.trailingAnchor, constant: -5),
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

  /// 设计稿 123 / #+= 层三排按键中的一排。按键只建一次，每次换层时从 `SymbolLayerLayout` 取键面（`updateSymbolLayerFaces`）；点按时读屏幕上的键面，所以按键显示的和写出的不会对不上。第三排在五个标点键两侧放 `#+=` / `123` 切换键和 ⌫，两端各宽 1.4 个键。
  private func makeSymbolLayerRow(_ index: Int) -> UIStackView {
    let row = makeRow()
    row.accessibilityIdentifier = "symbolLayerRow\(index)"
    let count = SymbolLayerLayout.characterRows(more: false, chinese: true)[index].count
    var keys: [UIButton] = []
    for column in 0..<count {
      let key = makeKey(title: "", accessibilityLabel: "") { [weak self] in
        self?.typeSymbolLayerKey(row: index, column: column)
      }
      (key as? KeyboardKeyButton)?.showsPressPreview = true
      key.configuration?.contentInsets = .zero
      // Android 的符号层按键：第一排 20，下面几排 18。
      applySymbolLayerFont(to: key, size: index == 0 ? 20 : 18)
      key.accessibilityIdentifier = "symbolLayerKey\(index)_\(column)"
      keys.append(key)
      row.addArrangedSubview(key)
    }
    symbolLayerKeys.append(keys)
    guard index == 2, let first = keys.first else { return row }
    let toggle = makeKey(title: SymbolLayerLayout.toggleTitle(more: false),
                         accessibilityLabel: SymbolLayerLayout.toggleLabel(more: false), function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.layer)
      self?.toggleMoreSymbols()
    }
    toggle.configuration?.contentInsets = .zero
    toggle.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    toggle.accessibilityIdentifier = "symbolLayerToggle"
    symbolLayerToggle = toggle
    row.insertArrangedSubview(toggle, at: 0)
    let delete = makeDeleteKey()
    delete.accessibilityIdentifier = "symbolLayerDeleteKey"
    row.addArrangedSubview(delete)
    row.distribution = .fill
    NSLayoutConstraint.activate(keys.dropFirst().map { $0.widthAnchor.constraint(equalTo: first.widthAnchor) } + [
      toggle.widthAnchor.constraint(equalTo: first.widthAnchor, multiplier: SymbolLayerLayout.edgeWeight),
      delete.widthAnchor.constraint(equalTo: first.widthAnchor, multiplier: SymbolLayerLayout.edgeWeight),
    ])
    return row
  }

  /// 符号层按键的键面，与字母一样用固定字号，免得大字设置把符号挤出按键。
  private func applySymbolLayerFont(to button: UIButton, size: CGFloat) {
    button.configuration?.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
      var attributes = attributes
      attributes.font = KeyboardTheme.current.usesMonospacedFont
        ? .monospacedSystemFont(ofSize: size, weight: .regular) : .systemFont(ofSize: size)
      return attributes
    }
  }

  /// 123 / #+= 层是否画中文版本：与底行 ，。 遵循同一规则。英文、本地工具和写 ASCII 标点的方案（韩语、越南语、藏文）用 ASCII 版本，其中的标点正是这些输入会写出的。
  private var symbolLayerIsChinese: Bool {
    isChineseMode && !isInLocalMode && !inputScheme.writesAsciiPunctuation
  }

  /// 手机底行的逗号键是否画并输入日语的 、：日语方案的中文模式，不在本地输入模式里。
  private var writesJapaneseComma: Bool {
    isChineseMode && inputScheme.isJapanese && !isInLocalMode
  }

  /// 哪些界面用设计稿的 123 / #+= 层作为符号层，对应 Android 的 `drawsDesignLayer`：所有方案和英文下的 26 键、手写以及笔画键。九键网格在网格上保留自己的数字层，假名键有自己的符号层，大千各排保留共用的每排十键符号行。不是 private：布局测试会固定它。
  static func drawsSymbolLayer(symbols: Bool, nineKey: Bool, kana: Bool, dachen: Bool) -> Bool {
    symbols && !nineKey && !kana && !dachen
  }

  private func typeSymbolLayerKey(row: Int, column: Int) {
    let chinese = symbolLayerIsChinese
    let face = SymbolLayerLayout.characterRows(more: showsMoreSymbols, chinese: chinese)[row][column]
    switch SymbolLayerLayout.input(for: face, chinese: chinese, chinesePunctuation: symbolLayerEngineWritesChinesePunctuation) {
    case .symbol(let key):
      countKeyPress(TypingKeyID.character(key))
      handleSymbol(key)
    case .literal(let text):
      countKeyPress(TypingKeyID.character(text))
      insertSymbolLayerLiteral(text)
    }
  }

  /// Engine 此刻是否把中文层送出的 ASCII 键写成中文标点：中文标点开关与标点锁定共同决定，规则与注音符号面板相同。
  private var symbolLayerEngineWritesChinesePunctuation: Bool {
    Self.writesChinesePunctuation(switchOn: chinesePunctuation,
                                  punctuationLock: session.sharedPreferences?["punctuation_lock"] as? String)
  }

  /// 在 Engine 标点路由上没有对应按键的符号层标点、中文标点关闭时中文层上的标点，或字母键的角标提示：结束组字，按键面原样写出，与 `handleSymbol` 处理 Engine 没有路由的符号时一样。
  private func insertSymbolLayerLiteral(_ text: String) {
    playInputClick()
    guard isChineseMode else {
      insertDirectText(text)
      refreshEnglishSuggestions()
      return
    }
    render(session.finishComposition())
    insertDirectText(text)
  }

  private func toggleMoreSymbols() {
    playInputClick()
    showsMoreSymbols.toggle()
    updateKeyboardLayout()
  }

  private func updateSymbolLayerFaces() {
    let rows = SymbolLayerLayout.characterRows(more: showsMoreSymbols, chinese: symbolLayerIsChinese)
    for (keys, faces) in zip(symbolLayerKeys, rows) {
      for (key, face) in zip(keys, faces) where key.configuration?.title != face {
        key.configuration?.title = face
        key.accessibilityLabel = "符号 \(face)"
      }
    }
    if let toggle = symbolLayerToggle, toggle.configuration?.title != SymbolLayerLayout.toggleTitle(more: showsMoreSymbols) {
      toggle.configuration?.title = SymbolLayerLayout.toggleTitle(more: showsMoreSymbols)
      toggle.accessibilityLabel = SymbolLayerLayout.toggleLabel(more: showsMoreSymbols)
    }
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
      configuration.titleTextAttributesTransformer = Self.functionLabelTransformer
      layoutToggle.configuration = configuration
    }
    layoutToggle.titleLabel?.adjustsFontSizeToFitWidth = true
    layoutToggle.titleLabel?.minimumScaleFactor = 0.7
    layoutToggle.titleLabel?.lineBreakMode = .byClipping
    layoutToggle.accessibilityIdentifier = "layoutToggleButton"
    // 长按字母层的 123 直接打开符号面板，不必再经过 #+= 层；符号层里这个键是 ABC，手势不会开始（`gestureRecognizerShouldBegin`）。长按会取消这次触摸，所以不会再切层。
    let symbolsHold = UILongPressGestureRecognizer(target: self, action: #selector(handleLayoutToggleHold(_:)))
    symbolsHold.name = "layoutToggleSymbolsHold"
    symbolsHold.minimumPressDuration = Self.nineKeyHoldDuration
    symbolsHold.cancelsTouchesInView = true
    symbolsHold.delegate = self
    layoutToggle.addGestureRecognizer(symbolsHold)
    layoutToggle.accessibilityCustomActions = [
      UIAccessibilityCustomAction(name: "打开符号面板") { [weak self] _ in
        guard let self, !self.showsSymbols else { return false }
        self.openSymbolPanelFromLayoutToggle()
        return true
      }
    ]
    layoutToggleButton = layoutToggle
    nineKeySymbolsButton = makeKey(title: "符", accessibilityLabel: "符号", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.symbol)
      self?.showSymbolPanel()
    }
    nineKeySymbolsButton.configuration?.contentInsets = .zero
    nineKeySymbolsButton.accessibilityIdentifier = "symbolPanelKey"
    row.addArrangedSubview(nineKeySymbolsButton)
    row.addArrangedSubview(layoutToggle)

    let emoji = makeSymbolKey(symbol: "face.smiling", accessibilityLabel: "表情") { [weak self] in
      self?.countKeyPress(TypingKeyID.emoji)
      self?.showEmojiPicker()
    }
    emoji.configuration?.image = KeyboardIcon.emoji.image(pointSize: Self.layerEmojiIconSide)
    emoji.accessibilityIdentifier = "layerEmojiKey"
    emoji.isHidden = true
    layerEmojiButton = emoji
    row.addArrangedSubview(emoji)

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
      // 手机 26 键底行上这个键是设计稿的 ，：发逗号键，由 Engine 的标点路由选出标点，与符号行一样。日语画的是 、，发的也是 、（经 `KeyboardPunctuationContext` 换成 `\` 键）：Engine 的日语方案走中文标点表，逗号键写出的是 ，，与键面不符。
      handleSymbol(phoneBottomRowShown ? (writesJapaneseComma ? "、" : ",") : quickPunctuationSymbols[0])
    }
    punctuation.configuration?.contentInsets = .zero
    punctuation.accessibilityIdentifier = "quickPunctuationKey"
    punctuation.accessibilityHint = "轻点输入，长按选择常用标点"
    quickPunctuationButton = punctuation
    quickPunctuationWidth = punctuation.widthAnchor.constraint(equalToConstant: 44)
    symbolKeyFaces.append((punctuation, ",", Self.chineseSymbolFaces[","] ?? ","))
    row.addArrangedSubview(punctuation)

    let space = makeSpaceKey()
    space.accessibilityIdentifier = "spaceKey"
    spaceButton = space
    row.addArrangedSubview(space)
    // 分离式键盘右半的空格：轻点、滑动移动光标和无障碍操作都与左边那个相同，宽度也相同；不分离时隐藏（中缝由 installSplitGaps 插在两者之间）。
    let splitSpace = makeSpaceKey()
    splitSpace.accessibilityIdentifier = "splitSpaceKey"
    splitSpace.isHidden = true
    splitSpaceButton = splitSpace
    row.addArrangedSubview(splitSpace)
    let splitSpaceWidth = splitSpace.widthAnchor.constraint(equalTo: space.widthAnchor)
    splitSpaceWidth.priority = .init(999)
    splitSpaceWidth.isActive = true
    // 手机 26 键底行空格键旁边的 。；其他底行上隐藏。
    let period = makeKey(title: "。", accessibilityLabel: "符号 。") { [weak self] in
      self?.countKeyPress(TypingKeyID.character("."))
      self?.handleSymbol(".")
    }
    period.configuration?.contentInsets = .zero
    period.accessibilityIdentifier = "bottomPeriodKey"
    period.isHidden = true
    periodKey = period
    symbolKeyFaces.append((period, ".", Self.chineseSymbolFaces["."] ?? "."))
    row.addArrangedSubview(period)
    let language = makeKey(title: "中/英", accessibilityLabel: "切换中英文", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.language)
      self?.toggleInputMode()
    }
    language.configuration?.contentInsets = .zero
    language.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    language.accessibilityIdentifier = "bottomLanguageKey"
    language.isHidden = false
    bottomLanguageButton = language
    bottomLanguageWidth = language.widthAnchor.constraint(equalToConstant: 34)
    fullSymbolsWidth = nineKeySymbolsButton.widthAnchor.constraint(equalToConstant: 34)
    row.addArrangedSubview(language)

    // 九键外框的 0，位于设计稿的底行：网格的数字层放 1-9，这个键在两层上都留在原位。
    let zero = makeKey(title: "0", accessibilityLabel: "数字 0") { [weak self] in
      guard let self else { return }
      countKeyPress(nineKeyDigitPadShown ? TypingKeyID.character("0") : TypingKeyID.nineKey(0))
      handleSymbol("0")
    }
    zero.configuration?.contentInsets = .zero
    applyLetterFont(to: zero)
    zero.accessibilityIdentifier = "nineKeyZero"
    zero.isHidden = true
    nineKeyZeroButton = zero
    row.addArrangedSubview(zero)

    let enter = makeKey(title: "换行", accessibilityLabel: "换行", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.enter)
      self?.handleReturn()
    }
    enter.accessibilityIdentifier = "returnKey"
    enter.titleLabel?.adjustsFontSizeToFitWidth = true
    enter.titleLabel?.minimumScaleFactor = 0.65
    enterButton = enter
    row.addArrangedSubview(enter)

    // iPad 的 26 键底行：空格键之后是第二个 123 和收起键盘的 ⌄，占据原本回车的位置；回车移到第二排字母。
    let tabletLayer = makeKey(title: "123", accessibilityLabel: "切换到数字和符号", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.layer)
      self?.toggleLayout()
    }
    tabletLayer.configuration?.contentInsets = NSDirectionalEdgeInsets(top: 0, leading: 4, bottom: 0, trailing: 4)
    tabletLayer.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    tabletLayer.accessibilityIdentifier = "tabletLayerKey"
    tabletLayer.isHidden = true
    tabletLayerButton = tabletLayer
    row.addArrangedSubview(tabletLayer)
    let dismiss = makeKey(title: TabletLetterLayout.dismissFace, accessibilityLabel: "收起键盘", function: true) { [weak self] in
      self?.dismissKeyboard()
    }
    dismiss.configuration?.contentInsets = .zero
    dismiss.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    dismiss.accessibilityIdentifier = "dismissKeyboardKey"
    dismiss.isHidden = true
    dismissKeyButton = dismiss
    row.addArrangedSubview(dismiss)

    standardActionWidths = [
      layoutToggle.widthAnchor.constraint(equalToConstant: 48.4),
      space.widthAnchor.constraint(greaterThanOrEqualToConstant: 44),
      enter.widthAnchor.constraint(equalToConstant: 59.4),
    ]
    symbolDeleteWidth = delete.widthAnchor.constraint(equalToConstant: 44)
    // 每个权重都以空格键为基准，它是这一行里唯一总会显示的键。时有时无的键（地球键、， 和 。、大千符号层上的 ⌫、123 / #+= 层上的表情或符号键）略低于 required，隐藏时 stack view 的零宽约束胜出。
    func weight(_ key: UIView, _ value: CGFloat, space spaceWeight: CGFloat, optional: Bool = false) -> NSLayoutConstraint {
      let constraint = key.widthAnchor.constraint(equalTo: space.widthAnchor, multiplier: value / spaceWeight)
      if optional { constraint.priority = .init(999) }
      return constraint
    }
    nineKeyActionWidths = [
      weight(layoutToggle, 1.25, space: Self.nineKeySpaceWeight),
      weight(language, 1.05, space: Self.nineKeySpaceWeight),
      weight(globe, 1, space: Self.nineKeySpaceWeight, optional: true),
      weight(zero, 1.05, space: Self.nineKeySpaceWeight),
      weight(enter, 1.6, space: Self.nineKeySpaceWeight),
    ]
    layerActionWidths = [
      weight(layoutToggle, SymbolLayerLayout.lettersWeight, space: SymbolLayerLayout.spaceWeight),
      weight(emoji, SymbolLayerLayout.panelWeight, space: SymbolLayerLayout.spaceWeight, optional: true),
      weight(nineKeySymbolsButton, SymbolLayerLayout.panelWeight, space: SymbolLayerLayout.spaceWeight, optional: true),
      weight(globe, SymbolLayerLayout.globeWeight, space: SymbolLayerLayout.spaceWeight, optional: true),
      weight(enter, SymbolLayerLayout.returnWeight, space: SymbolLayerLayout.spaceWeight),
    ]
    let phoneSpace = Self.phoneSpaceWeight
    phoneActionWidths = [
      weight(layoutToggle, 1.25, space: phoneSpace),
      weight(language, 1.05, space: phoneSpace),
      weight(globe, 1, space: phoneSpace, optional: true),
      weight(delete, 1, space: phoneSpace, optional: true),
      weight(punctuation, 1, space: phoneSpace, optional: true),
      weight(period, 1, space: phoneSpace, optional: true),
      weight(enter, 1.9, space: phoneSpace),
    ]
    let tabletSpace = TabletLetterLayout.spaceWeight
    tabletActionWidths = [
      weight(layoutToggle, TabletLetterLayout.layerWeight, space: tabletSpace),
      weight(language, TabletLetterLayout.languageWeight, space: tabletSpace),
      weight(globe, TabletLetterLayout.globeWeight, space: tabletSpace, optional: true),
      weight(tabletLayer, TabletLetterLayout.layerWeight, space: tabletSpace),
      weight(dismiss, TabletLetterLayout.dismissWeight, space: tabletSpace),
    ]
    actionGlobeButton = globe
    return row
  }

  /// 手机 26 键底行上空格键的弹性权重；其他键的权重都相对它来算。
  static let phoneSpaceWeight: CGFloat = 4
  /// 九键外框底行（123 | space | 0 | 中 | return）上空格键的弹性权重。
  static let nineKeySpaceWeight: CGFloat = 4.2
  /// 123 层表情键上的表情线框图标。
  private static let layerEmojiIconSide: CGFloat = 22

  /// 按键没有自己设定时的键面字体：title3 文字样式，皮肤要求时用等宽字体。
  private static let keyLabelTransformer = UIConfigurationTextAttributesTransformer { attributes in
    var attributes = attributes
    attributes.font = KeyboardTheme.current.usesMonospacedFont
      ? .monospacedSystemFont(ofSize: UIFont.preferredFont(forTextStyle: .title3).pointSize, weight: .medium)
      : .preferredFont(forTextStyle: .title3)
    return attributes
  }

  /// 123 和 中 / 英：15pt medium，与字母一样固定字号。
  private static let functionLabelTransformer = UIConfigurationTextAttributesTransformer { attributes in
    var attributes = attributes
    attributes.font = .systemFont(ofSize: 15, weight: .medium)
    return attributes
  }

  /// 底部那一排的空格键：轻点输入空格或选词，左右滑动移动光标（`handleSpacePan`）。分离式键盘的两个空格都由这里做出来。
  private func makeSpaceKey() -> UIButton {
    let space = makeKey(title: "空格", accessibilityLabel: "空格") { [weak self] in
      self?.countKeyPress(TypingKeyID.space)
      self?.handleSpace()
    }
    space.accessibilityHint = Self.spaceAccessibilityHint(voice: spaceVoiceEnabled)
    space.accessibilityCustomActions = spaceAccessibilityActions()
    let pan = UIPanGestureRecognizer(target: self, action: #selector(handleSpacePan(_:)))
    pan.name = "spaceCursorPan"
    pan.maximumNumberOfTouches = 1
    pan.cancelsTouchesInView = true
    pan.delegate = self
    space.addGestureRecognizer(pan)
    // 对应 Android 的 `SpaceGesturePolicy`：按住不动 450 ms 打开语音，先拖动超过 10pt 则改为移动光标。哪个先识别出来就算哪个，UIKit 只让其中一个识别成功；长按会取消这次触摸，所以不会输入空格。
    let hold = UILongPressGestureRecognizer(target: self, action: #selector(handleSpaceVoiceHold(_:)))
    hold.name = "spaceVoiceHold"
    hold.minimumPressDuration = Self.spaceVoiceHoldDuration
    hold.allowableMovement = Self.spaceVoiceHoldMovement
    hold.cancelsTouchesInView = true
    hold.delegate = self
    space.addGestureRecognizer(hold)
    return space
  }

  /// 对应 Android 的 `SpaceGesturePolicy.LONG_PRESS_MS` 和 `DRAG_THRESHOLD_DP`。
  static let spaceVoiceHoldDuration: TimeInterval = 0.45
  static let spaceVoiceHoldMovement: CGFloat = 10

  /// 按住空格是否打开语音，见 `armsSpaceVoice`。
  private var spaceVoiceArmed: Bool {
    Self.armsSpaceVoice(enabled: spaceVoiceEnabled, fullAccess: hasFullAccess,
                        handwritingInk: !handwriting.isHidden && handwriting.hasInk,
                        composing: hasComposition, localMode: isInLocalMode)
  }

  /// 语音结果此刻能不能插入，同 Android 的 `voiceInsertionReady`：组字中或本地输入模式里不能。
  private var voiceInsertionReady: Bool {
    !hasComposition && !isInLocalMode
  }

  /// 按住空格是否打开语音：「长按空格语音输入」开着（Android 的 `platform.android.space_voice`），并且语音真能打开时才武装。没有完全访问权限时语音面板打不开，手写板上有笔迹时空格要上屏第一个识别结果，组字中或本地输入模式里与 Android 的 `voiceInsertionReady` 一样不武装；这些情况下这次按压保持普通空格和拖动移光标，慢一点的空格不会被吞掉。`localMode` 放在最后按需求值，`isInLocalMode` 要走一次完整的 C ABI 往返。
  static func armsSpaceVoice(enabled: Bool, fullAccess: Bool, handwritingInk: Bool, composing: Bool,
                             localMode: @autoclosure () -> Bool) -> Bool {
    enabled && fullAccess && !handwritingInk && !composing && !localMode()
  }

  /// 空格键的 VoiceOver 操作。关掉长按空格语音后不再提供「语音输入」；提供时它是用户明确的请求，所以不看完全访问权限，没有权限时由 `showVoicePanel` 说明原因。
  private func spaceAccessibilityActions() -> [UIAccessibilityCustomAction] {
    var actions = [
      UIAccessibilityCustomAction(name: "光标左移") { [weak self] _ in self?.moveCursor(by: -1); return self != nil },
      UIAccessibilityCustomAction(name: "光标右移") { [weak self] _ in self?.moveCursor(by: 1); return self != nil },
    ]
    if spaceVoiceEnabled {
      actions.append(UIAccessibilityCustomAction(name: "语音输入") { [weak self] _ in
        guard let self, voiceInsertionReady else { return false }
        showVoicePanel()
        return true
      })
    }
    return actions
  }

  /// 底部那一排的两个空格键；第二个只在分离式键盘里显示，标题随第一个一起更新。
  private var spaceKeys: [UIButton] {
    [spaceButton, splitSpaceButton].compactMap { $0 }
  }

  private func makeDeleteKey() -> UIButton {
    let delete = makeIconKey(Self.backspaceIcon, accessibilityLabel: "删除")
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

  /// A stroke key: its letter goes to the Stroke editor, which appends the stroke and looks the characters up. The wildcard only extends a composition (the Engine leaves an idle `x` unhandled, and the keypad greys it out then), so with nothing composing it does nothing rather than type a letter. Every stroke key the Engine takes is handled, so an unhandled answer is never typed into the document as a letter either.
  private func handleStrokeKey(_ key: String) {
    playInputClick()
    guard isChineseMode else { return }
    synchronizeInputSchemePreference()
    guard typesStroke, StrokeKeyLayout.accepts(key, composing: hasComposition) else { return }
    render(session.handleCharacter(key))
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

    // Zhuyin spells with the digit row and with , . / ; -, so the symbol panel cannot hand its keys to the Engine: they would type ㄅ or ㄝ or pick a list row. The panel commits the conversion and types the mark itself, as the key draws it: the Chinese face for an ASCII mark that has one while Chinese punctuation is on, the ASCII mark otherwise.
    if typesZhuyin {
      render(session.finishComposition())
      insertDirectText(Self.zhuyinSymbolText(symbol, chinesePunctuation: zhuyinWritesChinesePunctuation))
      return
    }

    // 组字中或本地模式里 Engine 列为拼写的符号是输入，要在数字选候选和标点路由之前作为字符交给会话：U 模式的十六进制数字、网址模式的数字和网址符号、`www` 之后的 `.`。注音在上面单独处理，面板上的键对它不是注音键。
    // 会话不收时（例如粤拼紧跟在 `'` 之后的第二个 `'`）以未处理返回、不带上屏，继续走下面的标点路由，与藏文和 `'` 分支一致。
    if session.engineSpellsWhileComposing(symbol) {
      let spellingSnapshot = session.handleCharacter(symbol)
      if spellingSnapshot.isHandled || spellingSnapshot.commitText?.isEmpty == false {
        render(spellingSnapshot)
        return
      }
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
      noteOwnEdit()
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
    noteOwnEdit()
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
    // 双击只看两次手动点按的间隔，不看第一下之后是什么状态：句首自动大写时第一下关掉大写，第二下照样锁定。锁定时点一下回到小写并清掉计时，紧接着的一下不会又锁上。
    if letterCaseState == .capsLock {
      letterCaseState = .lowercase
      lastShiftTapTime = nil
    } else if let lastShiftTapTime, now - lastShiftTapTime <= 0.35 {
      letterCaseState = .capsLock
      self.lastShiftTapTime = nil
    } else {
      letterCaseState = letterCaseState == .lowercase ? .shifted : .lowercase
      lastShiftTapTime = now
    }
    updateLetterCaseControls()
  }

  private func updateAutomaticCapitalization() {
    // 越南语用拉丁字母书写，所以单词与英文一样跟随输入框的自动大写。藏文不跟随：威利转写的大写字母是另一种拼写，自动大写会把句首的 `ka` 变成 `Ka`。
    guard !isChineseMode || typesVietnamese else {
      isAutomaticShift = false
      updateLetterCaseControls()
      return
    }
    // 手动按下的单次大写留到下一个字母用掉为止，与 Android、HarmonyOS 一致；否则按下 Shift 后文字一变就会把它冲掉。
    guard letterCaseState != .capsLock, letterCaseState != .shifted || isAutomaticShift else {
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
    // 不清点按计时：两次点按之间文字变化触发一次重新计算，不能让双击失效。
    updateLetterCaseControls()
  }

  private func updateLetterCaseControls() {
    // 设计稿在所有模式下都画小写键面；只有下一键真的会输入大写时键面才变大写：英文的 Shift 或大写锁定，组字中的辅助码 Shift（无障碍标签里也会体现，并以大写辅助码交给 Engine），以及越南语和藏文区分大小写的字母。
    let casedLetters = typesCasedLetters
    let shifted = letterCaseState != .lowercase && (!isChineseMode || entersHelpcode || casedLetters)
    // 只读一次，不要每个键读一次。`isInLocalMode` 看起来像属性，其实是一次完整的 C ABI 往返：Rust 把整个视图（预编辑、每个候选及其编码和释义）序列化成 JSON，Swift 再解析回来。放在下面的循环里调用，每次按键都会做二十七遍。
    let inLocalMode = isInLocalMode
    let usesUppercase = shifted
    let korean = typesKorean
    let koreanShifted = korean && letterCaseState != .lowercase
    for (button, lowercase, hintLabel, cornerLabel) in letterButtons {
      // 角标符号属于所有模式和方案下的 QWERTY 字母键面，与 Android 的 `standardLetters` 一致；韩文字母键面没有，微软双拼的 `;` 也没有。
      let corner = korean ? nil : LetterHintTable.hint(for: lowercase)
      cornerLabel.text = corner
      cornerLabel.isHidden = corner == nil
      (button as? KeyboardKeyButton)?.cornerHint = corner
      (button as? KeyboardKeyButton)?.swipesCornerHint = swipeSymbolsEnabled
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
      // 用户在设置里关掉「双拼键位提示」时也不画；下边距和读屏的 accessibilityValue 都跟着 `hint == nil` 收回。
      let hint = isChineseMode && !inLocalMode && shuangpinKeyHintsEnabled
        ? shuangpinKeyHints[lowercase.uppercased()] : nil
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

    // iPad 键盘第三排两端各有一个 ⇧，两者显示同样的状态。
    for button in [shiftButton, rightShiftButton].compactMap({ $0 }) {
      switch letterCaseState {
      case .lowercase:
        button.accessibilityLabel = korean ? "双辅音" : isChineseMode && !casedLetters ? "切换到英文大写" : "大写"
        button.accessibilityValue = "关闭"
      case .shifted:
        button.accessibilityLabel = korean ? "双辅音" : "大写"
        button.accessibilityValue = isAutomaticShift ? "自动开启" : "下一字母"
      case .capsLock:
        button.accessibilityLabel = korean ? "双辅音锁定" : "大写锁定"
        button.accessibilityValue = "开启"
      }
    }
    styleShiftKeys()
  }

  /// 设计稿的 ⇧ 及其带下划线的大写锁定样式，画法与 Android 的 `styleShiftKey` 一致：关闭时用功能键底色、图标为按键颜色；打开时（单次或锁定）用字母键底色、图标为强调色。皮肤处理会给所有功能键套上关闭时的颜色；它调用的 `updateSchemeButton` 再经 `updateLetterCaseControls` 重新设置这两个键的样式。
  private func styleShiftKeys() {
    let skin = KeyboardTheme.current
    let on = letterCaseState != .lowercase
    for button in [shiftButton, rightShiftButton].compactMap({ $0 }) {
      guard var configuration = button.configuration else { continue }
      configuration.image = letterCaseState == .capsLock ? Self.capsLockIcon : Self.shiftIcon
      configuration.background.backgroundColor = on ? skin.keyBackground : skin.functionKeyBackground
      configuration.baseForegroundColor = on ? skin.accent : skin.keyForeground
      button.configuration = configuration
      decorateKey(button)
    }
  }

  /// 设计稿的按键图标为 22pt（dc.html 里 shift、caps lock 和 backspace 的路径），与 Android 的 `KeyboardIconKey` 一致。
  private static let functionIconSide: CGFloat = 22
  private static let shiftIcon = KeyboardIcon.shift.image(pointSize: functionIconSide)
  private static let capsLockIcon = KeyboardIcon.capsLock.image(pointSize: functionIconSide)
  private static let backspaceIcon = KeyboardIcon.backspace.image(pointSize: functionIconSide)

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
    configuration.titleTextAttributesTransformer = Self.functionLabelTransformer
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

  /// The face of the 中/英 key while the scheme is on: the language it writes, with Cantonese, Zhuyin and Stroke named apart from Mandarin. Not private: the scheme tests pin it.
  static func languageKeyTitle(_ scheme: ChineseInputScheme) -> String {
    if scheme.isJapanese { return "日" }
    switch scheme {
    case .korean: return "한"
    case .cantonese: return "粤"
    case .zhuyin: return "注"
    case .vietnamese: return "越"
    case .tibetan: return "藏"
    case .stroke: return "笔"
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
    case .stroke: return "笔画输入"
    default: return "中文输入"
    }
  }

  /// 空格键画的内容：用皮肤次要文字颜色显示的标签，`mic` 不为 false 时前面带麦克风线框图标。
  struct SpaceKeyFace: Equatable {
    let label: String
    let mic: Bool
  }

  /// 设计稿的空格键面：中文下是麦克风和方案简称，英文下是 `space`，符号层上是空格；手写板与其他方案一样显示方案名（手写），与 Android 的 SpaceKeyFace 一致。日语按按键的作用命名，空闲时是空白，选候选时是変換。不是 private：布局测试会固定它。
  static func spaceKeyFace(scheme: ChineseInputScheme, chinese: Bool, symbols: Bool, composing: Bool) -> SpaceKeyFace {
    if chinese && scheme.isJapanese { return SpaceKeyFace(label: composing ? "変換" : "空白", mic: false) }
    if symbols { return SpaceKeyFace(label: "空格", mic: true) }
    if !chinese { return SpaceKeyFace(label: "space", mic: true) }
    return SpaceKeyFace(label: scheme.shortLabel, mic: true)
  }

  private func updateSpaceKeyTitle() {
    let japaneseTitle = hasComposition ? "変換" : "空白"
    if var configuration = japaneseSpaceButton?.configuration,
       configuration.title != japaneseTitle {
      configuration.title = japaneseTitle
      japaneseSpaceButton?.configuration = configuration
      japaneseSpaceButton?.accessibilityLabel = japaneseTitle
    }
    let face = Self.spaceKeyFace(scheme: inputScheme, chinese: isChineseMode, symbols: showsSymbols, composing: hasComposition)
    let japanese = isChineseMode && inputScheme.isJapanese
    let color = KeyboardTheme.current.secondary
    for space in spaceKeys {
      guard var configuration = space.configuration else { continue }
      let hasMic = configuration.image != nil
      guard configuration.title != face.label || hasMic != face.mic || configuration.baseForegroundColor != color else { continue }
      configuration.title = face.label
      configuration.image = face.mic ? KeyboardIcon.mic.image(pointSize: Self.spaceMicSide) : nil
      configuration.imagePlacement = .leading
      configuration.imagePadding = 5
      configuration.baseForegroundColor = color
      configuration.titleTextAttributesTransformer = Self.spaceLabelTransformer
      space.configuration = configuration
      // 键面只是提示；VoiceOver 按按键本身命名，日语除外，日语里按键的名字就是它的动作。
      space.accessibilityLabel = japanese ? face.label : "空格"
    }
    japaneseKeys?.setComposing(hasComposition)
  }

  /// 空格键上的麦克风线框图标，尺寸取 Android 的 `SpaceKeyFace.MIC_DP`。
  private static let spaceMicSide: CGFloat = 22

  /// 空格键的标签：13pt，与字母一样固定字号。
  private static let spaceLabelTransformer = UIConfigurationTextAttributesTransformer { attributes in
    var attributes = attributes
    attributes.font = .systemFont(ofSize: 13)
    return attributes
  }

  private func updateReturnKey() {
    let title: String
    let returnKeyType = textDocumentProxy.returnKeyType ?? .default
    switch returnKeyType {
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
    // 设计稿把普通回车画成回车图标；输入框指明了动作的保留文字，那是宿主要求这个键显示的。
    let image = !confirms && returnKeyType == .default ? Self.returnIcon : nil
    let face = image == nil ? shownTitle : nil
    for enter in returnKeys {
      // 每次宿主文字变化都会走到这里，键面没变就不重写配置，免得每次按键都给两个回车键重新布局。确认字样只在组字时出现，所以标题和图标相同时字体变换也相同。
      if var configuration = enter.configuration, configuration.title != face || configuration.image !== image {
        configuration.title = face
        configuration.image = image
        configuration.titleTextAttributesTransformer = confirms ? Self.returnConfirmTransformer : Self.returnTitleTransformer
        enter.configuration = configuration
      }
      if enter.accessibilityLabel != shownTitle { enter.accessibilityLabel = shownTitle }
    }
    styleReturnKey()
  }

  private static let returnIconSide: CGFloat = 22
  /// 回车图标只画一次，与 `shiftIcon`、`backspaceIcon` 一样；模板图，颜色跟按键前景色走。
  private static let returnIcon = KeyboardIcon.returnKey.image(pointSize: returnIconSide)

  /// 组字中显示的确认：15pt semibold。
  private static let returnConfirmTransformer = UIConfigurationTextAttributesTransformer { attributes in
    var attributes = attributes
    attributes.font = .systemFont(ofSize: 15, weight: .semibold)
    return attributes
  }

  /// 输入框的动作词（发送、搜索……），用功能键的字号。
  private static let returnTitleTransformer = UIConfigurationTextAttributesTransformer { attributes in
    var attributes = attributes
    attributes.font = .systemFont(ofSize: 15, weight: .medium)
    return attributes
  }

  /// Whether Return commits the open composition and then still does the field's action: Korean and Vietnamese, whose runtime answers the commit unhandled.
  private var returnKeepsFieldAction: Bool {
    isChineseMode && (inputScheme.isKorean || inputScheme.isVietnamese)
  }

  /// 回车始终是强调键：无论空闲还是组字中，都用皮肤的动作底色和动作标签，与设计稿一致。键盘设计通过自己的按键表面画动作颜色（`applyKeyboardSkin`）。
  private func styleReturnKey() {
    let skin = KeyboardTheme.current
    guard skin.design == nil else { return }
    let background = skin.actionBackground
    let foreground = skin.actionForeground
    for enter in returnKeys {
      guard var configuration = enter.configuration,
            configuration.background.backgroundColor != background || configuration.baseForegroundColor != foreground
      else { continue }
      configuration.background.backgroundColor = background
      configuration.baseForegroundColor = foreground
      enter.configuration = configuration
    }
  }

  /// 底行的回车和 iPad 第二排字母上的回车（`TabletLetterLayout`）；同一时间只显示其中一个，两者键面相同。
  private var returnKeys: [UIButton] {
    [enterButton, tabletReturnKey].compactMap { $0 }
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
    // 隐私模式开着的整段时间里词库不从输入内容学习，不管学习开关怎么设。会话同时标成隐私会话（Android 的 `markPrivateSession`），运行时不记选词位置和上屏效率。
    _ = session.setLearningEnabled(DictionaryLearningPreference.enabled && !incognito)
    session.setPrivateSession(privacyGate.privateSession)
    let fuzzyRules = FuzzyPinyinPreference.settings(in: session.sharedPreferences)?.bits ?? 0
    if session.fuzzyPinyinRulesApplied != fuzzyRules {
      _ = session.setFuzzyPinyinRules(fuzzyRules)
    }
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

  private var currentTopRowHeight: CGFloat {
    Self.topRowHeight(
      glossLines: glossLineCount, candidateScale: candidateFontScale, preeditScale: preeditFontScale)
  }

  /// 实际画出的顶栏高度：收起时为零。
  private var shownTopRowHeight: CGFloat { topRowCollapsed ? 0 : currentTopRowHeight }

  private func setTopRowCollapsed(_ collapsed: Bool) {
    guard collapsed != topRowCollapsed else { return }
    topRowCollapsed = collapsed
    topRowHeightConstraint?.constant = shownTopRowHeight
    applyKeyboardMetrics()
    updatePreferredKeyboardHeight()
  }

  /// 读本机的工具栏设置；工具栏和顶栏在下一次 `updateShortcutButtons` 和 `renderCandidateStrip` 时跟着更新。
  private func readToolbarPreferences() {
    toolbarPhrases = TouchToolbarLocalPreference.phrases
    toolbarScheme = TouchToolbarLocalPreference.scheme
    toolbarHidden = TouchToolbarLocalPreference.hidden
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
      preeditButton.configuration?.titleTextAttributesTransformer = Self.preeditTransformer(scale: preeditScale)
    }
    candidateFontScale = candidateScale
    topRowHeightConstraint?.constant = shownTopRowHeight
    readingRowHeightConstraint?.constant = Self.readingRowHeight(preeditScale: preeditScale)
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

  /// 读音行上的拼写：设计稿的 12pt 乘以同步的编码字号比例，字距按设计稿的 .02em。
  static func preeditFont(scale: CGFloat) -> UIFont {
    .systemFont(ofSize: (preeditFontSize * scale).rounded())
  }

  private static func preeditTransformer(scale: CGFloat) -> UIConfigurationTextAttributesTransformer {
    UIConfigurationTextAttributesTransformer { attributes in
      var attributes = attributes
      let font = preeditFont(scale: scale)
      attributes.font = font
      attributes.uiKit.kern = font.pointSize * 0.02
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
    case .stroke: session.switchToStroke()
    case .handwriting: session.switchToHandwriting()
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
    // 不是模态的：上方的工具栏仍可操作，它的品牌键会关掉面板。
    panel.view.accessibilityViewIsModal = false
    panel.view.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(panel.view)
    NSLayoutConstraint.activate([
      panel.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      panel.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      panel.view.topAnchor.constraint(equalTo: compositionContainer?.bottomAnchor ?? view.topAnchor),
      panel.view.bottomAnchor.constraint(equalTo: view.bottomAnchor)
    ])
    panel.didMove(toParent: self)
    // 回复面板盖住按键期间隐藏按键行，VoiceOver 和触摸都不会落到面板下面的按键上。
    setKeyRowsCovered(true)
    UIAccessibility.post(notification: .layoutChanged, argument: panel.view)
    updateShortcutButtons()
  }

  private func dismissReplyPanel() {
    guard let panel = replyPanel else { return }
    panel.willMove(toParent: nil)
    panel.view.removeFromSuperview()
    panel.removeFromParent()
    replyPanel = nil
    // 键区面板会替换回复面板，它自己会再次盖住按键；只有没有键区面板接手时才恢复按键行。
    if keyAreaPanel == nil { setKeyRowsCovered(false) }
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

  @objc private func handleSpaceVoiceHold(_ hold: UILongPressGestureRecognizer) {
    guard hold.state == .began else { return }
    // 与 Android 一样，打开语音的那次按压也计为一次空格键按键。
    countKeyPress(TypingKeyID.space)
    playInputClick()
    showVoicePanel()
  }

  /// 键区里的语音面板。键盘扩展不能录音，所以面板是进入语音交接的入口，「语音结果」读的也是同一份交接：点圆球打开 app 的录音界面，有待插入的结果时则插入 app 传回的结果。点圆球时会核对结果与当前编辑器是否对得上，与 `showKeyboardVoice` 插入前的检查一样。
  private func showVoicePanel() {
    guard hasFullAccess else {
      showDiagnostic("语音输入需要开启键盘的“允许完全访问”。")
      renderCandidateStrip()
      return
    }
    guard voiceInsertionReady else { return }
    let store = VoiceTextHandoffStore()
    let entry: VoiceTextHandoff?
    do { entry = try store.read() } catch {
      showDiagnostic(error.localizedDescription)
      renderCandidateStrip()
      return
    }
    let context = KeyboardHostContext.documentIdentifier(for: textDocumentProxy).map {
      KeyboardDocumentContext(document: $0, before: textDocumentProxy.documentContextBeforeInput,
                              selected: textDocumentProxy.selectedText, after: textDocumentProxy.documentContextAfterInput)
    }
    closeKeyboardService()
    closeKeyboardPicker()
    let panel = KeyboardVoicePanelView(entry: entry, skin: KeyboardTheme.current)
    panel.onCancel = { [weak self] in self?.closeKeyboardPicker() }
    panel.onOrb = { [weak self] in
      guard let self else { return }
      closeKeyboardPicker()
      guard let entry else {
        KeyboardAppLauncher.open(KeyboardAppLauncher.voiceURL, from: self)
        return
      }
      insertVoiceHandoff(entry, store: store, context: context)
    }
    voicePanel = panel
    installPanel(panel)
  }

  /// 如果编辑器仍停在面板打开时的位置，就插入等待中的语音结果，并把它从交接里清掉。
  private func insertVoiceHandoff(_ entry: VoiceTextHandoff, store: VoiceTextHandoffStore, context: KeyboardDocumentContext?) {
    guard hasFullAccess, let context,
          context.matches(document: KeyboardHostContext.documentIdentifier(for: textDocumentProxy),
                          before: textDocumentProxy.documentContextBeforeInput, selected: textDocumentProxy.selectedText,
                          after: textDocumentProxy.documentContextAfterInput) else {
      showDiagnostic("输入位置已变化，请重新长按空格打开语音输入。")
      renderCandidateStrip()
      return
    }
    do {
      let text = try store.consume(entry.id)
      insertOwnText(text, source: .voice)
    } catch {
      showDiagnostic(error.localizedDescription)
      renderCandidateStrip()
    }
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
  private func synchronizeSharedTouchPreferences() -> Bool {
    guard let preferences = session.sharedPreferences else { return false }
    let previousTranslationSettings = InputHabitPreference.mirrored
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
    let nextTranslationSettings = InputHabitPreference.settings(in: preferences,
                                                                 fallback: previousTranslationSettings)
    let translationSettingsChanged = InputHabitPreference.translationDisplaySettingsChanged(
      previousTranslationSettings, nextTranslationSettings)
    let languageChanged = previousTranslationSettings.primaryLanguage != nextTranslationSettings.primaryLanguage
      || previousTranslationSettings.secondaryLanguage != nextTranslationSettings.secondaryLanguage
    if languageChanged {
      candidateGlossEpoch &+= 1
      candidateGlossRequestedGeneration = nil
      candidateTargetGlosses = [:]
    }
    GlobalThemePreference.mirror(preferences)
    let previousTheme = KeyboardTheme.current
    let skinChanged = KeyboardTheme.reload(preferences) != previousTheme
    // 高度也抄进 App Group：原先只抄键距、行距和语音入口，高度只进了这个控制器，于是布局面板打开时显示、提交时写回的都是 App Group 里可能过时的高度。
    KeyboardLayoutPreference.mirrorGeometry(preferences)
    if let adjustment = KeyboardLayoutPreference.sharedHeightAdjustment(preferences["touch_keyboard_height_adjustment"]) {
      sharedKeyboardHeightAdjustment = CGFloat(adjustment)
    }
    KeyboardLayoutPreference.numberKeypadOrder = KeyboardLayoutPreference.NumberKeypadOrder.shared(in: preferences)
    KeyboardLayoutPreference.twentySixKeyNumberLayout =
      KeyboardLayoutPreference.TwentySixKeyNumberLayout.shared(in: preferences)
    // 键盘停在数字层时改了这两项，键面要马上换过来，不等下一次切层。
    let digitLayerChanged = numberKeypadOrder != KeyboardLayoutPreference.numberKeypadOrder
      || twentySixKeyNumberLayout != KeyboardLayoutPreference.twentySixKeyNumberLayout
    numberKeypadOrder = KeyboardLayoutPreference.numberKeypadOrder
    twentySixKeyNumberLayout = KeyboardLayoutPreference.twentySixKeyNumberLayout
    if digitLayerChanged && actionRow != nil { updateKeyboardLayout() }
    KeyboardLayoutPreference.shuangpinKeyHints = KeyboardLayoutPreference.sharedShuangpinKeyHints(in: preferences)
    if shuangpinKeyHintsEnabled != KeyboardLayoutPreference.shuangpinKeyHints {
      shuangpinKeyHintsEnabled = KeyboardLayoutPreference.shuangpinKeyHints
      updateLetterCaseControls()
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
    return translationSettingsChanged
  }

  static func sharedPreferenceInt(_ value: Any?, range: ClosedRange<Int> = 1...6) -> Int? {
    guard let integer = SharedNumber.strictInt(value),
          range.contains(integer) else { return nil }
    return integer
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

  /// 候选行的展开控件：一条 1×22 细线，后面是按键前景色的 40pt 箭头，候选网格打开时箭头翻转。
  private func configureExpandButton() {
    var configuration = UIButton.Configuration.plain()
    configuration.image = KeyboardIcon.candidateExpand.image(pointSize: Self.expandIconSide)
    configuration.baseForegroundColor = KeyboardTheme.current.keyForeground
    configuration.contentInsets = .zero
    expandCandidatesButton.configuration = configuration
    expandCandidatesButton.accessibilityLabel = "展开全部候选"
    expandCandidatesButton.accessibilityIdentifier = "expandCandidates"
    expandCandidatesButton.isHidden = true
    let side = Self.expandButtonSide
    let height = expandCandidatesButton.heightAnchor.constraint(equalToConstant: side)
    // 低于 required：候选行可能比按钮稍矮，以行高为准。
    height.priority = .init(999)
    NSLayoutConstraint.activate([expandCandidatesButton.widthAnchor.constraint(equalToConstant: side), height])
    expandDivider.isUserInteractionEnabled = false
    expandDivider.isAccessibilityElement = false
    expandDivider.accessibilityIdentifier = "expandCandidatesDivider"
    expandDivider.backgroundColor = KeyboardTheme.current.hairline
    expandDivider.isHidden = true
    NSLayoutConstraint.activate([
      expandDivider.widthAnchor.constraint(equalToConstant: 1),
      expandDivider.heightAnchor.constraint(equalToConstant: 22),
    ])
  }

  static let expandButtonSide: CGFloat = 40
  private static let expandIconSide: CGFloat = 20

  /// 展开箭头：打开候选网格，已打开时关掉。
  private func toggleCandidatePanel() {
    if candidatePanel != nil {
      closeKeyboardPicker()
    } else {
      showCandidatePanel()
    }
  }

  /// 候选网格关闭时箭头朝下，打开时朝上，翻转用时 .2s。
  private func updateExpandRotation(animated: Bool) {
    let transform: CGAffineTransform = candidatePanel != nil ? CGAffineTransform(rotationAngle: .pi) : .identity
    expandCandidatesButton.accessibilityLabel = candidatePanel != nil ? "收起候选" : "展开全部候选"
    guard expandCandidatesButton.transform != transform else { return }
    guard animated, expandCandidatesButton.window != nil, !UIAccessibility.isReduceMotionEnabled else {
      expandCandidatesButton.transform = transform
      return
    }
    UIView.animate(withDuration: 0.2, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState]) {
      self.expandCandidatesButton.transform = transform
    }
  }

  private func showCandidatePanel() {
    closeKeyboardService()
    closeKeyboardPicker()
    playInputClick()
    do {
      let snapshot = try CandidatePanelSnapshot.decode(session.allCandidates())
      guard !snapshot.entries.isEmpty else { return }
      let columns = opensNineKeyPanel ? makeNineKeyPanelColumns() : nil
      let panel = KeyboardCandidatePanelView(
        candidates: snapshot.entries.map(\.text), preedit: snapshot.preedit,
        annotations: candidatePanelAnnotations(snapshot),
        markers: candidatePanelMarkers(snapshot),
        candidateScale: candidateFontScale, preeditScale: preeditFontScale, candidateFamilies: candidateFontFamilies,
        showsHeader: false,
        display: { [weak self] in self?.chineseOutput($0) ?? $0 },
        menuElements: { [weak self] index in
          guard let self, let snapshot = candidatePanelSnapshot, snapshot.entries.indices.contains(index) else { return [] }
          let entry = snapshot.entries[index]
          return candidateMenuElements(
            generation: snapshot.generation, globalIndex: entry.index, candidate: entry.text,
            offlineGloss: entry.translation, fixedPosition: entry.fixedPosition)
        },
        onSelect: { [weak self] index in
          guard let self, let snapshot = candidatePanelSnapshot, snapshot.entries.indices.contains(index) else { return }
          // 先收起面板、暂不清九键筛选：清筛选会换一代候选，这次选择就成了过期的。选完组字还在的话再清。
          closeCandidatePanel(clearingNineKeyFilter: false)
          closeKeyboardPicker()
          playInputClick()
          render(session.selectAnyCandidate(generation: snapshot.generation, globalIndex: snapshot.entries[index].index))
          clearNineKeyFilter()
        },
        onClose: { [weak self] in self?.closeKeyboardPicker() },
        // ⌫ 关闭候选网格并删除，与 Android 的底栏一致：网格列出的是正在编辑的组字的候选。九键三栏面板不画底栏，退格在右栏里。
        onBackspace: { [weak self] in
          guard let self else { return }
          countKeyPress(TypingKeyID.backspace)
          closeKeyboardPicker()
          handleBackspace()
        },
        columns: columns.map { ($0.leading, $0.trailing) })
      candidatePanel = panel
      candidatePanelSnapshot = snapshot
      nineKeyPanelColumns = columns
      // 九键三栏面板和普通候选网格一样装在键区：上面的候选栏和读音留着，用户看得到选拼音、筛选之后读音和首选怎么变。
      installPanel(panel)
      updateExpandRotation(animated: true)
      updateNineKeyPanelColumns()
    } catch {
      showDiagnostic("候选列表暂不可用")
    }
  }

  /// 全拼九键正在组字时，展开的是三栏的九键面板；其他方案、注音九键和本地模式照旧是带「返回」和 ⌫ 底栏的候选网格。
  private var opensNineKeyPanel: Bool {
    inputScheme == .nineKey && isChineseMode && hasComposition && !isInLocalMode
  }

  private func candidatePanelAnnotations(_ snapshot: CandidatePanelSnapshot) -> [KeyboardCandidateAnnotation] {
    snapshot.entries.map {
      candidatePanelAnnotation(code: $0.code, gloss: $0.translation, word: $0.text, engine: $0.annotation,
                               typed: snapshot.preedit)
    }
  }

  private func candidatePanelMarkers(_ snapshot: CandidatePanelSnapshot) -> [[CandidateMarker]] {
    snapshot.entries.map { CandidateMarker.markers(source: $0.source, fixedPosition: $0.fixedPosition) }
  }

  private func makeNineKeyPanelColumns() -> KeyboardNineKeyPanelColumns {
    let columns = KeyboardNineKeyPanelColumns(makeKey: { [unowned self] title, label, action in
      makeKey(title: title, accessibilityLabel: label, function: true, action: action)
    })
    columns.onAction = { [weak self] action in self?.handleNineKeyPanelAction(action) }
    return columns
  }

  private func updateNineKeyPanelColumns() {
    nineKeyPanelColumns?.update(
      spellings: currentNineKeySpellings, strokeMode: nineKeyPanelStrokeMode, strokes: currentNineKeyStrokes,
      singleCharacter: currentNineKeySingleCharacter,
      // 不知道有没有词库时（测试宿主以外没有 EngineResources 的 bundle）照常提供，Engine 会报词库不可用。
      strokesAvailable: InputSchemePreference.installedLanguageSchemes?.contains(.stroke) ?? true)
  }

  /// 九键面板两栏里的按键。选拼音、退格、筛选都会让 Engine 换一代候选，`refreshCandidatePanelAnnotations` 按新的一代重建面板，面板不关。
  private func handleNineKeyPanelAction(_ action: KeyboardNineKeyPanelColumns.Action) {
    switch action {
    case .spelling(let index):
      playInputClick()
      render(session.chooseNineKeySpelling(at: UInt(index)))
    case .stroke(let key):
      playInputClick()
      setNineKeyFilter(singleCharacter: currentNineKeySingleCharacter, strokes: currentNineKeyStrokes + key)
    case .back:
      playInputClick()
      closeKeyboardPicker()
    case .delete:
      // 笔画模式下先删笔画，删完了才删拼音。
      if nineKeyPanelStrokeMode && !currentNineKeyStrokes.isEmpty {
        playInputClick()
        setNineKeyFilter(singleCharacter: currentNineKeySingleCharacter,
                         strokes: String(currentNineKeyStrokes.dropLast()))
      } else {
        handleBackspace()
      }
    case .clear:
      playInputClick()
      render(discardComposition())
    case .toggleStrokes:
      playInputClick()
      nineKeyPanelStrokeMode.toggle()
      // 回到拼音时把笔画筛选一起去掉，否则左栏已经看不见笔画，候选却还按它筛。
      if !nineKeyPanelStrokeMode && !currentNineKeyStrokes.isEmpty {
        setNineKeyFilter(singleCharacter: currentNineKeySingleCharacter, strokes: "")
      } else {
        updateNineKeyPanelColumns()
      }
    case .toggleSingleCharacter:
      playInputClick()
      setNineKeyFilter(singleCharacter: !currentNineKeySingleCharacter, strokes: currentNineKeyStrokes)
    }
  }

  private func setNineKeyFilter(singleCharacter: Bool, strokes: String) {
    let snapshot = session.setNineKeyFilter(singleCharacter: singleCharacter, strokes: strokes)
    render(snapshot)
    // Engine 用诊断码报笔画词库不可用，换成用户看得懂的话。
    if snapshot.diagnosticText?.contains("LANGUAGE_DICTIONARY_UNAVAILABLE") == true {
      showDiagnostic("笔画词库不可用")
      renderCandidateStrip()
    }
  }

  /// 组字还在、筛选还开着时把单字和笔画筛选都关掉：面板收起之后看不到筛选状态，候选不该还被筛着。
  private func clearNineKeyFilter() {
    guard hasComposition, currentNineKeySingleCharacter || !currentNineKeyStrokes.isEmpty else { return }
    render(session.setNineKeyFilter(singleCharacter: false, strokes: ""))
  }

  /// 键区里的按键行由 `closeKeyboardPicker` 末尾随 `keyAreaPanel` 一起恢复，这里只收面板本身和九键面板的状态。
  private func closeCandidatePanel(clearingNineKeyFilter: Bool = true) {
    guard let panel = candidatePanel else { return }
    panel.removeFromSuperview()
    candidatePanel = nil
    candidatePanelSnapshot = nil
    let wasNineKeyPanel = nineKeyPanelColumns != nil
    nineKeyPanelColumns = nil
    nineKeyPanelStrokeMode = false
    updateExpandRotation(animated: true)
    UIAccessibility.post(notification: .layoutChanged, argument: expandCandidatesButton)
    if wasNineKeyPanel && clearingNineKeyFilter { clearNineKeyFilter() }
  }

  private func updateExpandControl() {
    // 只要组字有候选就提供，与设计稿一致。网格列出会话的完整结果，所以会话不持有的列表不提供这个控件：英文联想和手写识别结果。
    let hidden = !hasComposition || visibleCandidates.isEmpty || visibleDiagnostic != nil
    expandCandidatesButton.isHidden = hidden
    expandDivider.isHidden = hidden
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

    // 输入方式画设计稿的键盘线框图标；方案本身作为按钮的值朗读，并在它打开的选择器里显示。
    schemeButton.apply(skin: KeyboardTheme.current)
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
    // 手机的中间一排两侧各缩进半个键，微软双拼用 `;` 键填上这块空位；iPad 的中间一排缩进 2.5% 开始，以回车结尾（`TabletLetterLayout`）。
    // 这个比例按按键区域本身的宽度计算，单手模式会把它收窄。
    let keysWidth = KeyboardOneHandLayout.keysWidth(
      available: view.bounds.width - formFactor.padding.leading - formFactor.padding.trailing, mode: appliedOneHanded ?? .off)
    let margins: UIEdgeInsets
    if formFactor == .tablet {
      margins = UIEdgeInsets(top: 0, left: keysWidth * TabletLetterLayout.middleRowLeadingInset, bottom: 0, right: 0)
    } else {
      let inset: CGFloat = KeyboardLayoutPreference.geometry.centeredLetters && inputScheme != .microsoft
        ? keysWidth * CGFloat(KeyboardLayoutPreference.geometry.letterInsetRatio) : 0
      margins = UIEdgeInsets(top: 0, left: inset, bottom: 0, right: inset)
    }
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
    keyColumn.spacing = layout.rowSpacing
    nineGrid?.spacing = layout.rowSpacing
    nineControls?.spacing = layout.rowSpacing
    handwritingColumns.forEach { $0.spacing = layout.rowSpacing }
    handwritingPad.spacing = layout.keySpacing
    strokeKeys?.applySpacing(row: layout.rowSpacing, key: layout.keySpacing)
    nineKeyHeight.constant = layout.rowSpacing * 2
    japaneseHeight?.constant = Self.japaneseKeyBlockHeight
    // 分几步拼出来：写成一个长的 + 表达式时，CI 上的编译器会在类型推断上超时。
    var keyRows: [UIView] = letterRowViews
    if let numberRowView { keyRows.append(numberRowView) }
    keyRows += zhuyinRowViews
    keyRows += symbolRowViews
    keyRows += symbolLayerRowViews as [UIView]
    keyRows += nineKeyRows
    for row in keyRows {
      (row as? UIStackView)?.spacing = layout.keySpacing
    }
    actionRow.spacing = layout.keySpacing
    for width in splitGapWidths { width.constant = -2 * layout.keySpacing }
    nineKeyContainer.spacing = layout.keySpacing
    if let old = nineSidebarWidth, let sidebar = old.firstItem as? UIView {
      old.isActive = false
      nineSidebarWidth = sidebar.widthAnchor.constraint(equalTo: nineKeyContainer.widthAnchor, multiplier: layout.sidebarRatio)
      nineSidebarWidth?.isActive = true
    }
    standardActionWidths[0].constant = 48.4
    standardActionWidths[1].constant = 44
    standardActionWidths[2].constant = 59.4
    quickPunctuationWidth?.constant = 44
    updateShortcutButtons()
    // 行距计入键盘高度，而不是从按键高度里扣出来。
    updatePreferredKeyboardHeight()
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
    let moreSymbols: Bool
    let globe: Bool
    let hasSpellings: Bool
    let geometry: KeyboardGeometry
    let formFactor: KeyboardFormFactor
    let fullKeys: Bool
    let split: Bool
  }

  private var layoutInputs: KeyboardLayoutInputs {
    KeyboardLayoutInputs(
      chinese: isChineseMode, scheme: inputScheme, localMode: currentLocalMode,
      symbols: showsSymbols, moreSymbols: showsMoreSymbols, globe: needsInputModeSwitchKey,
      hasSpellings: !currentNineKeySpellings.isEmpty,
      geometry: KeyboardLayoutPreference.geometry,
      formFactor: formFactor, fullKeys: KeyboardLayoutPreference.tabletFullKeys,
      split: wantsSplitKeyboard)
  }

  /// 键盘此刻是否横屏。有窗口场景时按它的界面方向；没有时（例如测试里还没进窗口的控制器）退回到竖直 size class：手机横屏是 compact，iPad 两个方向都是 regular，于是按竖屏处理，不会误画分离式键盘。测试用 `traitOverrides` 把它设成 compact 来模拟 iPad 横屏。
  private var isLandscape: Bool {
    view.window?.windowScene?.interfaceOrientation.isLandscape ?? (traitCollection.verticalSizeClass == .compact)
  }

  /// 此刻该不该画横屏分离式键盘：平板形态、横屏、开关打开，而且当前显示的是会分开的字母布局。开关放在最后读，手机和竖屏不读 App Group。
  private var wantsSplitKeyboard: Bool {
    KeyboardSplitLayout.splitsLayout(scheme: inputScheme, chinese: isChineseMode, localMode: isInLocalMode)
      && KeyboardSplitLayout.isActive(
        formFactor: formFactor, landscape: isLandscape, enabled: KeyboardLayoutPreference.tabletSplit)
  }

  /// 旋转、台前调度改窗口大小、停靠与浮动切换，或者键盘再次出现时读到新的开关值，都可能让分离与否变化；只有真的变了才重新布局。
  private func updateSplitKeyboardIfNeeded() {
    guard actionRow != nil, wantsSplitKeyboard != splitKeyboardShown else { return }
    updateKeyboardLayout()
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
    // 不在手机 26 键底行时，， 键是快捷标点键，键面是第一个快捷标点（`updateKeyboardLayout`）。
    // 日语的逗号写作 、，手机底行的逗号键就画 、，按下也发 、。
    let japaneseComma = writesJapaneseComma
    for face in symbolKeyFaces where face.key !== quickPunctuationButton || phoneBottomRowShown {
      let chinese = japaneseComma && face.key === quickPunctuationButton ? "、" : face.chinese
      let title = sendsChinesePunctuation ? chinese : face.ascii
      guard face.key.configuration?.title != title || face.key.accessibilityLabel != "符号 \(title)" else { continue }
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
    applyKeyboardMetrics()
    applyOneHanded()
    standardRowHeights.forEach { $0.1.isActive = false }
    microsoftFinalKey?.isHidden = !(isChineseMode && inputScheme == .microsoft && !isInLocalMode)
    let kana = isChineseMode && inputScheme == .japaneseNineKey && !isInLocalMode
    japaneseKeys?.isHidden = !kana
    japaneseKeys?.setDigits(showsSymbols)
    japaneseHeight?.constant = Self.japaneseKeyBlockHeight
    japaneseHeight?.isActive = kana
    japaneseKeys?.applyLayout()
    actionRow?.isHidden = kana
    japaneseGlobeButton?.isHidden = !needsInputModeSwitchKey
    japaneseKeys?.setModeColumnFull(needsInputModeSwitchKey)
    let nineKey = isChineseMode && inputScheme == .nineKey && !isInLocalMode
    let handwritingScheme = isChineseMode && inputScheme == .handwriting && !isInLocalMode
    let writes = handwritingScheme && !showsSymbols
    if !writes && !handwriting.isHidden { handwriting.deactivate() }
    handwriting.isHidden = !writes
    handwritingPad.isHidden = !writes
    if writes { handwriting.activate() }
    handwritingActionHeight?.isActive = writes
    // Dachen takes the digit row and four punctuation keys for bopomofo, so Zhuyin draws its own four rows in place of the letter rows and the number row.
    let dachen = isChineseMode && inputScheme.isZhuyin && !isInLocalMode
    zhuyinRowViews.forEach { $0.isHidden = !dachen || showsSymbols }
    // 笔画方案在九键外框里画它的 2×3 笔画键：沿用同样的标点侧栏、删除列和功能行。它的符号层与 Android 一样是设计稿的 123 / #+= 层，所以只有拼音网格保留自己的数字层。
    let strokes = isChineseMode && inputScheme.isStroke && !isInLocalMode
    let strokePad = strokes && !showsSymbols
    // 「26 键数字键盘」选了九宫格时，手机 26 键的 123 画成九键的数字层：同一个外框和同一组键，只有回到字母的键回到 26 键字母。
    let digitPad = showsSymbols && Self.opensNineKeyDigitPad(
      layout: twentySixKeyNumberLayout, formFactor: formFactor,
      letterKeys: !(nineKey || strokes || handwritingScheme || kana || dachen))
    nineKeyDigitPadShown = digitPad
    let nineKeyFrame = nineKey || strokePad || digitPad
    let symbolLayer = Self.drawsSymbolLayer(symbols: showsSymbols, nineKey: nineKey || digitPad, kana: kana, dachen: dachen)
    let letterRowsShown = !(showsSymbols || nineKeyFrame || writes || kana || dachen)
    letterRowViews.forEach { $0.isHidden = !letterRowsShown }
    applyTabletLetterKeys()
    let fullKeys = formFactor.canShowFullKeys && KeyboardLayoutPreference.tabletFullKeys
    numberRowView?.isHidden = !fullKeys || showsSymbols || nineKeyFrame || writes || kana || dachen
    tabKey?.isHidden = !fullKeys
    let rowPunctuation = formFactor.showsLetterRowPunctuation
    for key in letterRowPunctuationKeys where key.isHidden == rowPunctuation { key.isHidden = !rowPunctuation }
    // 横屏分离式键盘：每排的中缝和右半的第二个空格一起显示或隐藏。九键、笔画、假名、大千和手写不分（wantsSplitKeyboard），它们的符号页也跟着不分。
    let split = wantsSplitKeyboard
    splitKeyboardShown = split
    for gap in splitGaps where gap.isHidden == split { gap.isHidden = !split }
    if splitSpaceButton?.isHidden == split { splitSpaceButton?.isHidden = !split }
    // 九键数字层保留三列网格，只换键面文字。这样不会换成每排十键的符号行，也保留了用户选的布局。
    let nineKeyDigits = nineKey && showsSymbols || digitPad
    nineKeyContainer.isHidden = !nineKeyFrame
    nineGrid?.isHidden = strokePad
    strokeKeys?.isHidden = !strokePad
    nineKeyRows.forEach { $0.isHidden = !nineKeyFrame || strokePad }
    applyNineKeyDigitLayer(nineKeyDigits)
    updateNineKeyMiddleKey()
    let hasSpellings = !currentNineKeySpellings.isEmpty
    spellingScrollView.isHidden = !hasSpellings
    punctuationStack.isHidden = hasSpellings
    if actionRow != nil {
      let phoneRow = Self.usesPhoneBottomRow(formFactor: formFactor, nineKeyFrame: nineKeyFrame, handwriting: writes, kana: kana)
      let tabletRow = Self.usesTabletBottomRow(formFactor: formFactor, letterRows: letterRowsShown)
      let style: ActionRowStyle = symbolLayer ? .symbolLayer : nineKeyFrame ? .nineKey : tabletRow ? .tablet : phoneRow ? .phone : .standard
      phoneBottomRowShown = style == .phone
      nineKeyHeight.isActive = nineKeyFrame
      arrangeActionRow(style)
      NSLayoutConstraint.deactivate(standardActionWidths + nineKeyActionWidths + phoneActionWidths + layerActionWidths + tabletActionWidths)
      // iPad 的 26 键底行在回车的位置放第二个 123 和 ⌄，回车在 iPad 上排到第二排字母末尾。
      if tabletLayerButton.isHidden != (style != .tablet) { tabletLayerButton.isHidden = style != .tablet }
      if dismissKeyButton.isHidden != (style != .tablet) { dismissKeyButton.isHidden = style != .tablet }
      if enterButton?.isHidden != (style == .tablet) { enterButton?.isHidden = style == .tablet }
      symbolDeleteWidth?.isActive = false
      quickPunctuationWidth?.isActive = false
      globeWidthConstraint?.isActive = false
      bottomLanguageWidth?.isActive = false
      actionGlobeButton.isHidden = !needsInputModeSwitchKey
      if needsInputModeSwitchKey && style == .standard {
        globeWidthConstraint = actionGlobeButton.widthAnchor.constraint(equalToConstant: 44)
        globeWidthConstraint?.isActive = true
      }
      let layout = KeyboardLayoutPreference.geometry
      // 符：#+= 层的符号键、假名标点菜单，以及字母行上的可选键。拼音网格从它的 @# 键打开面板，笔画键从 #+= 层打开，与 Android 一致。
      nineKeySymbolsButton.isHidden = !(kana || (symbolLayer ? showsMoreSymbols
        : !nineKeyFrame && !showsSymbols && layout.showsFullKeyboardSymbols))
      let symbolsTitle = symbolLayer ? "符号" : "符"
      if nineKeySymbolsButton.configuration?.title != symbolsTitle {
        nineKeySymbolsButton.configuration?.title = symbolsTitle
        nineKeySymbolsButton.configuration?.titleTextAttributesTransformer = symbolLayer ? Self.functionLabelTransformer : Self.keyLabelTransformer
      }
      fullSymbolsWidth?.isActive = !nineKeySymbolsButton.isHidden && (style == .phone || style == .standard || style == .tablet)
      layerEmojiButton.isHidden = !(symbolLayer && !showsMoreSymbols)
      nineKeyZeroButton.isHidden = style != .nineKey
      bottomLanguageButton?.isHidden = style == .symbolLayer
      bottomLanguageWidth?.isActive = style == .standard
      // iPad 的 26 键从第三排字母输入 ，。，所以它的底行没有自己的 ，。
      quickPunctuationButton.isHidden = nineKeyFrame || showsSymbols || kana || style == .tablet
      quickPunctuationWidth?.isActive = !quickPunctuationButton.isHidden && style == .standard
      periodKey.isHidden = style != .phone || showsSymbols
      let punctuation = quickPunctuationSymbols
      if phoneRow {
        // ， 与旁边的 。 一样从下面的标点键面取键面；长按仍弹出快捷标点。
        quickPunctuationButton.accessibilityValue = nil
      } else {
        quickPunctuationButton.configuration?.title = punctuation[0]
        quickPunctuationButton.accessibilityLabel = "常用标点"
        quickPunctuationButton.accessibilityValue = punctuation[0]
      }
      // A long press opens this menu without firing the tap action, so the choice counts the press of the key once, never the mark it types.
      quickPunctuationButton.menu = UIMenu(children: punctuation.map { symbol in
        UIAction(title: symbol) { [weak self] _ in
          self?.countKeyPress(TypingKeyID.quickPunctuation)
          self?.handleSymbol(symbol)
        }
      })
      // 底行的 ⌫ 是大千符号行唯一的 ⌫；123 / #+= 层和九键外框在各自的行里带 ⌫。
      actionDeleteButton.isHidden = !showsSymbols || kana || symbolLayer || nineKeyFrame
      symbolDeleteWidth?.isActive = !actionDeleteButton.isHidden && style == .standard
      switch style {
      case .phone: NSLayoutConstraint.activate(phoneActionWidths)
      case .nineKey: NSLayoutConstraint.activate(nineKeyActionWidths)
      case .symbolLayer: NSLayoutConstraint.activate(layerActionWidths)
      case .standard: NSLayoutConstraint.activate(standardActionWidths)
      case .tablet: NSLayoutConstraint.activate(tabletActionWidths)
      }
    }
    symbolRowViews.forEach { $0.isHidden = !showsSymbols || kana || nineKeyFrame || symbolLayer }
    symbolLayerRowViews.forEach { $0.isHidden = !symbolLayer }
    if symbolLayer { updateSymbolLayerFaces() }
    updateSymbolKeyFaces()
    for (row, height) in standardRowHeights { height.isActive = !row.isHidden }
    if var configuration = layoutToggleButton?.configuration {
      configuration.title = symbolLayer || digitPad ? SymbolLayerLayout.lettersTitle(chinese: symbolLayerIsChinese)
        : showsSymbols ? (kana ? "あいう" : (nineKey ? "九键" : "ABC")) : "123"
      layoutToggleButton?.configuration = configuration
    }
    layoutToggleButton?.accessibilityLabel =
      symbolLayer || digitPad ? (strokes ? "切换到笔画" : "切换到字母键盘") : showsSymbols ? "切换到字母" : "切换到数字和符号"
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
    updateSpaceKeyTitle()
    updatePreferredKeyboardHeight()
  }

  /// 底行是否为设计稿的手机底行（123 | ， | space | 。 | 中 | return）：手机键盘显示字母或大千符号行时是，任一设备形态上的手写板也是，与 Android 的 designEntries 给手写共用底行一致。九键外框和假名键保留自己的底行，123 / #+= 层自带一行（`ActionRowStyle.symbolLayer`），iPad 的字母也有自己的（`usesTabletBottomRow`）。
  static func usesPhoneBottomRow(formFactor: KeyboardFormFactor, nineKeyFrame: Bool, handwriting: Bool, kana: Bool) -> Bool {
    !nineKeyFrame && !kana && (handwriting || formFactor == .phone)
  }

  /// 底行是否为 iPad 的 26 键底行（123 | 中 | space | 123 | ⌄，`TabletLetterLayout`）：平板键盘显示字母行时才是，因为这一行的回车在第二排字母上。
  static func usesTabletBottomRow(formFactor: KeyboardFormFactor, letterRows: Bool) -> Bool {
    formFactor == .tablet && letterRows
  }

  /// 按当前设备形态，在字母行里显示 iPad 键盘的 ⌫、回车和第二个 ⇧，或者手机的 ⌫ 和 44pt 的 ⇧。
  private func applyTabletLetterKeys() {
    let tablet = formFactor == .tablet
    for key in [tabletDeleteKey, tabletReturnKey, rightShiftButton].compactMap({ $0 }) where key.isHidden == tablet {
      key.isHidden = !tablet
    }
    if let delete = letterDeleteKey, delete.isHidden != tablet { delete.isHidden = tablet }
    // 先停用当前生效的那个宽度约束，两个约束不会同时生效。
    if tablet {
      phoneShiftWidth?.isActive = false
      tabletShiftWidth?.isActive = true
    } else {
      tabletShiftWidth?.isActive = false
      phoneShiftWidth?.isActive = true
    }
  }

  /// 屏幕上当前界面画哪种底行。
  private enum ActionRowStyle {
    /// 手机 26 键底行：123 | ， | space | 。 | 中 | return，中/英 紧挨回车左边。
    case phone
    /// 九键外框：123 | space | 0 | 中 | return，中/英 紧挨回车左边。
    case nineKey
    /// 123 / #+= 层：拼音或 ABC | 表情或符号 | space | return。
    case symbolLayer
    /// iPad 26 键底行：123 | 中 | space | 123 | ⌄。
    case tablet
    /// iPad 上大千各排下面的底行，弹性空格键两侧是固定宽度的键。
    case standard
  }

  /// 按布局画出的顺序排列底行的键。按键是重新插入而不是从行里移除，所以它们的宽度约束一直保持安装；每种顺序都列出全部按键，隐藏的放在最后。
  private func arrangeActionRow(_ style: ActionRowStyle) {
    guard let toggle = layoutToggleButton, let language = bottomLanguageButton, let gap = actionSplitGap,
          let space = spaceButton, let splitSpace = splitSpaceButton, let enter = enterButton else { return }
    let symbols: UIView = nineKeySymbolsButton, globe: UIView = actionGlobeButton, delete: UIView = actionDeleteButton
    let comma: UIView = quickPunctuationButton, period: UIView = periodKey
    let emoji: UIView = layerEmojiButton, zero: UIView = nineKeyZeroButton
    let tabletLayer: UIView = tabletLayerButton, dismiss: UIView = dismissKeyButton
    let order: [UIView]
    switch style {
    case .phone:
      order = [symbols, toggle, globe, delete, comma, space, gap, splitSpace, period, language, enter, emoji, zero, tabletLayer, dismiss]
    case .nineKey:
      order = [toggle, globe, space, gap, splitSpace, zero, language, enter, symbols, delete, comma, period, emoji, tabletLayer, dismiss]
    case .symbolLayer:
      order = [toggle, emoji, symbols, globe, space, gap, splitSpace, enter, language, delete, comma, period, zero, tabletLayer, dismiss]
    case .tablet:
      order = [symbols, toggle, language, globe, space, gap, splitSpace, tabletLayer, dismiss, delete, comma, period, enter, emoji, zero]
    case .standard:
      order = [symbols, toggle, globe, delete, comma, space, gap, splitSpace, period, language, enter, emoji, zero, tabletLayer, dismiss]
    }
    guard actionRow.arrangedSubviews != order else { return }
    for (index, key) in order.enumerated() where actionRow.arrangedSubviews.firstIndex(of: key) != index {
      actionRow.removeArrangedSubview(key)
      actionRow.insertArrangedSubview(key, at: index)
    }
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
    // 手写板上按住 ⌫ 一笔一笔撤回笔迹，笔迹撤完这次按住就结束，不会接着删文档里已经上屏的文字；要删文字得再按一次。
    if !handwriting.isHidden && handwriting.hasInk {
      didRepeatBackspace = true
      handleBackspace()
      if !handwriting.hasInk {
        backspaceRepeatTimer?.invalidate()
        backspaceRepeatTimer = nil
      }
      return
    }
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
    if gestureRecognizer.name == "spaceVoiceHold" { return spaceVoiceArmed && !cursorMovement.isActive }
    if gestureRecognizer.name == "layoutToggleSymbolsHold" { return !showsSymbols }
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
    noteOwnEdit()
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
      // 分离式键盘有两个空格，只给正在拖的那个换标题；结束时 updateSpaceKeyTitle 把两个都还原。
      // 拖动时的标签单独显示、不带麦克风，与 Android 的临时空格标签一致。
      if let space = pan.view as? UIButton, var configuration = space.configuration {
        configuration.title = spaceDragEditsComposition ? "移动拼音光标" : "移动光标"
        configuration.image = nil
        space.configuration = configuration
      }
      if KeyboardFeedbackPreference.hapticsEnabled {
        KeyboardFeedbackPreference.hapticStrength.impact(keyFeedback)
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
    if let ascii = conversion["space_ascii"]
        .flatMap({ MetasequoiaInputSessionBridge.strictUInt64($0) })
        .flatMap(UInt8.init(exactly:)),
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

  /// 键盘每次经 `textDocumentProxy` 改动文档之后都记一笔，`textWillChange` 据此认出自己的回声。
  private func noteOwnEdit() {
    ownEditEcho.recordOwnEdit(at: ProcessInfo.processInfo.systemUptime)
  }

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
      textDocumentProxy.setMarkedText(text, selectedRange: NSRange(location: (text as NSString).length, length: 0))
      textDocumentProxy.unmarkText()
      noteOwnEdit()
    } else {
      showInlineComposition("")
      textDocumentProxy.insertText(text)
      noteOwnEdit()
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
    guard hasFullAccess, privacyGate.allows(.typing) else { return }
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
    guard let id, countsKeyPresses, hasFullAccess, privacyGate.allows(.keys) else { return }
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
    textDocumentProxy.deleteBackward()
    noteOwnEdit()
  }

  /// Bring the host's marked text in line with `text`; an empty `text` takes it out.
  private func showInlineComposition(_ text: String) {
    guard let edit = InlineCompositionPolicy.edit(showing: inlineMarkedText, next: text) else { return }
    inlineMarkedText = text
    switch edit {
    case .mark(let marked):
      textDocumentProxy.setMarkedText(marked, selectedRange: NSRange(location: (marked as NSString).length, length: 0))
    case .clear:
      textDocumentProxy.setMarkedText("", selectedRange: NSRange(location: 0, length: 0))
      textDocumentProxy.unmarkText()
    }
    noteOwnEdit()
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
    currentNineKeySingleCharacter = snapshot.nineKeySingleCharacter
    currentNineKeyStrokes = snapshot.nineKeyStrokes
    updateLetterCaseControls()
    if let commitText = snapshot.commitText {
      insertOwnText(source == .japanese ? commitText : chineseOutput(commitText), source: source,
                    replacingComposition: true)
    }
    let wasComposing = hasComposition
    let wasKoreanHanjaListOpen = koreanHanjaListOpen
    hasComposition = !snapshot.preedit.isEmpty
    // 菜单与工具栏共用顶栏，组字时顶栏会变成候选条；菜单打开时到来的组字（手写或回复结果）会把菜单关掉。
    if hasComposition && morePicker != nil { closeKeyboardPicker() }
    strokeKeys?.setComposing(hasComposition)
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
    // 全拼九键的 `preedit` 是数字，读音行显示 Engine 给的拼音读音（`ning'bai`）；没有读音时才退回数字。
    let shownPreedit = !snapshot.nineKeyReading.isEmpty ? snapshot.nineKeyReading
      : inputScheme.isJapanese && !snapshot.reading.isEmpty ? snapshot.reading : snapshot.preedit
    let composing = snapshot.phrasePrefix + shownPreedit
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
        drawsKeysAsGlyphs: isChineseMode && inputScheme.drawsKeysAsGlyphs, phrasePrefix: snapshot.phrasePrefix, preedit: snapshot.preedit,
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
    // 内联高度调节条占据工具栏的位置；下面的按键照常可用，所以打字时候选仍会盖在两者之上显示。
    shortcutBar.isHidden = showsCandidates || inlineHeightBar != nil || toolbarHidden
    // 显示方式「隐藏」：与 Android 隐藏空闲时的工具栏区域一样，有内容可显示之前顶栏不存在，按键上移占据它的位置。
    setTopRowCollapsed(toolbarHidden && !showsCandidates && inlineHeightBar == nil)
    inlineHeightBar?.isHidden = showsCandidates
    readingRow.isHidden = !showsCandidates
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
    // 全拼、双拼以及五笔候选都可能在这里携带展示注释：前两者是辅助码或纠错提示，五笔是词条反查编码。
    guard !engine.isEmpty, !isInLocalMode,
          inputScheme == .quanpin || usesShuangpin || inputScheme == .wubi else { return ("", "") }
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

  private func synchronizeTranslationRoute() -> Bool {
    let route = TranslationProviderPreference.route(in: session.sharedPreferences)
    guard route != translationRoute || route.cacheScope != translations.scope else { return false }
    translationRoute = route
    translations.use(route == .account ? BackendCandidateTranslationService() : ProviderCandidateTranslationService(route: route),
                     scope: route.cacheScope)
    DiagnosticLog.shared.write("translation_route provider=\(route.provider?.rawValue ?? "none")")
    // Rows reserved for network-only languages follow whether any service is chosen.
    applyCandidateGlossLayout()
    return true
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
          WubiCodeHintPreference.isEnabled(in: session.sharedPreferences) else { return "" }
    return WubiCodeHintPreference.hint(
      code: code, typed: typed,
      answeredByPinyinFallback: visibleCandidatesAnsweredByPinyinFallback)
  }

  private func refreshCandidatePanelAnnotations() {
    guard let panel = candidatePanel, let current = candidatePanelSnapshot else { return }
    guard let value = try? session.allCandidates(),
          let snapshot = try? CandidatePanelSnapshot.decode(value) else {
      closeKeyboardPicker()
      return
    }
    guard snapshot.generation == current.generation else {
      // 九键面板里选拼音、退格和筛选都会换一代候选，面板留着、按新的一代重建，组字结束才收起。其他面板照旧在候选换代时收起。
      guard nineKeyPanelColumns != nil, hasComposition else {
        closeKeyboardPicker()
        return
      }
      candidatePanelSnapshot = snapshot
      panel.reload(candidates: snapshot.entries.map(\.text), annotations: candidatePanelAnnotations(snapshot),
                   markers: candidatePanelMarkers(snapshot))
      updateNineKeyPanelColumns()
      return
    }
    panel.updateAnnotations(candidatePanelAnnotations(snapshot))
    updateNineKeyPanelColumns()
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
      guard let generation = CandidateGlossModel.integerValue(value, maximum: UInt64.max) else { return }
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
  /// 候选词块的内边距：两侧各 11，与设计稿的词块一致。
  static let candidateChipInsets = NSDirectionalEdgeInsets(top: 4, leading: 11, bottom: 4, trailing: 11)
  /// 词块的最小尺寸，让单字候选也足够好点。
  static let candidateChipMinimumSize = CGSize(width: 30, height: 34)
  static let candidateChipRadius: CGFloat = 9
  /// 默认候选字号下的候选文字，由 `candidate_font_size` 缩放。
  static let candidateChipFontSize: CGFloat = 18
  /// 候选下面的释义行和翻译行。
  static let candidateGlossFontSize: CGFloat = 10

  /// 候选字体：18pt 乘以同步的字号比例，本机装有文档字体链里的字体时就用它。与按键一样固定字号而不跟随动态字体：候选条的高度已经跟随字号设置。
  static func candidateChipFont(scale: CGFloat, families: [String], weight: UIFont.Weight = .regular) -> UIFont {
    let size = (candidateChipFontSize * scale).rounded()
    guard let first = families.first else { return .systemFont(ofSize: size, weight: weight) }
    let descriptor = UIFontDescriptor(fontAttributes: [
      .family: first,
      .cascadeList: families.dropFirst().map { UIFontDescriptor(fontAttributes: [.family: $0]) },
    ])
    return weighted(UIFont(descriptor: descriptor, size: size), weight)
  }

  private func makeCandidateButton(index: Int) -> KeyboardKeyButton {
    var configuration = UIButton.Configuration.plain()
    configuration.titleLineBreakMode = .byTruncatingTail
    configuration.baseForegroundColor = KeyboardTheme.current.keyForeground
    configuration.contentInsets = Self.candidateChipInsets
    // 只有首个词块（空格会上屏的那个）带底色（`updateCandidateButton`）；其余直接放在键盘背景上。
    configuration.background.backgroundColor = .clear
    configuration.background.strokeWidth = 0
    configuration.background.cornerRadius = Self.candidateChipRadius

    let button = KeyboardKeyButton(
      configuration: configuration,
      primaryAction: UIAction { [weak self] _ in
        guard let self else { return }
        self.playInputClick()
        self.selectCandidate(at: index)
      })
    // 词块按住时变淡，而不是像按键那样缩小。
    button.pressFeedback = .opacity(0.6)
    button.setContentCompressionResistancePriority(.required, for: .horizontal)
    let minimumWidth = button.widthAnchor.constraint(greaterThanOrEqualToConstant: Self.candidateChipMinimumSize.width)
    let minimumHeight = button.heightAnchor.constraint(greaterThanOrEqualToConstant: Self.candidateChipMinimumSize.height)
    minimumHeight.priority = .init(999)
    NSLayoutConstraint.activate([minimumWidth, minimumHeight])
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
      insets: Self.candidateChipInsets)
  }

  /// `leadLine` is the width of a line under the candidate that must not be cut, a Korean Hanja's 훈음, which widens the chip as the candidate itself would.
  private func pinCandidateWidth(
    of button: KeyboardKeyButton, firstLine title: AttributedString?, glossLines: Int, glossLine: CGFloat,
    leadLine: CGFloat = 0
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
      glossLine: glossLine, glossLines: glossLines, column: candidateColumnWidth(),
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
      // 设计稿的候选条：首个词块（空格会上屏的那个）放在按键颜色的底块上，其余直接放在键盘上；日语转换时标出当前所在的词块。
      configuration.background.customView = nil
      configuration.background.backgroundColor = converting ? skin.accent.withAlphaComponent(0.22)
        : number == 1 ? skin.keyBackground : .clear
      configuration.background.cornerRadius = Self.candidateChipRadius
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
    var glossWidth: CGFloat = 0
    let candidateFont = Self.candidateChipFont(scale: candidateFontScale, families: candidateFontFamilies, weight: weight)
    if annotation.isEmpty && lines.isEmpty && markers.isEmpty {
      configuration.titleLineBreakMode = .byTruncatingTail
      configuration.attributedTitle = nil
      configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
        var attributes = attributes
        attributes.font = candidateFont
        return attributes
      }
      configuration.title = display
    } else {
      configuration.titleLineBreakMode = .byWordWrapping
      // The runs below carry their own fonts; a transformer would flatten the gloss lines to the candidate size.
      configuration.titleTextAttributesTransformer = nil
      let paragraph = NSMutableParagraphStyle()
      paragraph.alignment = .natural
      paragraph.lineBreakMode = .byTruncatingTail
      var title = AttributedString(display, attributes: AttributeContainer([
        .font: candidateFont,
        .paragraphStyle: paragraph,
      ]))
      title += Self.markerRun(markers, color: annotationColor, scale: candidateFontScale)
      if !annotation.isEmpty {
        title += AttributedString(" " + annotation, attributes: AttributeContainer([
          .font: CandidateFontPreference.font(.caption1, scale: candidateFontScale), .paragraphStyle: paragraph,
          .foregroundColor: annotationColor,
        ]))
      }
      let caption = UIFont.systemFont(ofSize: Self.candidateGlossFontSize)
      readingWidth = reading.map { ceil(($0 as NSString).size(withAttributes: [.font: caption]).width) } ?? 0
      glossWidth = lines.map { ceil(($0 as NSString).size(withAttributes: [.font: caption]).width) }.max() ?? 0
      let content = KeyboardKeyButton.chipContentWidth(
        titleLine: max(NSAttributedString(title).size().width, readingWidth), glossLine: glossWidth,
        glossLines: lines.count, column: candidateColumnWidth())
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
                      glossLine: glossWidth, leadLine: readingWidth)
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
    makeIconKey(UIImage(systemName: symbol), accessibilityLabel: accessibilityLabel, action: action)
  }

  /// 显示 `image` 的功能键：SF Symbol 或设计稿的某个模板图标，用按键颜色着色。
  private func makeIconKey(
    _ image: UIImage?, accessibilityLabel: String, action: (() -> Void)? = nil
  ) -> UIButton {
    var configuration = UIButton.Configuration.plain()
    configuration.image = image
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
    configuration.titleTextAttributesTransformer = Self.keyLabelTransformer
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
    updateSplitKeyboardIfNeeded()
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

  /// 用户调整之前的键盘高度：由内边距、当前尺寸的顶栏、顶栏下的间距和按存储行距排列的各排按键累加而成，所以顶栏变高（释义行、更大的候选字号）或行距变宽会加高键盘，而不是从按键里扣。
  private var preferredKeyboardHeight: CGFloat {
    // 手写共用顶栏，因此也共用竖屏的通用高度。
    formFactor.keyboardHeight(
      topRow: shownTopRowHeight, rowSpacing: CGFloat(KeyboardLayoutPreference.rowSpacing),
      landscape: isLandscape, handwriting: !handwriting.isHidden, numberRow: showsKeyboardNumberRow)
  }

  /// 是否显示第五排按键：iPad 的数字行，或代替它的大千各排。
  private var showsKeyboardNumberRow: Bool {
    numberRowView?.isHidden == false || zhuyinRowViews.first?.isHidden == false
  }

  private func updatePreferredKeyboardHeight() {
    let adjustedHeight = preferredKeyboardHeight + sharedKeyboardHeightAdjustment
    if keyboardHeightConstraint?.constant != adjustedHeight { keyboardHeightConstraint?.constant = adjustedHeight }
  }

  /// 当前设备形态下键盘四周的内边距和顶栏下的间距（dc.html `kbPad`、`kbGap`）。iPad 键盘停靠或浮动时设备形态会变。
  private func applyKeyboardMetrics() {
    guard let root = keyboardRoot, let topRow = compositionContainer, keyboardPaddingConstraints.count == 4 else { return }
    let padding = formFactor.padding
    keyboardPaddingConstraints[0].constant = padding.top
    keyboardPaddingConstraints[1].constant = padding.leading
    keyboardPaddingConstraints[2].constant = -padding.trailing
    keyboardPaddingConstraints[3].constant = -padding.bottom
    root.setCustomSpacing(topRowCollapsed ? 0 : formFactor.topRowGap, after: topRow)
  }

  /// 假名面板的四排：按存储行距排列的手机竖屏按键区高度，所以竖屏时假名键盘与字母键盘一样高。面板自己决定按键尺寸，在所有设备形态上都保持这个高度。
  static var japaneseKeyBlockHeight: CGFloat {
    KeyboardHeightPercent.portraitKeyBlockHeight(
      tablet: false, numberRow: false, rowSpacing: CGFloat(KeyboardLayoutPreference.rowSpacing))
  }

  private func showClipboardHistory() {
    closeKeyboardService()
    closeKeyboardPicker()
    // 密码、验证码这类输入框和隐私模式下根本没有「云端」页，也就什么都不去取（`KeyboardPrivacyGate` 的 `cloudClipboard`，与 Android 的 `cloudClipboardAllowed` 一致）；本机历史照常可插入，只是不保存新的剪贴板。
    let cloud = privacyGate.allows(.cloudClipboard) ? KeyboardCloudClipboard(hasFullAccess: hasFullAccess) : nil
    cloud?.fieldAllowsCloud = { [weak self] in self?.privacyGate.allows(.cloudClipboard) ?? false }
    let panel = KeyboardClipboardView(
      hasFullAccess: hasFullAccess, cloud: cloud,
      capturesHistory: { [weak self] in self?.privacyGate.allows(.clipboardHistory) ?? false },
      showsHeader: false, onInsert: { [weak self] text in
      guard let self else { return }
      render(session.finishComposition())
      insertOwnText(text)
      closeKeyboardPicker()
    }, onClose: { [weak self] in self?.closeKeyboardPicker() })
    clipboardPanel = panel
    installPanel(panel)
  }

  /// 常用语：已存储的短语，每次打开面板时在主线程之外读取，再显示在键区。点一条会在已组的内容之后发出它；「添加常用语」打开 app 的常用语页面，因为键盘只列出已存储的内容。
  private func showPhrasesPanel() {
    closeKeyboardService()
    closeKeyboardPicker()
    let request = UUID()
    phrasesRequest = request
    DispatchQueue.global(qos: .userInitiated).async { [weak self] in
      let loaded = Result { try CommonPhrasesBridge.load() }
      DispatchQueue.main.async { [weak self] in
        guard let self, phrasesRequest == request else { return }
        phrasesRequest = nil
        presentPhrases(loaded)
      }
    }
  }

  private func presentPhrases(_ loaded: Result<[CommonPhrasesBridge.Phrase], Error>) {
    let phrases = (try? loaded.get()) ?? []
    var failed = false
    if case .failure(let error) = loaded {
      failed = true
      DiagnosticLog.shared.write("common_phrases_failed \(error)")
    }
    let panel = KeyboardPhrasesView(phrases: phrases, skin: KeyboardTheme.current, loadFailed: failed,
      onInsert: { [weak self] text in
        guard let self else { return }
        render(session.finishComposition())
        insertOwnText(text)
        closeKeyboardPicker()
      },
      onAddPhrase: { [weak self] in self?.openApp(KeyboardAppLauncher.phrasesURL) })
    phrasesPanel = panel
    installPanel(panel)
  }

  /// 在键区显示 `panel`：位于顶栏之下、按键之上，面板打开期间按键隐藏，透明面板就不会透出它们。工具栏留在上方且可操作，因为关面板靠的就是它的按钮。
  private func installPanel(_ panel: UIView) {
    guard let root = keyboardRoot, let topRow = compositionContainer else { return }
    // 面板替换回复面板的方式与替换按键相同。
    if replyKeyboardShown { closeReplyKeyboard() }
    keyAreaPanel = panel
    panel.accessibilityViewIsModal = false
    panel.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(panel)
    NSLayoutConstraint.activate([
      panel.topAnchor.constraint(equalTo: topRow.bottomAnchor),
      panel.bottomAnchor.constraint(equalTo: root.bottomAnchor),
      panel.leadingAnchor.constraint(equalTo: root.leadingAnchor),
      panel.trailingAnchor.constraint(equalTo: root.trailingAnchor),
    ])
    setKeyRowsCovered(true)
    updateToolbarActiveState()
    UIAccessibility.post(notification: .layoutChanged, argument: panel)
  }

  /// 隐藏或恢复顶栏下面的所有行，以及它们旁边的单手模式侧栏。这些行保留布局，键盘高度不变；它们只是不再绘制、不再接收触摸。
  private func setKeyRowsCovered(_ covered: Bool) {
    for row in keyColumn.arrangedSubviews + [oneHandGutter] {
      row.alpha = covered ? 0 : 1
      row.isUserInteractionEnabled = !covered
      row.accessibilityElementsHidden = covered
    }
  }

  // The drags report on every gesture frame; the shared document is written once the value has
  // settled, so a single adjustment does not take a file lock a hundred times.
  private func commitTouchKeyboardGeometry() {
    let saved = session.persistTouchKeyboardGeometry(
      keySpacing: KeyboardLayoutPreference.keySpacing,
      rowSpacing: KeyboardLayoutPreference.rowSpacing,
      heightAdjustment: KeyboardLayoutPreference.heightAdjustment,
      voiceEnabled: KeyboardLayoutPreference.voiceShortcutEnabled)
    guard !saved else { return }
    // 共享文档没写成（多半是和设置 App 同时写、比较交换输了）。原先不声不响：App Group 和这个键盘用着新值，文档里还是旧值，下次键盘出现时被文档盖回去，看起来就是「调的高度自己变回去了」。现在记一笔日志，并按文档把 App Group 和键盘对齐，用户当场看到的就是实际留下的值。
    DiagnosticLog.shared.write("touch_geometry_not_persisted")
    guard let document = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    KeyboardLayoutPreference.mirrorGeometry(document)
    sharedKeyboardHeightAdjustment = CGFloat(KeyboardLayoutPreference.heightAdjustment)
    applyLayoutPreferences()
    updatePreferredKeyboardHeight()
  }

  /// 品牌键：关掉盖住按键的任何东西（面板、候选网格、回复面板或菜单本身），否则在第一页打开菜单。
  private func toggleMorePicker() {
    if keyAreaPanel != nil || phrasesRequest != nil {
      closeKeyboardPicker()
      return
    }
    if replyKeyboardShown {
      closeReplyKeyboard()
      return
    }
    showMorePicker()
  }

  /// 收起键：面板、候选网格或回复面板盖住按键时先回到键盘，与 Android 一致；没有盖住时收起整个键盘。
  private func dismissOrReturnToKeys() {
    if keyAreaPanel != nil || phrasesRequest != nil {
      closeKeyboardPicker()
      return
    }
    if replyKeyboardShown {
      closeReplyKeyboard()
      return
    }
    dismissKeyboard()
  }

  private func showMorePicker() {
    closeKeyboardService()
    closeKeyboardPicker()
    moreToolsPage = .root
    let picker = KeyboardMorePickerView(tools: makeTools(), formFactor: formFactor)
    morePicker = picker
    installPanel(picker)
    picker.resetToFirstPage()
  }

  /// 默认高度下按键区的高度：`rows` 排高为 `keyHeight` 的按键加上排间距。手机竖屏为 189pt。公式取自 `KeyboardHeightPercent`，app 的键盘页面也用它。
  static func keyBlockHeight(keyHeight: CGFloat, rows: Int, rowSpacing: CGFloat) -> CGFloat {
    KeyboardHeightPercent.keyBlockHeight(keyHeight: keyHeight, rows: rows, rowSpacing: rowSpacing)
  }

  /// 把存储的高度调整值（单位为点）换算成调节条显示的按键区百分比。
  static func heightPercent(adjustment: CGFloat, keyBlock: CGFloat) -> Int {
    KeyboardHeightPercent.percent(adjustment: adjustment, keyBlock: keyBlock)
  }

  /// 把调节条上的百分比换回以整点存储的调整值，限制在共用的 -12…48 范围内。
  static func heightAdjustment(percent: Int, keyBlock: CGFloat) -> CGFloat {
    KeyboardHeightPercent.adjustment(percent: percent, keyBlock: keyBlock)
  }

  /// `touch_keyboard_height_adjustment` 校验后的范围，单位为点。
  static let heightAdjustmentRange: ClosedRange<CGFloat> = KeyboardHeightPercent.adjustmentRange

  private var currentKeyBlockHeight: CGFloat {
    Self.keyBlockHeight(
      keyHeight: formFactor.keyHeight(landscape: isLandscape, handwriting: false),
      rows: formFactor.keyRows(numberRow: showsKeyboardNumberRow),
      rowSpacing: CGFloat(KeyboardLayoutPreference.rowSpacing))
  }

  /// 键盘高度：内联调节条占据工具栏的位置，下面的按键照常可用，改动当场可见。设置仍以点为单位存储；调节条以按键区高度的百分比显示。
  private func showInlineHeight() {
    closeKeyboardService()
    closeKeyboardPicker()
    // 调节条已经开着时保留它和正在预览的高度，不再叠一条新的，也不覆盖打开时记下的原高度。
    guard inlineHeightBar == nil, let container = compositionContainer else { return }
    let keyBlock = currentKeyBlockHeight
    inlineHeightSnapshot = sharedKeyboardHeightAdjustment
    let range = Self.heightPercent(adjustment: Self.heightAdjustmentRange.lowerBound, keyBlock: keyBlock)
      ... Self.heightPercent(adjustment: Self.heightAdjustmentRange.upperBound, keyBlock: keyBlock)
    let bar = InlineHeightBar(
      percent: Self.heightPercent(adjustment: sharedKeyboardHeightAdjustment, keyBlock: keyBlock), range: range,
      onChange: { [weak self] percent in self?.previewKeyboardHeight(Self.heightAdjustment(percent: percent, keyBlock: keyBlock)) },
      onCancel: { [weak self] in self?.closeInlineHeight(commit: false) },
      onReset: { [weak self] in self?.previewKeyboardHeight(0) },
      onDone: { [weak self] in self?.closeInlineHeight(commit: true) })
    bar.referenceHeight = keyBlock
    bar.translatesAutoresizingMaskIntoConstraints = false
    container.addSubview(bar)
    NSLayoutConstraint.activate([
      bar.leadingAnchor.constraint(equalTo: shortcutBar.leadingAnchor),
      bar.trailingAnchor.constraint(equalTo: shortcutBar.trailingAnchor),
      bar.topAnchor.constraint(equalTo: shortcutBar.topAnchor),
      bar.bottomAnchor.constraint(equalTo: shortcutBar.bottomAnchor),
    ])
    inlineHeightBar = bar
    renderCandidateStrip()
    UIAccessibility.post(notification: .layoutChanged, argument: bar)
  }

  /// 按 `adjustment` 画出键盘，但不保存。
  private func previewKeyboardHeight(_ adjustment: CGFloat) {
    sharedKeyboardHeightAdjustment = adjustment
    updatePreferredKeyboardHeight()
  }

  /// 「完成」把调节条显示的高度保存到键盘和设置 app 都读取的地方；「取消」或键盘收起时恢复调节条打开时的高度。
  private func closeInlineHeight(commit: Bool) {
    guard let bar = inlineHeightBar else { return }
    if commit {
      KeyboardLayoutPreference.heightAdjustment = Double(sharedKeyboardHeightAdjustment)
      commitTouchKeyboardGeometry()
    } else {
      previewKeyboardHeight(inlineHeightSnapshot)
    }
    bar.removeFromSuperview()
    inlineHeightBar = nil
    renderCandidateStrip()
    UIAccessibility.post(notification: .layoutChanged, argument: moreShortcut)
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
      showsHeader: false,
      onInsert: { [weak self] emoji in self?.insertOwnText(emoji, source: .local) },
      onDelete: { [weak self] in
        self?.countKeyPress(TypingKeyID.backspace)
        self?.deleteOwnBackward()
      },
      onClose: { [weak self] in self?.closeKeyboardPicker() },
      onABC: { [weak self] in self?.closeKeyboardPicker() })
    applyEmojiPickerAppearance(picker)
    emojiPicker = picker
    installPanel(picker)
  }

  /// 按键区里的表情面板与键盘明暗一致时透出键盘背景；表情面板单独设成另一种明暗时铺上自己的皮肤底色，否则按键和颜文字会按面板的明暗着色、却画在键盘明暗的背景上。
  private func applyEmojiPickerAppearance(_ picker: KeyboardEmojiPickerView) {
    let style = KeyboardAppearancePreference.style(KeyboardAppearancePreference.emojiKey, in: session.sharedPreferences)
    picker.overrideUserInterfaceStyle = style
    let showsKeyboard = Self.emojiPanelShowsKeyboardBackground(panel: style, keyboard: view.traitCollection.userInterfaceStyle)
    picker.backgroundColor = showsKeyboard ? .clear : KeyboardTheme.current.background
  }

  /// 表情面板是否透出键盘背景：面板跟随键盘（`.unspecified`）或与键盘明暗相同时透出，否则铺自己的底色。不是 private：测试会固定它。
  static func emojiPanelShowsKeyboardBackground(panel: UIUserInterfaceStyle, keyboard: UIUserInterfaceStyle) -> Bool {
    panel == .unspecified || panel == keyboard
  }

  /// 长按字母层的 123（见 `makeActionRow`）。
  @objc private func handleLayoutToggleHold(_ gesture: UILongPressGestureRecognizer) {
    guard gesture.state == .began, !showsSymbols else { return }
    openSymbolPanelFromLayoutToggle()
  }

  /// 从 123 打开符号面板，记一次「符」键，与点 #+= 层的「符」相同。
  private func openSymbolPanelFromLayoutToggle() {
    countKeyPress(TypingKeyID.symbol)
    showSymbolPanel()
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
    // 把背景交给系统底板的皮肤（原生皮肤、经典兜底）面板是透明的，与主键盘透出同一块系统底板（iOS 26 起是 Liquid Glass），而不是铺一块量出来的不透明色；面板整块盖住键盘，所以下面的工具栏和按键先藏起来，关面板时再放出来。
    if KeyboardTheme.current.drawsNativeBackground { keyboardRoot?.alpha = 0 }
    symbolPanel = panel
    UIAccessibility.post(notification: .screenChanged, argument: panel)
  }

  private func showSchemePicker() {
    closeKeyboardService()
    closeKeyboardPicker()
    let picker = KeyboardSchemePickerView(
      selected: inputScheme, isChineseMode: isChineseMode, showsHeader: false, formFactor: formFactor,
      shuangpinProfile: session.sharedPreferences?["shuangpin_profile"] as? String,
      onSelect: { [weak self] scheme in
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
      }, onAddLanguage: { [weak self] in
        // 语言在 app 的输入页面里添加，词库也在那里安装。
        self?.openApp(KeyboardAppLauncher.inputSettingsURL)
      }, onClose: { [weak self] in self?.closeKeyboardPicker() })
    schemePicker = picker
    installPanel(picker)
  }

  private func showSkinPicker() {
    guard skinPicker == nil else { return }
    closeKeyboardService()
    closeKeyboardPicker()
    let picker = KeyboardSkinPickerView(
      selected: KeyboardTheme.current.id, document: session.sharedPreferences, showsHeader: false, formFactor: formFactor,
      onSelect: { [weak self] id in
        self?.selectTheme(GlobalThemePreference.selecting(id), statisticsID: id)
      }, onSelectDesign: { [weak self] design in
        guard let mapping = GlobalThemePreference.applyingDesign(design) else { return }
        // 与 Android 的皮肤面板一样，任何设计都算作 `custom`。
        self?.selectTheme(mapping, statisticsID: GlobalThemeCatalog.customId)
      }, onClose: { [weak self] in self?.closeKeyboardPicker() })
    skinPicker = picker
    installPanel(picker)
  }

  /// 把在键盘里选的主题写进共享文档，再画出来。App Group 里的副本跟着文档走，与 App 写入选择时一样。文档接受的选择以 `statisticsID` 计入「换装达人」，隐私模式和凭据输入框里除外（`KeyboardPrivacyGate` 的 `typing`），与 Android 的 `recordSkinStatistics` 一致；没有完全访问权限时键盘访问不到统计存储。
  private func selectTheme(_ mapping: @escaping (inout [String: Any]) -> Void, statisticsID: String) {
    if session.updateTheme(mapping) {
      if let document = session.sharedPreferences { GlobalThemePreference.mirror(document) }
      if hasFullAccess && privacyGate.allows(.typing) { TypingStatisticsExtras.recordSkin(statisticsID) }
    }
    KeyboardTheme.reload(session.sharedPreferences)
    closeKeyboardPicker()
    applyKeyboardAppearance()
    applyKeyboardSkin()
    playInputClick()
  }

  private func closeKeyboardPicker() {
    dismissNineKeyHoldOptions()
    phrasesRequest = nil
    if let panel = phrasesPanel {
      panel.removeFromSuperview()
      phrasesPanel = nil
      UIAccessibility.post(notification: .layoutChanged, argument: phrasesShortcut)
    }
    closeCandidatePanel()
    // 键区面板不是模态的，所以焦点只回到打开它们的工具栏按钮上。
    if let picker = morePicker {
      picker.removeFromSuperview()
      morePicker = nil
      moreToolsPage = .root
      UIAccessibility.post(notification: .layoutChanged, argument: moreShortcut)
    }
    if let picker = schemePicker {
      picker.removeFromSuperview()
      schemePicker = nil
      UIAccessibility.post(notification: .layoutChanged, argument: schemeButton)
    }
    if let panel = clipboardPanel {
      panel.removeFromSuperview()
      clipboardPanel = nil
      UIAccessibility.post(notification: .layoutChanged, argument: clipboardShortcut.isHidden ? moreShortcut : clipboardShortcut)
    }
    if let picker = emojiPicker {
      picker.removeFromSuperview()
      emojiPicker = nil
      UIAccessibility.post(notification: .layoutChanged, argument: emojiShortcut.isHidden ? moreShortcut : emojiShortcut)
    }
    if let panel = symbolPanel {
      panel.removeFromSuperview()
      symbolPanel = nil
      keyboardRoot?.alpha = 1
      UIAccessibility.post(notification: .screenChanged, argument: nineKeySymbolsButton)
    }
    if let picker = skinPicker {
      picker.removeFromSuperview()
      skinPicker = nil
      UIAccessibility.post(notification: .layoutChanged, argument: skinShortcut.isHidden ? moreShortcut : skinShortcut)
    }
    if let panel = voicePanel {
      panel.removeFromSuperview()
      voicePanel = nil
      UIAccessibility.post(notification: .layoutChanged, argument: spaceButton)
    }
    if keyAreaPanel != nil {
      keyAreaPanel = nil
      setKeyRowsCovered(false)
    }
    updateToolbarActiveState()
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
        // 只有真正画出来的描边才换成强调色。iOS 27 起每个按钮配置默认带一条透明的 1pt 描边，只看宽度会给拼写和展开箭头加上描边，而设计稿里这两者都不带边框。
        if configuration.background.strokeWidth > 0, (configuration.background.strokeColor?.cgColor.alpha ?? 0) > 0 {
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
    for enter in returnKeys {
      guard var configuration = enter.configuration else { continue }
      // 回车在所有主题里都是强调键：用皮肤的动作底色，设计皮肤通过按键表面来画。
      configuration.background.backgroundColor = skin.actionBackground
      configuration.baseForegroundColor = skin.actionForeground
      enter.configuration = configuration
      decorateKey(enter)
    }
    styleReturnKey()
    updateLanguageModeButton()
    updateSchemeButton()
    updateShortcutButtons()
    renderCandidateStrip()
    updateSpellingStrip()
    updateSpaceKeyTitle()
    exitLocalModeButton.configuration?.baseForegroundColor = candidatePalette?.accent ?? skin.accent
    preeditButton.configuration?.baseForegroundColor = candidatePalette?.secondary ?? skin.secondary
    expandCandidatesButton.configuration?.baseForegroundColor = candidatePalette?.text ?? skin.keyForeground
    expandDivider.backgroundColor = skin.hairline
    inlineHeightBar?.apply(skin: skin)
    oneHandGutter.apply(skin: skin)
    voicePanel?.apply(skin: skin)
    for (_, _, hint, corner) in letterButtons {
      hint.textColor = skin.accent
      corner.textColor = skin.secondary
    }
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
    if let emojiPicker { applyEmojiPickerAppearance(emojiPicker) }
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
    exitLocalModeButton.configuration?.baseForegroundColor = palette?.accent
    preeditButton.configuration?.baseForegroundColor = palette?.secondary
    expandCandidatesButton.configuration?.baseForegroundColor = palette?.text
    renderCandidateStrip()
  }

  override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
    super.traitCollectionDidChange(previousTraitCollection)
    if isViewLoaded, actionRow != nil,
       previousTraitCollection?.userInterfaceStyle != traitCollection.userInterfaceStyle {
      applyKeyboardSkin()
      // 键盘的明暗变了，表情面板是否与它一致也要重新判断。
      if let emojiPicker { applyEmojiPickerAppearance(emojiPicker) }
    }
    // Docking or undocking the iPad keyboard changes the width class, and with it the form factor.
    if isViewLoaded, actionRow != nil,
       previousTraitCollection?.horizontalSizeClass != traitCollection.horizontalSizeClass {
      updateKeyboardLayoutIfNeeded()
      // The form factor also sets how large a synced candidate size may be drawn.
      applyCandidateGlossLayout()
    }
  }

  /// iPad 旋转时 size class 不变，traitCollectionDidChange 不会被叫到；转完再核对一次分离式键盘（viewDidLayoutSubviews 通常已经处理过）。
  override func viewWillTransition(to size: CGSize, with coordinator: UIViewControllerTransitionCoordinator) {
    super.viewWillTransition(to: size, with: coordinator)
    coordinator.animate(alongsideTransition: nil) { [weak self] _ in self?.updateSplitKeyboardIfNeeded() }
  }

  @objc private func prepareKeyFeedback() {
    if KeyboardFeedbackPreference.hapticsEnabled { keyFeedback.prepare() }
  }

  private func playInputClick() {
    if KeyboardFeedbackPreference.soundEnabled {
      UIDevice.current.playInputClick()
    }
    if KeyboardFeedbackPreference.hapticsEnabled {
      KeyboardFeedbackPreference.hapticStrength.impact(keyFeedback)
      keyFeedback.prepare()
    }
  }
}
