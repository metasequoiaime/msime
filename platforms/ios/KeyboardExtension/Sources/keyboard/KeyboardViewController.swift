import SwiftUI
import CoreImage
import CoreFoundation
import UIKit

@MainActor
final class KeyboardViewController: UIInputViewController, UIGestureRecognizerDelegate {
  private enum LetterCaseState: Equatable {
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
  /// 建引擎会话之前的内存读数。存储属性按声明顺序初始化，所以它紧挨在 `session` 上面：前面几个属性（皮肤背景视图、读 App Group 的键盘高度）算进它自己，`keyboard_loaded` 把它记成 `base=`，与 `session=` 之差只有引擎会话。
  private let launchMemory = DiagnosticLog.MemorySample.current()
  private let session = MetasequoiaInputSessionBridge()
  private var englishCorrectionRejectedPrefix = ""
  private var englishCorrectionRejectedDocument: UUID?
  /// 候选条内容每换一次加一，在途的英文建议回来时拿它判断自己是否已过时。
  private var englishSuggestionStripGeneration: UInt64 = 0
  /// 当前可点的英文建议所属的词和输入框；点击时再次核对，不能拿旧答案替换新词。
  private var englishSuggestionPrefix = ""
  private var englishSuggestionDocument: UUID?
  /// 候选栏当前的英文建议第二行是否为纠错；只有补全时点原文不算拒绝纠错。
  private var englishSuggestionCorrected = false
  /// 引擎会话刚建好时的内存读数，紧跟在 `session` 后面声明，`keyboard_loaded` 记成 `session=`。
  private let sessionMemory = DiagnosticLog.MemorySample.current()
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
  /// Whether the request for `candidateGlossRequestedGeneration` covered the whole candidate list. When false it covered only the strip's page, and opening the candidate grid still asks once more for the whole list of the same generation.
  private var candidateGlossRequestedFullList = false
  private let translations = CandidateTranslationStore()
  /// The translation service the shared document picks; `.none` means nothing was chosen or the chosen provider is incomplete, and no words leave the device.
  private var translationRoute: TranslationRoute = .none
  /// Account glosses are tied to the login generation, not just to the provider name. The
  /// keyboard can outlive the account screen, so this identity is refreshed asynchronously.
  private var translationAccountScope: String?
  private var translationAccountIdentityTask: Task<Void, Never>?
  private var translationRouteGeneration = 0
  private lazy var onlineCandidates: OnlineCandidateProvider = {
    let provider = OnlineCandidateProvider(session: session)
    provider.onApplied = { [weak self] in self?.render($0) }
    return provider
  }()
  /// 云候选和候选栏 AI 都关闭时，按键路径不需要为每个字符序列化一次在线查询。
  private var onlineCandidatesEnabled = false
  private var servicePanel: UIViewController?
  private var replyPanel: UIHostingController<ReplyKeyboardView>?
  private weak var compositionContainer: CandidateCanvasHostView?
  private let replyModel = ReplyKeyboardModel()
  private var personalDictionaryTimer: Timer?
  private let idleMaintenance = IdleMaintenancePoll()
  /// 「我的词条」在键盘里变了之后让词库同步记下来（`local_change`），见 `KeyboardDictionarySync`。
  private let dictionarySync = KeyboardDictionarySync()
  private var lastKeyTouchTime: TimeInterval = -.infinity
  private var synchronizingPersonalDictionary = false
  private let preeditButton = UIButton()
  private var appliedPreeditMenuKey: String?
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
  private struct CandidateButtonInputs: Equatable {
    let candidate: String
    let hint: String
    let hintSpoken: String?
    let reading: String?
    let glosses: [String]
    let pronunciation: String?
    let markers: [CandidateMarker]
    let number: Int
    let converting: Bool
    let aiPlaceholder: Bool
    let source: Int
    let confidentCorrection: Bool
    let styleRevision: UInt64
  }
  private var candidateButtonInputs: [CandidateButtonInputs?] = []
  private var candidateButtonStyleRevision: UInt64 = 0
  /// 输入方式：始终在工具栏里，在键区打开方案选择。
  private let schemeButton = KeyboardToolbarButton(icon: .toolbarScheme, accessibilityLabel: "选择输入方案")
  private let shortcutBar = UIStackView()
  private var candidateContent: UIStackView?
  /// 工具栏的语音按钮。iOS 的语音入口是长按空格和功能菜单的「语音输入」，工具栏不放语音按钮；按钮仍排在栏里但保持隐藏，与下面的高情商回复一样，按标识符查找它的地方不会找到别的控件。
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
  /// 输入方式或功能面板打开时是不是横屏；转屏后行数变了就关掉它（`closePickerBuiltForOtherOrientation`）。
  private var pickerLandscape = false
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
  private var swipeSymbolsDirection: KeyboardLayoutPreference.SwipeDirection = .up
  /// 上次配置诊断日志时闸门是否挡着它，`textDidChange` 据此判断要不要重新配置。
  private var diagnosticLogSuppressed = false
  /// 存储的单手模式，在键盘出现时读取，由它的磁贴和侧栏修改。
  private var oneHandedMode: KeyboardOneHandedMode = .off
  /// 本机的工具栏设置（`TouchToolbarLocalPreference`），与 Android 的 `applyLocalSettings` 一样在键盘出现时读取：常用语和输入方式按钮，以及显示方式「隐藏」。
  private var toolbarPhrases = true
  private var toolbarScheme = true
  private var toolbarHidden = false
  /// 工具栏上除品牌键和收起键之外还有没有显示的按钮，由 `updateShortcutButtons` 算出；一个都没有时空闲工具栏内容按显示方式「隐藏」处理（`toolbarRowHidden`）。
  private var toolbarHasTools = true
  /// 按键行当前按哪种单手模式布局（`applyOneHanded`）；第一次布局之前为 nil。
  private var appliedOneHanded: KeyboardOneHandedMode?
  /// 上一次 `applyOneHanded` 排出的样子：模式之外还有 iPad 单手的块宽和侧边数字键盘的宽度，任何一项变了才重排。
  private var appliedOneHandArrangement: OneHandArrangement?
  /// 顶栏下面的那一行：各排按键组成的列，单手模式下还有它旁边的侧栏；iPad 单手时还有空出来那一侧的空白和侧边数字键盘。
  private let oneHandRow = UIStackView()
  /// 所有按键行，依次叠在顶栏下方。
  private let keyColumn = UIStackView()
  private let oneHandGutter = KeyboardOneHandGutterView()
  /// 手机单手时侧栏占整行的 15%。
  private var oneHandGutterWidth: NSLayoutConstraint?
  /// iPad 单手时侧栏和按键列的宽度是常量（手机大小的一块，`KeyboardOneHandLayout.blockWidth`），空出来的宽度交给 `oneHandSideSpacer`。
  private var oneHandGutterTabletWidth: NSLayoutConstraint?
  private var keyColumnTabletWidth: NSLayoutConstraint?
  /// iPad 单手时空出来的那一侧：一块不接触摸、也不把触摸交给任何键的空白（普通 `UIView`，不是 stack view，`KeyAreaStackView` 不会为它找键），吃掉块和侧边数字键盘之外的宽度。
  private let oneHandSideSpacer = UIView()
  /// iPad 单手时贴在空出来那一侧屏幕边上的侧边数字键盘（共享偏好 `touch_one_handed_side_panel` 选了 `number_pad`，主键区画九键一族的字母层时显示）。它是行里的一个容器，里面的 `oneHandSidePadStack` 从顶上排下来：三行数字与九宫格逐行同高，`. 0 ⌫` 一行与底行同高。
  private let oneHandSidePad = UIView()
  private let oneHandSidePadStack = UIStackView()
  private let oneHandSidePadGrid = UIStackView()
  private var oneHandSidePadRows: [UIStackView] = []
  private var oneHandSidePadWidth: NSLayoutConstraint?
  /// 侧边数字键盘三行与九宫格同高、最后一行与底行同高；只在它显示时生效，九宫格隐藏时不跟着压成零高。
  private var oneHandSidePadHeights: [NSLayoutConstraint] = []
  /// 共享偏好 `touch_one_handed_side_panel`，键盘出现和读到新文档时更新。
  private var oneHandedSidePanel: TouchOneHandedSidePanel = .off
  /// 长按空格键时在键区打开的语音面板。
  private var voicePanel: KeyboardVoicePanelView?
  /// 正在进行的按住说话，录音和识别在后台的主 App 里（`KeyboardVoiceDictation`）。
  private var voiceDictation: KeyboardVoiceDictation?
  /// 听写开始时的输入位置；结果回来时位置变了就不直接上屏。
  private var voiceDictationContext: KeyboardDocumentContext?
  /// 按住空格开始听写时手指的纵坐标，用来判断上滑取消；松手后或读屏开始的听写为 nil。
  private var voiceHoldOriginY: CGFloat?
  /// 手写区：铺满键盘宽度的书写卡片，与 Android 的 rebuildHandwritingRows、HarmonyOS 的 handwritingPad 一致；⌫ 与「重写」在底行（`handwritingRewriteButton`、`handwritingDeleteButton`）。
  private let handwriting = HandwritingInputView()
  private var handwritingResults: [String] = []
  private var handwritingActionHeight: NSLayoutConstraint?
  private var candidatePanel: KeyboardCandidatePanelView?
  /// 面板正在显示的那一代候选。选择和长按菜单都按它里面的代次和全局序号找候选；九键面板换代时整份换掉。
  private var candidatePanelSnapshot: CandidatePanelSnapshot?
  /// 面板里刚选了候选、这次选择的快照正在画：普通网格这时换代也不收起，而是换上新的一代，由 `selectPanelCandidate` 画完后决定面板留不留（#9193）。
  private var candidatePanelFollowsSelection = false
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
    let letters: String?
    let numberHint: UILabel?
  }
  private var nineKeyGridKeys: [NineKeyGridKey] = []
  /// 「九键滑动输入数字」（共享偏好 `touch_nine_key_swipe_digits`，#10229），键盘出现时从共享文档同步；九宫格的键每次判定时经 `activeNineKeySwipeDigits` 读它。
  private var nineKeySwipeDigits = NineKeySwipeDigits.mirrored
  /// 26 键双拼画不画键位提示（共享偏好 `touch_shuangpin_key_hints`），键盘出现时从共享文档同步，绘制键面时不再读 App Group。
  private var shuangpinKeyHintsEnabled = KeyboardLayoutPreference.shuangpinKeyHints

  /// 全拼 26 键组字时 ⇧ 保留（臂辅助码）而不换成「分词」（共享偏好 `touch_quanpin_shift`），键盘出现时从共享文档同步。
  private var quanpinShiftEnabled = KeyboardLayoutPreference.quanpinShift
  private enum MoreToolsPage { case root, localInput }
  private var moreToolsPage: MoreToolsPage = .root
  private let dismissShortcut = KeyboardToolbarButton(icon: .collapse, accessibilityLabel: "收起键盘")
  /// 每个字母键及其两个提示：`hint` 是沿底边显示的双拼韵母，`corner` 是下滑时输入的符号（`LetterHintTable`），画在右上角。
  private var letterButtons: [(button: UIButton, lowercase: String, hint: UILabel, corner: UILabel)] = []
  /// 滑行输入（`KeyboardLayoutPreference.glideTyping`）的开关，每次键盘出现时读一次，按键时不再读 App Group。
  private var glideTypingEnabled = false
  private let glideGesture = GlideTypingGestureRecognizer()
  private let glideTrail = GlideTrailView()
  private var glideActive = false
  private var cursorSwipeEnabled = false
  private var cursorSwipeActive = false
  private var cursorSwipeHadMultipleTouches = false
  private var cursorSwipeDocument: UUID?
  private lazy var keyboardCursorPan: UIPanGestureRecognizer = {
    let pan = UIPanGestureRecognizer(target: self, action: #selector(handleKeyboardCursorPan(_:)))
    pan.name = "keyboardCursorPan"
    pan.maximumNumberOfTouches = 1
    pan.cancelsTouchesInView = true
    pan.delaysTouchesBegan = false
    pan.delaysTouchesEnded = false
    pan.delegate = self
    return pan
  }()
  /// a..z 二十六个字母键，按字母顺序；滑行请求里的键中心按这个顺序给出。
  private var glideLetterKeys: [UIButton] = []
  /// 第二排 L 右边的 `;` 键，微软双拼和粤语双拼显示（`ChineseInputScheme.showsSemicolonFinalKey`）。名字沿用最早只给微软双拼时的叫法，测试按 `microsoftFinalKey` 这个无障碍 id 找它。
  private var microsoftFinalKey: UIButton?
  /// Comma and full stop at the end of the third letter row; shown only on the tablet keyboard.
  private var letterRowPunctuationKeys: [UIButton] = []
  /// 键盘的设备形态：键盘高度、上下内边距、顶栏下的间隔、按键高度和排数，以及面板和候选栏都按它。
  private var formFactor: KeyboardFormFactor { .resolve(traitCollection) }
  /// 键面按哪种形态画：iPad 全宽键盘单手时按键缩到手机大小，键面用手机的（没有 Tab 和 iPad 底行，字母字号、中间一排缩进和底行样式都按手机；「数字行与 Tab 键」的数字行照画，见 `updateKeyboardLayout`），键盘高度和内边距仍按 `formFactor`，单手开关不改键盘外框的高度。
  private var keyFaceFormFactor: KeyboardFormFactor {
    formFactor == .tablet && (appliedOneHanded ?? .off) != .off ? .phone : formFactor
  }
  private var letterRowViews: [UIView] = []
  private var symbolExtraRowView: UIStackView?
  private var symbolExtraRowKeys: [UIButton] = []
  /// iPad 字母上方的数字行及 Q 前的 Tab 键，见 `KeyboardLayoutPreference.tabletFullKeys`。
  private var numberRowView: UIStackView?
  private var tabKey: UIButton?
  /// 横屏分离式键盘每一排中间的中缝（见 `KeyboardSplitLayout`）。不分离时隐藏，stack view 不给它宽度也不给它两侧的键距；分离时它是一块普通视图，落在上面的触摸由它接住后丢掉，不会被 `KeyAreaStackView` 当成键距交给旁边的键。
  private var splitGaps: [UIView] = []
  private var splitGapWidths: [NSLayoutConstraint] = []
  /// 第三排字母的中缝只给一种键面用：iPad 键面这一排末尾有 ，。，九个字符键从 b 后面分（zxcvb | nm，。）；手机键面 ，。 隐藏，七个字母从 v 后面分（zxcv | bnm）。两段中缝都插好，按键面只显示其中一段。
  private var tabletOnlySplitGap: UIView?
  private var phoneOnlySplitGap: UIView?
  /// 分离时右半最里面的第二个空格键，和 `spaceButton` 做同样的事。
  private weak var splitSpaceButton: UIButton?
  /// 上一次 `updateKeyboardLayout` 画的是不是分离式键盘；旋转或改设置后与 `wantsSplitKeyboard` 不同就重新布局。
  private var splitKeyboardShown = false
  /// 共用的每排十键符号行：大千注音的 #+= 页。大千的 123 与别的键面一样打开数字九宫格，从它左列的 #+= 进这一页，底行的切换键印 123、回到数字九宫格（`toggleLayout`）；其他键面的 #+= 是设计稿的符号层（`drawsSymbolLayer`）。按键经 `handleSymbol` 的注音分支原样写出，不当作注音键。
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
  /// 拼音网格 1 键放进候选条的常用标点；为空时候选条画 Engine 的候选。下一次 `render` 换回 Engine 的候选。
  private var nineKeyQuickSymbols: [String] = []
  /// 候选条上正显示的邮箱后缀（`EmailSuffixPolicy`）；为空时候选条画别的内容。每次 `updateCandidateStrip` 都按光标前的文字重新决定，不另外保存读到的上文。
  private var visibleEmailSuffixes: [String] = []
  /// 自己改了文档之后，下一轮主线程上已排好一次邮箱后缀的重新匹配。
  private var emailSuffixRefreshScheduled = false
  /// The four Dachen rows, shown instead of the letter rows while the Zhuyin scheme is active.
  private var zhuyinRowViews: [UIView] = []
  /// 全拼 14 键的三排键（`FourteenKeyLayout`），中文模式下代替三排字母。
  private var fourteenKeyRowViews: [UIView] = []
  /// 18 键的三排键（`EighteenKeyLayout`），与 14 键一样代替三排字母；三排从同一左右边界铺开，第二排六个键平分整行。
  private var eighteenKeyRowViews: [UIView] = []
  /// 14 键和 18 键的各键和它代表的那一组字母，长按和横滑时据此找出这一键的字母。
  private var letterGridButtons: [(button: UIButton, key: FourteenKeyLayout.Key)] = []
  /// 第三排两端分词键和 ⌫ 的宽度：手机上是 26 键 ⇧ 和 ⌫ 的 44pt，iPad 上是 26 键 ⇧ 占一排的比例（`applyTabletLetterKeys` 切换）。
  private var fourteenKeyEdgeWidths: [(phone: NSLayoutConstraint, tablet: NSLayoutConstraint)] = []
  /// 14 键和 18 键第三排最左的键：组字时是「分词」，送 `'`；空闲时是「符」，打开符号面板，与九键的 1 键相同。
  private var letterGridSeparatorButtons: [UIButton] = []
  /// 14 键的拼音选择条：挂在读音行右侧，横向滚动，不盖住候选行。九键的拼音条仍在网格左侧的侧栏里。
  private let readingSpellingScrollView = UIScrollView()
  private let readingSpellingStack = UIStackView()
  private var readingSpellingButtons: [UIButton] = []
  /// 拼音选择条显示时给它留的最小宽度，读音再长也不会把它挤没；隐藏时停用，读音照旧占满这一行。
  private var readingSpellingMinimumWidth: NSLayoutConstraint?
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
  private var spaceHoldMovesCursor = false
  private var backspaceRepeatTimer: Timer?
  private var didRepeatBackspace = false
  private var renderedComposition = false
  private var hasComposition: Bool {
    get { renderedComposition || hasPendingAsynchronousInput }
    set { renderedComposition = newValue }
  }
  /// 测试宿主没有真实文档代理；系统扩展默认启用异步，回归可显式覆盖以检查交付顺序。
  var asynchronousInputEnabled = Bundle.main.bundleURL.pathExtension == "appex"
  private var inputDeliveryEpoch: UInt64 = 0
  private var asynchronousInputDocument: UUID?
  /// 连续异步按键（九键数字、26 键全拼和双拼字母）返回的中间组字只保留最新一帧，避免主线程逐个重画候选栏；含上屏文本的快照仍立即交付。
  private var deferredInputSnapshot: MetasequoiaInputSnapshot?
  private var deferredInputRenderScheduled = false
  /// 查询已交付但最新候选尚未绘制时仍算输入中，避免提前开始空闲维护或切换方案。
  var hasPendingAsynchronousInput: Bool {
    session.hasPendingCharacters || deferredInputSnapshot != nil
  }
  private var visibleInputGeneration: UInt64?
  private var visibleCandidateIdentities: [MetasequoiaCandidateIdentity?] = []
  /// 组字中收到内存警告、引擎缓存还没清，等这次组字结束时清（见 `didReceiveMemoryWarning`）。
  private var engineCacheResetPending = false
  private var pairedPunctuation = PairedPunctuationStack()
  /// Japanese conversion keeps the selected candidate in the strip until Return commits it.
  private var japaneseConversionIndex: Int?
  private var isChineseMode = true
  /// 经功能菜单「手写」进入手写前的方案和中英状态，再点一次这个图块回到它们（#9703）；经其他途径换方案时清掉，那时离开手写按 `InputSchemePreference.handwritingReturn` 取第一个不是手写的方案。
  private var schemeBeforeHandwriting: ChineseInputScheme?
  private var englishBeforeHandwriting = false
  /// Shift 发起的英文片段不改变用户明确选择的语言，结束后回到原方案。
  private var temporaryEnglishFromScheme = false

  /// The mode a newly opened keyboard starts in, from the shared `default_ime_mode`.
  ///
  /// Only a new keyboard reads it: once the 中/英 key has been pressed, that choice holds for as long as this keyboard lives, and a settings change never flips the mode under the typist. iOS does not tell a keyboard which app it is typing into, so `ime_mode_scope` has nothing to key a per-app memory on and is not used here (see `HostCapabilities::ime_mode_scope`).
  static func startsInChinese(_ preferences: [String: Any]?) -> Bool {
    preferences?["default_ime_mode"] as? String != "english"
  }
  private var inputContext = KeyboardInputContext()
  private var inputScheme: ChineseInputScheme = .editionFallback {
    didSet {
      if (letterGrid(for: oldValue) == nil) != (letterGrid(for: inputScheme) == nil) {
        applyReadingRowPlacement()
      }
    }
  }
  /// 当前控制器实际应用的布局；App Group 可被设置页随时改写，不能拿镜像判断旧键面。
  private var shuangpinLayout: TouchShuangpinLayoutPreference.Layout = .twentySixKey {
    didSet {
      if inputScheme.shuangpinProfile != nil && oldValue != shuangpinLayout { applyReadingRowPlacement() }
    }
  }
  /// 当前控制器实际应用的五笔布局，理由同 `shuangpinLayout`。
  private var wubiLayout: TouchWubiLayoutPreference.Layout = .twentySixKey {
    didSet {
      if inputScheme == .wubi && oldValue != wubiLayout { applyReadingRowPlacement() }
    }
  }
  /// `scheme` 在本控制器已应用的双拼、五笔布局下画哪种字母网格（14 键或 18 键）；26 键字母和其他键区为 nil。
  private func letterGrid(for scheme: ChineseInputScheme) -> LetterKeyGrid? {
    LetterKeyGrid.of(scheme, shuangpinLayout: shuangpinLayout, wubiLayout: wubiLayout)
  }
  private var letterGrid: LetterKeyGrid? { letterGrid(for: inputScheme) }
  /// 当前方案画字母网格：全拼或双拼的 14 键、18 键，或五笔 14 键。
  private var usesLetterGrid: Bool { letterGrid != nil }
  /// 此刻第二排 L 右边是否显示 `;` 键：方案要它（微软双拼、粤语双拼），而且在中文模式、不在本地模式、不画 14/18 键网格。键的显隐和手机中间一排的缩进都按它算，切到英文或本地模式时 `;` 藏起来，中间一排要回到两侧各缩进半个键。
  private var showsSemicolonKey: Bool {
    isChineseMode && inputScheme.showsSemicolonFinalKey && !isInLocalMode && !usesLetterGrid
  }
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
  /// 字母层用自定义符号表（`touch_nine_key_sidebar_symbols`）时的左列：一屏最多三项、等高，多的上下滚动。出厂表时隐藏，左列仍是原来的 `punctuationStack`，画面不变。
  private let customSidebarScroll = NineKeySidebarScrollView()
  /// 两张符号表此刻画在左列上的样子（空表是出厂表），变了才重建按键。
  private var builtLetterSidebar: [String] = []
  private var builtDigitSidebar: [String] = []
  /// 数字层左列此刻是否画着半角语境追加的符号（`NineKeySidebar.asciiDigitExtras`）。
  private var builtDigitSidebarExtras = false
  private let numberPadSidebar = UIStackView()
  private let numberPadOperators = UIStackView()
  /// 九键右列的 ⌫：数字九宫格左列每一项与它同高。
  private weak var nineKeyDeleteKey: UIButton?
  private var quickPunctuationButton: UIButton!
  private var quickPunctuationWidth: NSLayoutConstraint?
  /// 14 键和手写等底行的独立句号；手机 26 键通过逗号长按输入句号。
  private var periodKey: UIButton!
  /// 分离式键盘底行的中缝，留着引用是为了重排这一行时它仍紧挨空格键。
  private var actionSplitGap: UIView?
  private var symbolDeleteWidth: NSLayoutConstraint?
  private var standardActionWidths: [NSLayoutConstraint] = []
  /// 九键字母层底行的弹性权重，以空格键的 4.2 为基准：123 1.25、地球键 1、0 1.05、中 1.05、回车 1.6。
  private var nineKeyActionWidths: [NSLayoutConstraint] = []
  private var numberPadActionWidths: [NSLayoutConstraint] = []
  private var numberPadExpandedZeroWidths: [NSLayoutConstraint] = []
  private var numberPadGridWidths: [NSLayoutConstraint] = []
  private var numberPadExpandedGridWidths: [NSLayoutConstraint] = []
  /// 123 / #+= 层的底行，以空格键的 6 为基准（`SymbolLayerLayout`）。
  private var layerActionWidths: [NSLayoutConstraint] = []
  /// 设计稿的手机底行弹性权重，以空格键的 4 为基准：123 1.25、中 1.05、地球键 1、， 1、。 1、回车 1.9。
  private var phoneActionWidths: [NSLayoutConstraint] = []
  /// iPad 26 键底行，以空格键的 6.4 为基准（`TabletLetterLayout`）：123 1.5、中 1.2、地球键 1、123 1.5、⌄ 1.2。
  private var tabletActionWidths: [NSLayoutConstraint] = []
  private var phoneExpandedSpaceWidths: [NSLayoutConstraint] = []
  /// 英文 26 键底行：空格左边多一个句点，空格以 `englishPhoneSpaceWeight` 为基准，其余键与中文 26 键同宽；隐藏 中/英 时用 expanded 那一组。
  private var phoneEnglishActionWidths: [NSLayoutConstraint] = []
  private var phoneEnglishExpandedSpaceWidths: [NSLayoutConstraint] = []
  /// 手写底行的弹性权重（`handwritingSpaceWeight` 等）；隐藏 中/英 时改用 expanded 那一组，把它的份额交给空格。
  private var handwritingActionWidths: [NSLayoutConstraint] = []
  private var handwritingExpandedSpaceWidths: [NSLayoutConstraint] = []
  /// 手写底行 中 与回车之间的「重写」和 ⌫；其他底行上都隐藏。
  private var handwritingRewriteButton: UIButton!
  private var handwritingDeleteButton: UIButton!
  /// 手写底行 ， 和 。 右上角的 ？ ！ 角标，长按输入它们；其他底行上隐藏。
  private var handwritingCommaHint: UILabel!
  private var handwritingPeriodHint: UILabel!
  private var nineKeyExpandedSpaceWidths: [NSLayoutConstraint] = []
  private var tabletExpandedSpaceWidths: [NSLayoutConstraint] = []
  private var tabletSplitExpandedSpaceWidths: [NSLayoutConstraint] = []
  /// iPad 底行的第二个 123 和收起键盘的 ⌄；其他底行上都隐藏。
  private var tabletLayerButton: UIButton!
  private var dismissKeyButton: UIButton!
  /// 底行是否按手机 26 键底行布局（`usesPhoneBottomRow`），是的话 ， 跟随标点键面，而不是快捷标点菜单的第一个标点。
  private var phoneBottomRowShown = false
  /// 九键字母层按「底栏带标点」画着 ， 和 。：这时 ， 与手机底行一样发逗号键、跟随标点键面，长按仍是快捷标点菜单。
  private var nineKeyPunctuationRowShown = false
  /// 「按键布局」（`TouchKeyLayoutStyle`）：三类键面的底行样式和顶部数字行。键盘出现和重载文档时从共享文档同步，画键时不读 App Group。
  private var keyLayoutStyle = TouchKeyLayoutStyle.mirrored
  /// 非标准样式底行的宽度约束，按 `TouchBottomRowLayout` 的一行缓存，换样式或换键面时整组停用再启用另一组。
  private var styledActionWidthCache: [[TouchBottomRowLayout.Key]: [NSLayoutConstraint]] = [:]
  /// 借来的数字九宫格在另两种空格位置下的宽度约束（`borrowedNumberPadWidths`）。
  private var borrowedNumberPadWidthCache: [BorrowedNumberPadWidthsKey: [NSLayoutConstraint]] = [:]
  private struct BorrowedNumberPadWidthsKey: Hashable {
    let space: TouchNumberPadSpace
    let hidesLanguage: Bool
    let globe: Bool
  }
  private var activeStyledActionWidths: [NSLayoutConstraint] = []
  /// 九键「右侧大回车」底行的宽度约束（`tallReturnActionWidths`），按行和底行键距缓存：约束的常数里扣了键距，键距一变就是另一组。
  private var tallReturnActionWidthCache: [TallReturnWidthsKey: [NSLayoutConstraint]] = [:]
  private struct TallReturnWidthsKey: Hashable {
    let row: [TouchBottomRowLayout.Key]
    let spacing: CGFloat
  }
  /// 九键「右侧大回车」的回车（`installTallReturnKey`）：叠在按键列上，从右列第三格伸到底行。
  private weak var tallReturnKey: UIButton?
  /// 「右侧大回车」在底行原来回车的位置留出的空格，与右列同宽，伸下来的回车盖在它上面。
  private let tallReturnSlot = UIView()
  /// 数字键盘选了「空格在右列」（`touch_number_pad_space`）时右列第三格的空格（`installNumberPadColumnSpace`）：叠在 ！ 上，手势与底行空格相同，底行的空格这时隐藏。
  private weak var numberPadColumnSpace: UIButton?
  /// 九键右列第三格的 ！；「右侧大回车」时它让出位置给回车，！ 改由符号栏末尾的 `nineKeySidebarClosingMark` 输入。
  private weak var nineKeyClosingButton: UIButton?
  private weak var nineKeySidebarClosingMark: UIButton?
  /// 九键数字层符号栏第三排的 符；「右侧大回车」时 符 在符号栏下方的底行，这一个隐藏，运算符占满整条符号栏。
  private weak var numberPadSymbolsKey: UIButton?
  /// 26 键「长空格」时 123 / #+= 层底行原来放表情或符号入口的那一格换成的逗号，上滑、下滑或长按输入句号。
  private var layerCommaButton: UIButton!
  /// 26 键「长空格」把 符 从底行挪到工具栏，这是工具栏上的入口；`toolbarSymbolsEntryShown` 记着上次排工具栏时它在不在。
  private let symbolsShortcut = UIButton()
  private var toolbarSymbolsEntryShown = false
  /// 上次排工具栏时这一行是否因长空格留着（`longSpaceToolbarRowKept`）：按这块键盘算，停在英文 26 键上也留着，只是没有 符。
  private var toolbarRowKept = false
  private let nineKeyContainer = UIStackView()
  private let spellingScrollView = KeyboardSpellingScrollView()
  private var spellingButtons: [UIButton] = []
  private var usesTraditionalOutput = false
  /// 高情商回复面板是否由工具栏的「回复」按钮打开。它是工具而不是方案：打开时盖住键区，关闭后回到原来的键盘，引擎方案始终不变。
  private var replyKeyboardShown = false
  private var reportedStatisticsFailure = false
  /// Key presses waiting for the next write. Counted only while statistics are on, which the store is asked on every appearance; with them off nothing is kept, not even in memory.
  private var keyPresses = TypingKeyCounter()
  private var countsKeyPresses = false
  /// 上次要共享层写出以来，有没有交给它 `record` 过上屏；没有就不必再调 flush。只在主线程读写。
  private var typingStatisticsUnflushed = false
  private var keyPressFlushTimer: Timer?
  private var visiblePreedit = ""
  /// The already-chosen half of a phrase at the front of `visiblePreedit`, which 「候选栏预编辑」 never hides.
  private var visiblePhrasePrefix = ""
  /// The spelling with the Engine's caret drawn in, while the user has moved that caret off the end.
  private var visibleCaretSpelling: String?
  /// The spelling the caret can be moved through and where the caret sits in it, while one is being composed; nil for a Japanese reading, whose conversion owns the caret keys.
  private var editableSpelling: (text: String, caret: Int)?
  /// 五笔 14 键正在组字时引擎的组码（`editing_text`，每一位是一键的代表字母）；其他情况为空。
  private var visibleWubiGridKeys = ""
  /// Whether the current space-bar drag moves the caret inside the composition rather than in the document.
  private var spaceDragEditsComposition = false
  /// The desktop candidate skin the strip draws with, or nil while it follows the keyboard skin (see CandidatePalette).
  private var candidatePalette: CandidatePalette?
  /// 当前候选皮肤的解码快照，普通条与展开面板共用；解码在后台完成。
  private var candidateCanvasSkin: ExternalCandidateSkin.Canvas?
  /// 上一次加载候选皮肤时的请求（开关、明暗、主题、皮肤目录和清单的修改时间与大小），没变就沿用已解码的快照。只有内存警告清它（见 `viewWillAppear` 里的说明）。
  private var candidateCanvasRequestKey: Data?
  private var candidateCanvasEpoch: UInt64 = 0
  /// Whether the top row shows candidates (or the reading, a diagnostic or the local mode) rather than the toolbar; set by `renderCandidateStrip`, read by `currentStripBackground`.
  private var stripShowsCandidates = false
  private var candidateCanvasActive = true
  /// 上一次给整棵按键树换装（`applyKeyboardSkin` 里的 `recolor`）时的皮肤和 trait。`viewWillAppear` 和 `viewIsAppearing` 拿它和现在的比，都没变就不再重新换装。
  private var appliedKeySkin: KeyboardTheme?
  private var appliedKeySkinTraits: UITraitCollection?
  /// 有按键在窗口外经过 `decorateKey`：阴影色是静态的 CGColor，这时按默认外观解析，要等一次在窗口里的完整换装重新解析。
  private var keySkinNeedsWindowPass = true
  private var candidatePreeditGapConstraint: NSLayoutConstraint?
  private static let candidateCanvasQueue = DispatchQueue(label: "app.msime.candidate-canvas", qos: .userInitiated)
  private var candidateRevision: UInt64 = 0
  private var visibleCandidates: [String] = []
  private var visibleCandidateCodes: [String] = []
  private var visibleCandidateGlosses: [String] = []
  /// Offline glosses in the non-English target languages, by language code and then candidate text. Replaced per language by each answer; an entry left over from an earlier composition is still that word's gloss.
  private var candidateTargetGlosses: [String: [String: String]] = [:]
  private var offlineGlossInstalled: (resources: String, languages: Set<String>)?
  /// 「显示读音」：查询的键（`CandidatePronunciationModel.key`，语言加文字）到读音的缓存，查不到的记作 ""。读音只取决于文字、语言和本机的表，与候选代次无关，开关或表变了才整份丢掉（`candidatePronunciationEpoch`）。
  private var candidatePronunciations: [String: String] = [:]
  private var candidatePronunciationsPending = Set<String>()
  private var candidatePronunciationEpoch: UInt64 = 0
  /// 音标表装好没有，在应用候选栏布局时重查（`applyCandidateGlossLayout`）；顶栏是否多留一行按它和开关定，打字过程中不变。
  private var candidatePronunciationTableInstalled = false
  private var visibleCandidateAnnotations: [String] = []
  private var visibleCandidateSources: [Int] = []
  private var visibleCandidateFixedPositions: [Int] = []
  private var visibleCandidateHighlighted: [Bool] = []
  /// 候选条上每个候选是不是坐了首选的双拼纠错（View 的 `confident_correction`），与 `visibleCandidates` 对齐。
  private var visibleCandidateConfidentCorrections: [Bool] = []
  /// 候选条隐藏云占位行后，显示序号对应的 Engine 当前页索引。
  private var visibleCandidateEngineIndices: [Int] = []
  private var visibleCandidatePageCount = 0
  private var appliedCandidateColumnWidth: CGFloat = 0
  private var visibleCandidatesAnsweredByPinyinFallback = false
  private var visibleDiagnostic: String?
  private var diagnosticDismissTimer: Timer?
  private var shuangpinKeyHints: [String: String] = [:]
  private var nineKeyShuangpinLayout: NineKeyShuangpinLayout?
  // What the last commit armed, if anything. These belong to the editor rather than to Engine:
  // a different document, a moved caret, or a session rebuilt while the keyboard was away all
  // mean the gesture is about something else, which is what the editor generation carries.
  // The last snapshot's own answers. Every one of these was in the response the keyboard just
  // received; asking the session again costs a full C ABI round trip per question, and the render
  // path asks several times for every keystroke.
  private var currentLocalMode = "none"
  /// 最近一次快照报告的引号与方头括号样式（View 的 `punctuation_marks`）。
  private var currentCornerQuotes = false
  private var currentHollowBrackets = false
  /// View 的 `punctuation_marks` 顿号开关：`\` 缺省开、`/` 缺省关，快照里读到什么用什么。
  private var currentEnumerationCommaBackslash = true
  private var currentEnumerationCommaSlash = false
  private var currentNineKeySpellings: [String] = []
  private var currentNineKeySingleCharacter = false
  private var currentNineKeyStrokes = ""
  private var appliedLayoutInputs: KeyboardLayoutInputs?
  private var appliedLetterCaseInputs: LetterCaseInputs?
  private var appliedSpellingStripInputs: SpellingStripInputs?
  private var candidateGlossTimer: Timer?
  /// Engine's local mode, as of the last snapshot.
  ///
  /// Every snapshot carries it, and every path that can change it renders one, so this is the
  /// same answer the session would give. Asking the session instead costs a full C ABI round trip
  /// - the whole view serialised to JSON and parsed back - and the keystroke path asked more than
  /// thirty times per key before this.
  private var isInLocalMode: Bool { !currentLocalMode.isEmpty && currentLocalMode != "none" }
  /// 拆字模式（Engine 的 `chaizi`）：候选下的注释是读音，不是还要输入的编码。
  private var readsChaizi: Bool { currentLocalMode == "chaizi" }
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
  private(set) var inlineMarkedText = ""

  /// The chips the strip numbers, the page size the session was given (see CandidatePageSizePreference), so a digit picks the chip carrying its number. Everything past it is in the expanded panel.
  private var candidatePageSize: Int {
    CandidatePageSizePreference.clamped(session.sharedPreferences?["candidate_page_size"] as? Int)
  }
  // 顶栏空闲时是工具栏；组字时由读音和候选占据同一位置。不管显示什么，它的高度都取组字布局的高度，所以组字开始或结束时按键不会移动。
  // 读音默认画在候选行最左边，顶栏只有候选行那么高（#7405：用户反馈候选区和工具栏占地方，设计稿和 Android 的读音行另占 14pt，打字时顶栏是 56pt）。全拼 14 键例外，仍是读音行在上、候选行在下：它的拼音选择条排在读音右边，挤进候选行就没有位置了。
  // 不是 private：高度断言从这些值推出，而不是把总和再写一遍。
  /// 顶栏不会低于它：工具栏按钮激活时的底板是 40pt（`KeyboardToolbarButton.padSide`），上下各留 1pt。
  static let topRowMinimumHeight: CGFloat = 42
  /// 全拼 14 键在候选上方单独一行的读音行，以 12pt 显示拼写。
  static let readingRowHeight: CGFloat = 14
  /// 读音行上的拼写（dc.html：12px、kb.sub、letter-spacing .02em）。
  static let preeditFontSize: CGFloat = 12
  private static let candidateRowHeight: CGFloat = 38
  /// 带释义的候选行：第一行释义和候选字同在这一行里，18pt 候选（21）加 10pt 释义（13）再加上下内边距各 4 正好 42，与 Android 的 `ImeToolbar.CANDIDATE_LINE_DP` 一致。原先释义另加一整行 14pt，开着释义时顶栏是 66pt，比 Android 高 10pt，空闲时工具栏也一直按这个高度留着。
  static let glossedCandidateRowHeight: CGFloat = 42
  /// 第二行释义起每行再加的高度（Android 的 `EXTRA_GLOSS_ROW_DP`）。
  static let glossLineHeight: CGFloat = 14
  static func glossHeight(lines: Int, barScale: CGFloat = 1) -> CGFloat {
    CGFloat(max(lines, 0)) * glossLineHeight * barScale
  }
  // 候选或编码字号调大时所在行跟着变高，调小时行高不会低于默认值：这一行同时也是触摸目标。「候选栏大小」（`barScale`，#10137）连这个默认值一起缩放，用户调小候选栏时这一行跟着变矮；`candidateScale`、`preeditScale` 是已经乘过 `barScale` 的字号倍数。
  static func readingRowHeight(preeditScale: CGFloat, barScale: CGFloat = 1) -> CGFloat {
    max(readingRowHeight * barScale, ceil(readingRowHeight * preeditScale))
  }
  static func candidateRowHeight(candidateScale: CGFloat, glossed: Bool = false, barScale: CGFloat = 1) -> CGFloat {
    let base = glossed ? glossedCandidateRowHeight : candidateRowHeight
    return max(base * barScale, ceil(base * candidateScale))
  }
  /// 顶栏：候选行，第一行释义在候选行里，其余释义行在它下面；`separateReadingRow`（全拼 14 键）时上面再加一行读音。高度不低于 `topRowMinimumHeight` 乘以「候选栏大小」：空闲时这一行是工具栏，它和候选栏一起变矮或变高，组字开始、结束时按键仍不动。
  static func topRowHeight(
    glossLines: Int, candidateScale: CGFloat = 1, preeditScale: CGFloat = 1, separateReadingRow: Bool = false,
    barScale: CGFloat = 1
  ) -> CGFloat {
    max(topRowMinimumHeight * barScale,
        (separateReadingRow ? readingRowHeight(preeditScale: preeditScale, barScale: barScale) : 0)
      + candidateRowHeight(candidateScale: candidateScale, glossed: glossLines > 0, barScale: barScale)
      + glossHeight(lines: glossLines - 1, barScale: barScale))
  }
  /// 用户调整之前的键盘高度：按默认候选字号和存储的行距计算，每个候选下带 `glossLines` 行释义（默认取配置的行数）；`separateReadingRow` 同 `topRowHeight`。
  /// Tests and host layout consumers use this contract so the default gloss row stays accounted for.
  static func keyboardHeight(
    _ formFactor: KeyboardFormFactor = .phone, landscape: Bool = false, handwriting: Bool = false,
    numberRow: Bool = false, glossLines: Int? = nil, separateReadingRow: Bool = false, phoneNumberRow: Bool = false
  ) -> CGFloat {
    let lines = glossLines ?? configuredGlossLines(fullAccess: false, onlineRoute: false)
    return formFactor.keyboardHeight(
      topRow: topRowHeight(glossLines: lines, separateReadingRow: separateReadingRow),
      rowSpacing: CGFloat(KeyboardLayoutPreference.rowSpacing),
      landscape: landscape, handwriting: handwriting, numberRow: numberRow, phoneNumberRow: phoneNumberRow)
  }
  /// 手机竖屏键盘：`keyboardHeight()`。
  static var defaultKeyboardHeight: CGFloat { keyboardHeight() }

  /// 离线词典可直接填充释义；仅在启用了在线服务且确实需要网络时读取系统完全访问权限。
  /// `offline` 是已安装离线释义词典的语言代码，`onlineRoute` 表示已选择翻译服务。
  static func canFillGloss(_ language: CandidateTranslationLanguage, fullAccess: @autoclosure () -> Bool, onlineRoute: Bool,
                           offline: Set<String> = []) -> Bool {
    !CandidateTranslationPreference.needsNetwork(language, offline: offline)
      || (CandidateTranslationPreference.onlineEnabled && onlineRoute && fullAccess())
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
  /// 候选和编码的字号倍数，已经乘过 `candidateBarScale`。
  private var candidateFontScale: CGFloat = 1
  private var preeditFontScale: CGFloat = 1
  /// 「候选栏大小」（`candidate_scale_percent`）：顶栏、候选行、读音行和释义行的高度倍数（#10137）。
  private var candidateBarScale: CGFloat = 1
  /// 每个候选词块的最小高度约束，常量是 `candidateChipMinimumSize.height` 乘以 `candidateBarScale`，「候选栏大小」变化时在 `applyCandidateGlossLayout` 里一起更新。
  private var candidateChipHeightConstraints: [NSLayoutConstraint] = []
  private var candidateFontFamilies: [String] = []
  private var topRowHeightConstraint: NSLayoutConstraint?
  private var readingRowHeightConstraint: NSLayoutConstraint?
  /// 顶栏的读音；只在候选行显示时才显示。默认是候选行最左边的一段，全拼 14 键是候选行上面单独的一行（`separateReadingRow`）。
  private let readingRow = UIView()
  /// 读音单独一行时才生效的约束，和读音并入候选行时的约束，由 `applyReadingRowPlacement` 二选一。
  private var separateReadingConstraints: [NSLayoutConstraint] = []
  private var inlineReadingConstraints: [NSLayoutConstraint] = []
  /// 读音并入候选行而此刻没有读音时，把那一段收成零宽，候选从行首开始。
  private var inlineReadingCollapse: NSLayoutConstraint?
  /// 读音是否在候选行上面单独一行：14 键和 18 键是，它们的拼音选择条排在读音右边。按方案而不是按中英文状态判断，切到英文时顶栏高度不变，按键不跳。
  private var separateReadingRow: Bool { usesLetterGrid }
  /// 根视图的内边距（`KeyboardFormFactor.padding`）：上、前、后、下。
  private var keyboardPaddingConstraints: [NSLayoutConstraint] = []

  /// 按键音效包（`KeySoundPackPolicy`）：要哪个包、已经加载的样本和音量。偏好在键盘出现和重读之后由 `synchronizeKeySoundPack` 更新，按键时只读这几个缓存。
  private let keySounds = KeyboardSoundPlayer()
  private var wantedKeySoundPack: String?
  private var keySoundGain = KeySoundPackPolicy.gain(volume: KeySoundPackPolicy.defaultVolume)
  /// 按键振动的开关和档位都从这一个 App Group 实例读。`KeyboardFeedbackPreference.hapticsEnabled` 这类属性每读一次都新建一个 `UserDefaults`，原来开着振动时按下、松开一次键要建五六个，都压在触摸处理的主线程上。不把值缓存起来：App 的设置页里也有输入框，键盘开着时在那里改振动，下一下按键就要按新的来。
  private let feedbackDefaults = KeyboardFeedbackPreference.defaults
  private var hapticsEnabled: Bool { KeyboardFeedbackPreference.hapticsEnabled(in: feedbackDefaults) }
  private var hapticStrength: KeyboardHapticStrength { KeyboardFeedbackPreference.hapticStrength(in: feedbackDefaults) }
  private var feedbackStrength: KeyboardHapticStrength?
  private var feedbackGenerator: UIImpactFeedbackGenerator?
  private var keyFeedback: UIImpactFeedbackGenerator {
    let strength = hapticStrength
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
    "(": "（", ")": "）", "[": "【", "]": "】", "\\": "、", "/": "、",
    "<": "《", ">": "》", "'": "‘", "\"": "“", "_": "——",
  ]

  /// 按引号与方头括号样式（View 的 `punctuation_marks`）取中文键面：直角引号下 `'` 画「、`"` 画『，空心括号下 `[` `]` 画〖〗，其余与 `chineseSymbolFaces` 相同。这些键经 Engine 上屏，键面与写出的字一致。顿号键随 View 的顿号开关：`\` 关着时画 `\`，`/` 开着时画 `、`。不是 private：键面测试会固定它。
  static func chineseSymbolFace(_ ascii: String, cornerQuotes: Bool, hollowBrackets: Bool,
                                enumerationCommaBackslash: Bool = true,
                                enumerationCommaSlash: Bool = false) -> String? {
    switch ascii {
    case "'" where cornerQuotes: return "「"
    case "\"" where cornerQuotes: return "『"
    case "[" where hollowBrackets: return "〖"
    case "]" where hollowBrackets: return "〗"
    case "\\": return enumerationCommaBackslash ? "、" : ascii
    case "/": return enumerationCommaSlash ? "、" : ascii
    default: return chineseSymbolFaces[ascii]
    }
  }

  /// 英文模式下标点锁定为中文时，按当前 View 的顿号开关选择发给 Engine 的键。nonisolated：纯函数，SmartPunctuationTests 这类非隔离测试可直接钉住。
  nonisolated static func lockedEnglishPunctuationInput(
    for symbol: String, enumerationCommaBackslash: Bool, enumerationCommaSlash: Bool) -> String? {
    KeyboardPunctuationContext.engineInput(
      for: symbol, japanese: false,
      enumerationCommaBackslash: enumerationCommaBackslash,
      enumerationCommaSlash: enumerationCommaSlash)
  }

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
  static func zhuyinSymbolText(_ symbol: String, chinesePunctuation: Bool, cornerQuotes: Bool = false,
                               hollowBrackets: Bool = false,
                               enumerationCommaBackslash: Bool = true,
                               enumerationCommaSlash: Bool = false) -> String {
    chinesePunctuation
      ? chineseSymbolFace(symbol, cornerQuotes: cornerQuotes, hollowBrackets: hollowBrackets,
                          enumerationCommaBackslash: enumerationCommaBackslash,
                          enumerationCommaSlash: enumerationCommaSlash) ?? symbol : symbol
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
    registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitHorizontalSizeClass.self]) {
      (controller: KeyboardViewController, previous: UITraitCollection) in controller.refreshTraits(previous)
    }
    // 先读数再做别的：与 `sessionMemory` 之差是 `session` 之后声明的存储属性和 `loadView` 的开销，与下面 `keyboard_built` 之差是建键盘界面的开销。
    let loadedMemory = DiagnosticLog.MemorySample.current()
    translations.onArrival = { [weak self] in self?.renderCandidateStrip() }
    // 方案以共享文档为准。会话在本控制器创建时已经同步读过一次文档，先把其中记的方案抄进 App Group 镜像再按镜像行事，不额外读盘；否则在 `viewWillAppear` 的后台重载回来之前，键盘会先按镜像里的旧方案（比如双拼）画出来并开始接收按键。
    InputSchemePreference.mirror(session.sharedPreferences)
    TouchShuangpinLayoutPreference.mirror(session.sharedPreferences)
    TouchWubiLayoutPreference.mirror(session.sharedPreferences)
    TouchKeyLayoutStyle.mirror(session.sharedPreferences)
    keyLayoutStyle = TouchKeyLayoutStyle.mirrored
    NineKeySwipeDigits.mirror(session.sharedPreferences)
    nineKeySwipeDigits = NineKeySwipeDigits.mirrored
    // 输入习惯（学习、调频、离线释义、候选翻译与语言）同理以文档为准。镜像里没写过释义开关时读作开启，而文档默认关闭（`candidate_english_gloss` 是 opt-in）；不先抄过来，下面按镜像算出的顶栏会多留一行释义，键盘高 4pt，等后台重载把文档抄进镜像后又缩回去，文档没有更新时重载不抄，就一直按镜像显示释义。
    InputHabitPreference.mirror(InputHabitPreference.settings(in: session.sharedPreferences))
    shuangpinLayout = TouchShuangpinLayoutPreference.layout
    wubiLayout = TouchWubiLayoutPreference.layout
    inputScheme = InputSchemePreference.scheme
    isChineseMode = ImeModeMemoryPreference.startsInChinese(fallback: Self.startsInChinese(session.sharedPreferences))
    appliedCharacterWidth = CharacterWidthPreference.value(in: session.sharedPreferences)
    setFullWidthInput(CharacterWidthPreference.startsFullwidth(in: session.sharedPreferences))
    appliedChinesePunctuation = Self.sharedChinesePunctuation(session.sharedPreferences)
    chinesePunctuation = appliedChinesePunctuation
    synchronizeAICredential()
    synchronizeOnlineCandidatePreference()
    applyKeyboardAppearance()
    // 隐私模式要在诊断日志配置之前读出来：开着时日志一行也不写，包括下面这行。
    incognito = KeyboardPrivacyPreference.incognito
    readGesturePreferences()
    configureDiagnosticLog()
    DiagnosticLog.shared.write("keyboard_loaded full_access=\(hasFullAccess ? 1 : 0) idiom=\(UIDevice.current.userInterfaceIdiom == .pad ? "pad" : "phone") base=\(DiagnosticLog.MemorySample.mebibytes(launchMemory.footprint)) session=\(DiagnosticLog.MemorySample.mebibytes(sessionMemory.footprint)) \(loadedMemory.fields)")
    if session.initializationFailed { DiagnosticLog.shared.write("runtime_initialization_failed") }
    // 音标表在不在决定顶栏多不多留一行，第一帧之前就要知道。
    refreshPronunciationTable()
    glossLineCount = currentGlossLines()
    // 简繁与方案同理以共享文档为准：先把会话创建时读到的文档里记的字形抄进镜像，否则在后台重载回来之前（文档没有更新时它根本不会抄），键盘按镜像里的旧字形转换上屏文字。
    ChineseOutputPreference.mirror(session.sharedPreferences)
    usesTraditionalOutput = ChineseOutputPreference.usesTraditional
    oneHandedMode = KeyboardLayoutPreference.oneHanded
    oneHandedSidePanel = TouchOneHandedSidePanel(in: session.sharedPreferences)
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
      self, selector: #selector(flushStatistics), name: .NSExtensionHostWillResignActive, object: nil)
    DiagnosticLog.shared.write("keyboard_built \(DiagnosticLog.MemorySample.current().fields)")
  }

  override func viewDidAppear(_ animated: Bool) {
    super.viewDidAppear(animated)
    // 到这里键盘已经画出第一帧，视图的位图缓冲和皮肤图片都已分配；`keyboard_built` 之前的几行量不到这部分。
    DiagnosticLog.shared.write("keyboard_shown \(DiagnosticLog.MemorySample.current().fields)")
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
      // 邮箱后缀不是候选条底下的内容，由 `updateCandidateStrip` 按这时的上文重新决定给不给。
      self.updateCandidateStrip(preedit: self.visiblePreedit,
                                candidates: self.visibleEmailSuffixes.isEmpty ? self.visibleCandidates : [])
    }
    synchronizeInputContext()
    prepareKeyFeedback()
    // 出现时先跑一轮词库维护，不必等两秒的定时器：两份队列文件在后台读，个人词库只在有待应用的编辑、设置页要求刷新或上次列出已超过 15 分钟（`PersonalDictionaryStore.listingLifetime`）时才同步，快照版本照常按 `DictionarySnapshotWorker.idleVersionPollInterval`（已有版本、没有活动请求时一分钟）节流。以前这里每次出现都在主线程上强制同步一次个人词库、强制重算一次词库版本，没有任何变化时也要读写共享文件、查询引擎。
    startIdleMaintenance()
    personalDictionaryTimer?.invalidate()
    personalDictionaryTimer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
      MainActor.assumeIsolated { self?.startIdleMaintenance() }
    }
  }

  /// 异步首键尚未渲染时也在输入；词间的短暂停顿不触发同步权限和词库维护。
  private var idleForMaintenance: Bool {
    !hasComposition && !isInLocalMode && !session.hasPendingCharacters
      && ProcessInfo.processInfo.systemUptime - lastKeyTouchTime >= 0.3
  }

  /// 两秒一轮的词库维护。读两份共享文件放到后台，主线程只在读完、并且仍然空闲时做真正要做的同步；通常两边都无事可做，主线程这一轮只读一次完全访问权限。
  private func startIdleMaintenance() {
    guard idleForMaintenance, !idleMaintenance.isRunning else { return }
    let fullAccess = hasFullAccess
    guard fullAccess else {
      // 没有完全访问时个人词库不同步，快照工作者停下并归还租约，两边都不读共享目录。
      snapshotWorker.tick(idle: true, fullAccess: false)
      return
    }
    idleMaintenance.start(readSnapshotState: snapshotWorker.stateReader()) { [weak self] outcome in
      // 后台读文件期间用户可能已经开始输入：这一轮让给按键，下一次定时器再来。
      guard let self, self.idleForMaintenance else { return }
      switch outcome.personalDictionary {
      case .success(true): self.synchronizePersonalDictionary(force: false, fullAccess: fullAccess)
      case .success(false): break
      case .failure(let error): self.showDiagnostic(error.localizedDescription)
      }
      self.snapshotWorker.tick(idle: true, fullAccess: fullAccess, prefetched: outcome.snapshotState)
    }
  }

  override func viewWillAppear(_ animated: Bool) {
    super.viewWillAppear(animated)
    candidateCanvasActive = true
    // 不清候选皮肤的请求键：键里已有皮肤 id、明暗、主题和清单的修改时间与大小，皮肤真的换了照样重新加载。以前这里每次出现都清掉它，跟随桌面候选皮肤的用户每次出现都要丢掉快照、在后台把整包图片重新读一遍解一遍，候选条先退回配色再换回皮肤。
    InputSchemePreference.refreshResourceAvailability()
    // 进程可能跨过换季或应用主题的改动；在下面任何地方解析皮肤之前先刷新。变了就重新解析主题，让末尾的 `applyKeyboardSkin` 给按键换上新的颜色实例：按键进窗口时已按旧季节解析过同一批动态色，同一个实例再设一次 UIButton 不会重新解析。
    let seasonChanged = KeyboardTheme.refreshSeason()
    if seasonChanged { KeyboardTheme.reload(session.sharedPreferences, dark: drawsDark) }
    incognito = KeyboardPrivacyPreference.incognito
    oneHandedMode = KeyboardLayoutPreference.oneHanded
    readToolbarPreferences()
    readGesturePreferences()
    applyOneHanded()
    updateKeyboardLayoutIfNeeded()
    // 隐私模式和输入框都可能在两次出现之间变了，日志开不开要重新判断。
    configureDiagnosticLog()
    DiagnosticLog.shared.write("focus_in \(DiagnosticLog.MemorySample.current().fields)")
    KeyboardUsageReporting.presented(fullAccess: hasFullAccess)
    // 收起时销毁的会话在这里同步重建，挡在这次出现的第一帧前面；记下耗时，在设备上看它有多重。
    let rebuildsSession = session.dictionarySessionSuspended
    let resumeStart = ProcessInfo.processInfo.systemUptime
    do {
      try session.resumeDictionarySession()
      if rebuildsSession { DiagnosticLog.shared.write("dictionary_resumed ms=\(Self.elapsedMilliseconds(since: resumeStart))") }
    }
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
    _ = synchronizeTranslationRoute()
    synchronizeOnlineCandidatePreference()
    glideTypingEnabled = KeyboardLayoutPreference.glideTyping
    // 完全访问和按键音开关都可能在两次出现之间变了；包先按会话手上的文档开始加载，重读之后选择变了再换。
    synchronizeKeySoundPack()
    // The Tauri settings app writes the shared PreferencesStore rather than the
    // legacy App Group UserDefaults used by the old SwiftUI settings page.
    // Reload it off-thread so a fuzzy-pinyin change is visible the next time
    // the keyboard appears without blocking UIKit's input lifecycle.
    session.reloadSharedPreferences { [weak self] loaded in
      guard let self else { return }
      // Not applied also covers a document no newer than the one the session has, which is not a failure.
      guard loaded else { DiagnosticLog.shared.write("preferences_not_applied"); return }
      self.configureDiagnosticLog()
      DiagnosticLog.shared.write("preferences_applied \(DiagnosticLog.MemorySample.current().fields)")
      // A candidate skin, theme or colour synced from the desktop arrives with the document.
      self.refreshCandidatePalette()
      self.applyKeyboardAppearance()
      let translationSettingsChanged = self.synchronizeSharedTouchPreferences()
      self.synchronizeCharacterWidth()
      self.synchronizeChinesePunctuation()
      self.synchronizeAICredential()
      self.synchronizeOnlineCandidatePreference()
      self.synchronizeChineseOutputPreference()
      self.applyLearningPreferences()
      self.synchronizeKeySoundPack()
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
    // 换了季节必须整棵重新换装（见上面 `refreshSeason` 的注释）；否则只在皮肤变了时才在这里重走按键树，trait 和窗口外建过的按键留给 `viewIsAppearing`。
    applyKeyboardSkin(onlyIfChanged: !seasonChanged)
    synchronizeReplyKeyboard()
    startCountingKeyPresses()
  }

  /// 到这里视图已经进了窗口、trait 也定了，而第一帧还没画。`viewWillAppear` 时视图通常还不在窗口里：在那里按 trait 和「有按键在窗口外装饰过」判断，`viewDidLoad` 留下的标记会让第一次出现再整棵走一遍，在窗口外走完又把标记留给下一次，每次出现都是完整换装。所以这两项在这里判断，只有确实要补的时候才换装。
  override func viewIsAppearing(_ animated: Bool) {
    super.viewIsAppearing(animated)
    if keySkinNeedsWindowPass || appliedKeySkin != KeyboardTheme.current || appliedKeySkinTraits != traitCollection {
      applyKeyboardSkin()
    }
  }

  /// 诊断日志里的耗时：从 `start`（`systemUptime`）到现在的整毫秒数。
  private static func elapsedMilliseconds(since start: TimeInterval) -> Int {
    Int(((ProcessInfo.processInfo.systemUptime - start) * 1000).rounded())
  }

  /// iOS ends a keyboard extension that keeps using too much memory, so a warning is worth a line when a report says the keyboard vanished.
  override func didReceiveMemoryWarning() {
    super.didReceiveMemoryWarning()
    DiagnosticLog.shared.write("memory_warning \(DiagnosticLog.MemorySample.current().fields)")
    // 收到警告后仍占着可选面板、释义结果或引擎候选缓存的键盘扩展很快会被系统结束。能重建的都丢掉，正在进行的组字和会话保留。
    closeKeyboardPicker()
    closeKeyboardService()
    handwriting.deactivate()
    snapshotWorker.stop()
    // 按键音的播放器下次出现时重建，在那之前退回系统按键声。
    keySounds.release()
    candidateGlossEpoch &+= 1
    candidateGlossRequestedGeneration = nil
    translations.clearCacheForMemoryPressure()
    onlineCandidates.cancel()
    // 引擎清缓存后会为当前组字重查一遍候选（`reset_cache` 末尾的 `refresh_candidates`），返回的快照才是此后点选序号所对应的列表。组字中清缓存却丢掉快照，候选条和引擎是否一致就只能靠重查碰巧排出同样的结果；照「清除候选缓存」那样重画，又要在内存最紧的时候重查词图、重置候选条。组字中的缓存多半正是这次组字要用的行，清掉马上又查回来，省不下多少。所以没有组字时当场清：这时引擎没有候选，候选条上的英文联想和手写结果不归引擎管，快照里没有要画的东西；组字中只记下来，等这次组字结束（上屏或取消）时由 `render` 补清，长句组字攒下的缓存不会一直留到下一次空闲时的警告。
    let engineCache: String
    if hasComposition {
      engineCacheResetPending = true
      engineCache = "deferred"
    } else {
      engineCacheResetPending = false
      _ = session.resetCache()
      engineCache = "reset"
    }
    ResolvedTheme.clearCacheForMemoryPressure()
    candidateCanvasEpoch &+= 1
    candidateCanvasRequestKey = nil
    candidateCanvasSkin = nil
    candidatePalette = currentCandidatePalette()
    applyCandidateAppearance()
    DiagnosticLog.shared.write("memory_released engine_cache=\(engineCache) \(DiagnosticLog.MemorySample.current().fields)")
  }

  /// 补上组字中那次内存警告没清的引擎缓存。只在组字刚结束时由 `render` 调用：引擎这时没有候选，清缓存返回的快照没有要画的东西，丢掉它和空闲时的警告是同一个道理。
  private func releaseDeferredEngineCache() {
    guard engineCacheResetPending, !hasComposition else { return }
    engineCacheResetPending = false
    _ = session.resetCache()
    DiagnosticLog.shared.write("engine_cache_reset \(DiagnosticLog.MemorySample.current().fields)")
  }

  /// `diagnostic_log.server` 开着时让诊断日志写到共享目录，关掉后停写；隐私模式和凭据输入框里同样停写（`KeyboardPrivacyGate` 的 `diagnosticLog`）。
  private func configureDiagnosticLog() {
    let preferences = session.sharedPreferences ?? MetasequoiaInputSessionBridge.loadSharedPreferences()
    diagnosticLogSuppressed = !privacyGate.allows(.diagnosticLog)
    let directory = session.stateDirectory ?? MetasequoiaInputSessionBridge.sharedStateDirectory
    let enabled = DiagnosticLog.isEnabled(in: preferences) && !diagnosticLogSuppressed
    DiagnosticLog.shared.configure(directory: directory, enabled: enabled)
    // 键位几何的调试导出跟着诊断日志开关走（#10018）；刚打开时把已注册的那份补写出来，不必等布局再变。
    keyGeometryExportDirectory = enabled && directory.hasPrefix("/")
      ? URL(fileURLWithPath: directory, isDirectory: true).appendingPathComponent("debug", isDirectory: true) : nil
    exportKeyGeometry()
  }

  /// 读取本机手势开关；字母键和根手势使用缓存，按键时不再读 App Group。
  private func readGesturePreferences() {
    let spaceVoice = KeyboardLayoutPreference.spaceVoice
    let spaceVoiceChanged = spaceVoice != spaceVoiceEnabled
    spaceVoiceEnabled = spaceVoice
    swipeSymbolsEnabled = KeyboardLayoutPreference.swipeSymbols
    swipeSymbolsDirection = KeyboardLayoutPreference.swipeSymbolsDirection
    cursorSwipeEnabled = KeyboardLayoutPreference.cursorSwipe
    if !cursorSwipeEnabled { endKeyboardCursorSwipe() }
    keyboardCursorPan.isEnabled = cursorSwipeEnabled
    for letter in letterButtons {
      (letter.button as? KeyboardKeyButton)?.swipesCornerHint = swipeSymbolsEnabled
      (letter.button as? KeyboardKeyButton)?.cornerHintSwipeDirection = swipeSymbolsDirection
    }
    for space in spaceKeys {
      space.configuration?.contentInsets = .zero
      space.titleLabel?.adjustsFontSizeToFitWidth = true
      space.titleLabel?.minimumScaleFactor = 0.7
      space.accessibilityHint = Self.spaceAccessibilityHint(voice: spaceVoiceEnabled)
      space.accessibilityCustomActions = spaceAccessibilityActions()
    }
    // 空格键上的麦克风跟着长按空格语音走：从设置 App 改完切回键盘时立刻换键面（#10143）。键盘还没建好时（`viewDidLoad` 里第一次读）由建完后的那次 `updateSpaceKeyTitle` 画。
    if spaceVoiceChanged && !spaceKeys.isEmpty { updateSpaceKeyTitle() }
  }

  /// 空格键的 VoiceOver 提示：语音关闭时，长按空格改为移动光标。
  private static func spaceAccessibilityHint(voice: Bool) -> String {
    voice ? "轻点输入空格或选词，左右滑动移动光标，按住说话，松开上屏" : "轻点输入空格或选词，左右滑动或长按移动光标"
  }

  override func selectionWillChange(_ textInput: UITextInput?) {
    super.selectionWillChange(textInput)
    if !ownEditEcho.isEcho(at: ProcessInfo.processInfo.systemUptime) {
      englishCorrectionRejectedPrefix = ""
    }
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
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    inputDeliveryEpoch &+= 1
    englishCorrectionRejectedPrefix = ""
    finishTemporaryEnglish(force: true)
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
    let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy)
    if let previous = asynchronousInputDocument, previous != document {
      inputDeliveryEpoch &+= 1
      session.flushPendingCharacters()
      flushDeferredInputSnapshot()
      render(discardComposition())
      asynchronousInputDocument = document
      // 换了输入框就是上一个失焦了：攒着的上屏这时写出，不等键盘收起。
      flushTypingStatistics()
    }
    synchronizeInputContext()
    // 宿主可能把焦点换到了凭据输入框，诊断日志跟着当前输入框开关；只在闸门的判断变了时重新配置，不必每次按键都去读偏好。
    if privacyGate.allows(.diagnosticLog) == diagnosticLogSuppressed { configureDiagnosticLog() }
    updateReturnKey()
    updateAutomaticCapitalization()
    synchronizeReplyKeyboard()
    refreshEmailSuffixes()
  }

  override func selectionDidChange(_ textInput: UITextInput?) {
    super.selectionDidChange(textInput)
    cancelCursorSwipeIfDocumentChanged()
    // 光标移到了 `xxx@` 后面，或者从那里移开。
    refreshEmailSuffixes()
  }

  override func viewWillDisappear(_ animated: Bool) {
    super.viewWillDisappear(animated)
    candidateCanvasActive = false
    compositionContainer?.canvasVisible = false
    candidatePanel?.canvasVisible = false
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    inputDeliveryEpoch &+= 1
    englishCorrectionRejectedPrefix = ""
    finishTemporaryEnglish(force: true)
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
    idleMaintenance.cancel()
    closeKeyboardPicker()
    // 键盘收起时还在调整高度就当作取消：没点「完成」的预览不保存，与 Android onFinishInputView 一致。
    closeInlineHeight(commit: false)
    endKeyboardCursorSwipe()
    cursorMovement.cancel()
    spaceHoldMovesCursor = false
    endGlide()
    updateSpaceKeyTitle()
    // Putting the keyboard away used to drop whatever was composed. macOS commits in
    // prepareForDeactivation: for the same reason: the user typed those letters and never asked to
    // throw them away.
    render(session.finishComposition())
    let releasesSession = !session.dictionarySessionSuspended
    let suspendStart = ProcessInfo.processInfo.systemUptime
    // 销毁会话时引擎同步写完选词计数和个人上下文，也在主线程上。
    if session.suspendDictionarySession() && releasesSession {
      DiagnosticLog.shared.write("dictionary_suspended ms=\(Self.elapsedMilliseconds(since: suspendStart))")
    }
    ownEditEcho.reset()
    cancelBackspacePress()
    diagnosticDismissTimer?.invalidate()
    diagnosticDismissTimer = nil
    keyPressFlushTimer?.invalidate()
    keyPressFlushTimer = nil
    flushStatistics()
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
    let extraRow = makeSymbolExtraRow()
    symbolExtraRowView = extraRow
    keyColumn.addArrangedSubview(extraRow)
    for (index, row) in letterRows.enumerated() {
      let rowView = makeLetterRow(row, includesShift: index == letterRows.count - 1)
      letterRowViews.append(rowView)
      keyColumn.addArrangedSubview(rowView)
    }
    // 大千注音、14 键和 18 键的各排不在这里建：只有用这几种方案的人用得到，第一次要画它们时由 `ensureAlternateLetterRows` 插到字母行和九键外框之间。
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
    keyColumn.addArrangedSubview(handwriting)
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
    installTallReturnKey()
    installNumberPadColumnSpace()
    installOneHandSidePad()
    // 右侧大回车和数字层右列的空格不在任何一行里，单独告诉键区让它们分到相邻键距里靠它们的那一半。
    root.overlayKeys = [tallReturnKey, numberPadColumnSpace].compactMap { $0 }
    // 中/英紧挨回车的底行（手机、九键），回车把键帽左边几 pt 让给中/英；别的排法里两者不相邻，不起作用。
    root.yieldingKey = enterButton
    root.yieldReceiver = bottomLanguageButton
    installSplitGaps(in: root)
    // 分几步拼出来：写成一个长的 + 表达式时，编译器会在类型推断上超时。
    var standardRows: [UIView] = [numberRow, extraRow]
    standardRows += letterRowViews
    standardRows += symbolRowViews
    standardRows += symbolLayerRowViews as [UIView]
    standardRowHeights = standardRows.map {
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
    root.addGestureRecognizer(keyboardCursorPan)
    updateKeyboardLayout()
  }

  /// 顶栏下面的那一行：填满它的按键列，以及单手模式的侧栏；侧栏默认隐藏，由 `applyOneHanded` 显示在按键的另一侧。iPad 单手时这一行里还有空出来那一侧的空白和侧边数字键盘（`installOneHandSidePad` 建它的键），默认也隐藏。这一行总是从左到右排列，因为存储的模式记的就是按键靠哪一侧。
  private func installOneHandRow(in root: UIStackView) {
    keyColumn.axis = .vertical
    keyColumn.spacing = root.spacing
    oneHandRow.axis = .horizontal
    oneHandRow.alignment = .fill
    oneHandRow.distribution = .fill
    oneHandRow.spacing = KeyboardOneHandLayout.gap
    oneHandRow.semanticContentAttribute = .forceLeftToRight
    oneHandSidePad.isHidden = true
    oneHandRow.addArrangedSubview(oneHandSidePad)
    oneHandSideSpacer.isHidden = true
    oneHandSideSpacer.isAccessibilityElement = false
    oneHandSideSpacer.accessibilityIdentifier = "oneHandSideSpacer"
    oneHandRow.addArrangedSubview(oneHandSideSpacer)
    oneHandRow.addArrangedSubview(keyColumn)
    oneHandGutter.isHidden = true
    oneHandRow.addArrangedSubview(oneHandGutter)
    // 设为 required，且只在侧栏显示时生效：底行隐藏的键会打破它们可选的宽度权重，侧栏宽度如果也只是可选的，布局引擎会拿它和那些约束权衡，把按键压窄。
    oneHandGutterWidth = oneHandGutter.widthAnchor.constraint(equalTo: oneHandRow.widthAnchor, multiplier: KeyboardOneHandLayout.gutterRatio)
    oneHandGutterTabletWidth = oneHandGutter.widthAnchor.constraint(equalToConstant: 0)
    // 按键列的常量宽度也是 required，理由与上面侧栏的相同：字母行里 iPad 才画的键（Tab、iPad 的 ⌫ 和回车、行内 ，。、第二个 ⇧）隐藏时，它们按字母键宽度定的 999 宽度约束一直被 stack view 的零宽约束打破，字母越窄违反得越少；按键列的宽度若也是 999，布局引擎会把按键列压到字母键的下限（约 164pt），多出来的宽度都给了没有任何宽度约束的空白。空白只靠 stack view 的首尾相接定宽，窗口窄到放不下「块加间距」时（全宽 iPad 键盘实际不会这么窄，转屏的那一趟布局里可能短暂出现）由它让出那几 pt，不与这条约束冲突；多出来的宽度同样都给它。
    keyColumnTabletWidth = keyColumn.widthAnchor.constraint(equalToConstant: 0)
    oneHandSidePadWidth = oneHandSidePad.widthAnchor.constraint(equalToConstant: 0)
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

  /// iPad 单手时空出来那一侧的侧边数字键盘：3 × 4 拨号盘 `1 2 3 / 4 5 6 / 7 8 9 / . 0 ⌫`，不随别的设置变。键用 `makeKey` 建，数字的键面和字号与九键数字层的数字相同；数字和 `.` 走数字层的字面上屏（`insertNumberPadLiteral`：正在组的拼音先按原样上屏，全角开关照常作用），⌫ 与九键右列的 ⌫ 同一个动作和连删。统计记普通数字键、`Period` 和 `Backspace`。不挂滑行、横滑光标、长按选项和角标。
  ///
  /// 键在 `oneHandSidePadStack` 里从顶上排下来：前三行平分与九键外框同高的一块（九宫格三行逐行对齐），`. 0 ⌫` 一行与底行同高。在底行建好之后调用，高度约束要用到它和九键外框；只在面板显示时生效（`applyOneHanded`）。
  private func installOneHandSidePad() {
    oneHandSidePad.accessibilityIdentifier = "oneHandSidePad"
    oneHandSidePadStack.axis = .vertical
    oneHandSidePadStack.alignment = .fill
    oneHandSidePadStack.spacing = keyColumn.spacing
    oneHandSidePadGrid.axis = .vertical
    oneHandSidePadGrid.distribution = .fillEqually
    oneHandSidePadGrid.spacing = 7
    for digits in [["1", "2", "3"], ["4", "5", "6"], ["7", "8", "9"]] {
      let row = makeRow()
      for digit in digits { row.addArrangedSubview(makeSidePadDigitKey(digit)) }
      oneHandSidePadRows.append(row)
      oneHandSidePadGrid.addArrangedSubview(row)
    }
    let bottom = makeRow()
    let decimal = makeKey(title: ".", accessibilityLabel: "小数点") { [weak self] in
      self?.countKeyPress(TypingKeyID.character("."))
      self?.insertNumberPadLiteral(".")
    }
    decimal.accessibilityIdentifier = "sidePadDecimal"
    Self.drawNumberPadDigitFace(decimal)
    bottom.addArrangedSubview(decimal)
    bottom.addArrangedSubview(makeSidePadDigitKey("0"))
    let delete = makeDeleteKey()
    delete.accessibilityIdentifier = "sidePadDelete"
    bottom.addArrangedSubview(delete)
    oneHandSidePadRows.append(bottom)
    oneHandSidePadStack.addArrangedSubview(oneHandSidePadGrid)
    oneHandSidePadStack.addArrangedSubview(bottom)
    oneHandSidePadStack.translatesAutoresizingMaskIntoConstraints = false
    oneHandSidePad.addSubview(oneHandSidePadStack)
    // 面板隐藏时 stack view 把容器压成零宽，三列键的键距放不进去；所以右边略低于 required，隐藏时让开，显示时照样成立。
    let trailing = oneHandSidePadStack.trailingAnchor.constraint(equalTo: oneHandSidePad.trailingAnchor)
    trailing.priority = .init(999)
    NSLayoutConstraint.activate([
      oneHandSidePadStack.topAnchor.constraint(equalTo: oneHandSidePad.topAnchor),
      oneHandSidePadStack.leadingAnchor.constraint(equalTo: oneHandSidePad.leadingAnchor),
      trailing,
      oneHandSidePadStack.bottomAnchor.constraint(lessThanOrEqualTo: oneHandSidePad.bottomAnchor),
    ])
    oneHandSidePadHeights = [
      oneHandSidePadGrid.heightAnchor.constraint(equalTo: nineKeyContainer.heightAnchor),
      bottom.heightAnchor.constraint(equalTo: actionRow.heightAnchor),
    ]
  }

  private func makeSidePadDigitKey(_ digit: String) -> UIButton {
    let key = makeKey(title: digit, accessibilityLabel: "数字 \(digit)") { [weak self] in
      self?.countKeyPress(TypingKeyID.character(digit))
      self?.insertNumberPadLiteral(digit)
    }
    key.accessibilityIdentifier = "sidePadDigit\(digit)"
    Self.drawNumberPadDigitFace(key)
    return key
  }

  /// 九键数字层数字的键面：21pt 中粗、不留内边距，窄键上缩小而不换行（`makeNineKeyLayout` 给九宫格的同一套）。
  private static func drawNumberPadDigitFace(_ button: UIButton) {
    guard var configuration = button.configuration else { return }
    configuration.contentInsets = .zero
    configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
      var attributes = attributes
      attributes.font = KeyboardTheme.current.usesMonospacedFont
        ? .monospacedSystemFont(ofSize: 21, weight: .medium) : .systemFont(ofSize: 21, weight: .medium)
      return attributes
    }
    button.configuration = configuration
    button.titleLabel?.adjustsFontSizeToFitWidth = true
    button.titleLabel?.minimumScaleFactor = 0.7
  }

  /// `applyOneHanded` 排出的样子。`blockWidth` 是 iPad 单手时「侧栏加按键」那一块的宽度，手机上和宽度还没定下来时为 0（侧栏按比例跟着整行走）；`padWidth` 是侧边数字键盘的宽度，不显示时为 0。
  private struct OneHandArrangement: Equatable {
    let mode: KeyboardOneHandedMode
    let blockWidth: CGFloat
    let padWidth: CGFloat
  }

  /// 侧边数字键盘此刻该不该画（宽度另算）：偏好选了数字键盘，主键区画的是九键一族（拼音九键、九键双拼、笔画，`TouchKeyLayoutOffering.nineKeySchemes`）的字母层。英文、本地模式（画 26 键）、123 数字层和 #+= 层、注音、假名九键、手写、14/18 键和 26 键都不画。面板盖住按键时它和按键一起遮住（`setKeyRowsCovered`），不在这里判断。
  private var oneHandSidePadWanted: Bool {
    oneHandedSidePanel == .numberPad && isChineseMode && !isInLocalMode && !showsSymbols
      && TouchKeyLayoutOffering.nineKeySchemes.contains(inputScheme)
  }

  /// 按单手模式布局按键行：按键收窄到侧栏留下的宽度，靠在存储的那一侧，侧栏在另一侧。手机上「侧栏加按键」占整行；全宽的 iPad 键盘上这一块缩到手机大小贴到那一侧，空出来的一侧是空白，够宽、又画着九键一族的字母层时贴屏幕边放侧边数字键盘（键在右：面板、空白、侧栏、按键；键在左：按键、侧栏、空白、面板）。iPad 画横屏分离式键盘时单手让位（`KeyboardOneHandLayout.effective`）；存储的值无论如何都保留。工具栏、候选和盖在按键上的面板保持全宽。
  ///
  /// 模式变了键面也跟着变（`keyFaceFormFactor`），调用方随后要 `updateKeyboardLayoutIfNeeded`；`updateKeyboardLayout` 自己在排键面之前调用它。
  private func applyOneHanded() {
    // 方案在按键建好之前就已应用，那一步已经请求过布局。
    guard keyColumn.superview === oneHandRow else { return }
    let formFactor = formFactor
    // 升级前存下的单手模式只在手机形态的键盘上画过，第一次在全宽 iPad 键盘上画时按关闭处理，见 `KeyboardLayoutPreference.oneHandedDockedTabletKey`。
    if formFactor == .tablet && !splitKeyboardShown && oneHandedMode != .off {
      oneHandedMode = KeyboardLayoutPreference.adoptOneHandedOnDockedTablet()
    }
    let mode = KeyboardOneHandLayout.effective(oneHandedMode, formFactor: formFactor, split: splitKeyboardShown)
    let available = view.bounds.width - formFactor.padding.leading - formFactor.padding.trailing
    // 宽度还没定下来（第一次布局之前）时先按手机的比例排，`viewDidLayoutSubviews` 有了宽度再排一次。
    let tablet = formFactor == .tablet && mode != .off && available > 0
    let arrangement = OneHandArrangement(
      mode: mode,
      blockWidth: tablet ? KeyboardOneHandLayout.blockWidth(available: available, formFactor: formFactor) : 0,
      padWidth: tablet && oneHandSidePadWanted ? KeyboardOneHandLayout.sidePadWidth(available: available, formFactor: formFactor) : 0)
    guard arrangement != appliedOneHandArrangement else { return }
    let previousKeyFace = keyFaceFormFactor
    appliedOneHandArrangement = arrangement
    appliedOneHanded = mode
    let on = mode != .off
    let padShown = arrangement.padWidth > 0
    oneHandGutter.setKeysOnRight(mode == .right)
    let order: [UIView] = mode == .left
      ? [keyColumn, oneHandGutter, oneHandSideSpacer, oneHandSidePad]
      : [oneHandSidePad, oneHandSideSpacer, oneHandGutter, keyColumn]
    for (index, item) in order.enumerated() where oneHandRow.arrangedSubviews.firstIndex(of: item) != index {
      oneHandRow.removeArrangedSubview(item)
      oneHandRow.insertArrangedSubview(item, at: index)
    }
    // 面板和空白之间不留间距，空白另一侧照常隔 6pt 才是侧栏或按键：面板贴屏幕边，「面板加空白」正好是整行减去块再减去一个间距。
    oneHandRow.setCustomSpacing(mode == .left ? UIStackView.spacingUseDefault : 0, after: oneHandSidePad)
    oneHandRow.setCustomSpacing(mode == .left ? 0 : UIStackView.spacingUseDefault, after: oneHandSideSpacer)
    // 宽度约束在视图隐藏之前停用、在显示之后启用，这样它不会碰上 stack view 给隐藏视图的零宽约束。
    if !on || tablet { oneHandGutterWidth?.isActive = false }
    if !tablet {
      oneHandGutterTabletWidth?.isActive = false
      keyColumnTabletWidth?.isActive = false
    }
    if !padShown {
      oneHandSidePadWidth?.isActive = false
      NSLayoutConstraint.deactivate(oneHandSidePadHeights)
    }
    if oneHandGutter.isHidden == on { oneHandGutter.isHidden = !on }
    if oneHandSideSpacer.isHidden == tablet { oneHandSideSpacer.isHidden = !tablet }
    if oneHandSidePad.isHidden == padShown { oneHandSidePad.isHidden = !padShown }
    if on && !tablet { oneHandGutterWidth?.isActive = true }
    if tablet {
      let gutter = arrangement.blockWidth * KeyboardOneHandLayout.gutterRatio
      oneHandGutterTabletWidth?.constant = gutter
      keyColumnTabletWidth?.constant = max(0, arrangement.blockWidth - gutter - KeyboardOneHandLayout.gap)
      oneHandGutterTabletWidth?.isActive = true
      keyColumnTabletWidth?.isActive = true
    }
    if padShown {
      oneHandSidePadWidth?.constant = arrangement.padWidth
      oneHandSidePadWidth?.isActive = true
      NSLayoutConstraint.activate(oneHandSidePadHeights)
    }
    // 字母键的字号在按键重画配置时才按 `keyFaceFormFactor` 取（`applyLetterFont`），键面形态变了就让它们重画一次。把同一份配置原样赋回去不算变化，按键不会重画，字号停在旧键面上；重新装一个变换器才是一份新的配置，再请求一次配置更新，下一趟布局之前按新键面重画。
    if keyFaceFormFactor != previousKeyFace {
      let buttons: [UIButton] = letterButtons.map { $0.button } + letterGridButtons.map { $0.button }
      for button in buttons {
        applyLetterFont(to: button)
        button.setNeedsUpdateConfiguration()
      }
    }
    updateLetterRowInsets()
  }

  /// 存储并画出新的单手模式，与 Android 的 `toggleOneHanded` 从磁贴和侧栏触发时一样。iPad 上单手换了键面，随后按新键面重排。功能菜单开着时重画它的磁贴。
  private func setOneHanded(_ mode: KeyboardOneHandedMode) {
    oneHandedMode = mode
    KeyboardLayoutPreference.oneHanded = mode
    applyOneHanded()
    updateKeyboardLayoutIfNeeded()
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
      glideActive = true
      updateKeyHitSuppression()
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
    glideActive = false
    updateKeyHitSuppression()
    if fading { glideTrail.fadeOut() } else { glideTrail.clear() }
  }

  /// 两种根手势各自结束，不清掉另一种手势仍持有的按键抑制。
  private func updateKeyHitSuppression() {
    (keyboardRoot as? KeyAreaStackView)?.suppressesKeyHits = glideActive || cursorSwipeActive
  }

  private var cursorSwipeAllowed: Bool {
    cursorSwipeEnabled && !cursorSwipeHadMultipleTouches && !hasComposition && !isInLocalMode && !replyKeyboardShown
      && !glideTypingArmed && !glideActive
      // 假名网格的横滑是选假名，所以假名九键的字母层不挂横滑光标；它的数字九宫格和 #+= 页与别的键面一样可以。
      && !(isChineseMode && inputScheme == .japaneseNineKey && !showsSymbols)
  }

  private func cancelCursorSwipeIfDocumentChanged() {
    if cursorSwipeActive && KeyboardHostContext.documentIdentifier(for: textDocumentProxy) != cursorSwipeDocument {
      endKeyboardCursorSwipe()
    }
  }

  /// 步进器失去文档或自行取消时，同时终止识别序列，旧的 changed 不能重新启动光标移动。
  private func endKeyboardCursorSwipe(cancelGesture: Bool = true) {
    if cursorSwipeActive {
      cursorMovement.cancel()
      updateSpaceKeyTitle()
    }
    cursorSwipeActive = false
    cursorSwipeDocument = nil
    updateKeyHitSuppression()
    if cancelGesture && (keyboardCursorPan.state == .began || keyboardCursorPan.state == .changed) {
      let enabled = keyboardCursorPan.isEnabled
      keyboardCursorPan.isEnabled = false
      keyboardCursorPan.isEnabled = enabled
    }
  }

  @objc private func handleKeyboardCursorPan(_ pan: UIPanGestureRecognizer) {
    switch pan.state {
    case .began:
      guard cursorSwipeAllowed, !cursorMovement.isActive,
            let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else {
        endKeyboardCursorSwipe()
        return
      }
      cursorSwipeActive = true
      cursorSwipeDocument = document
    case .changed:
      guard cursorSwipeActive else { return }
    case .ended, .cancelled, .failed:
      endKeyboardCursorSwipe(cancelGesture: false)
      return
    default: return
    }
    handleSpaceCursorMovement(pan.state, at: pan.translation(in: view).x,
                              velocity: pan.velocity(in: view).x, space: nil)
    if cursorMovement.isActive { updateKeyHitSuppression() }
    else { endKeyboardCursorSwipe() }
  }

  /// 抬手：把这一笔交给 Engine，回应像轻点字母键那样渲染。Engine 不处理（方案或模式不对、解不出音节）时丢掉这一笔，一个键也不输入。
  private func typeGlide(_ samples: [GlideTyping.Sample], frames: [CGRect]) {
    synchronizeInputSchemePreference()
    guard glideTypingArmed, let request = GlideTyping.request(frames: frames, samples: samples) else { return }
    let snapshot = session.glide(request)
    guard snapshot.isHandled else { return }
    render(snapshot)
  }

  /// 字母键此刻是不是 26 键双拼：中文模式、双拼方案、不画 14/18 键网格、不在本地模式。
  private var typesShuangpinLetters: Bool { isChineseMode && usesShuangpin && !usesLetterGrid && !isInLocalMode }

  /// 已注册给会话的键位几何（#10018）。代次每注册一次加一，测试据此数注册了几次。
  private(set) var keyGeometryRegistration = TouchPointPolicy.Registration()

  /// 带着按下点交给引擎（`msime_client_touch_character`）的按键数，测试据此分辨一键走的是哪个入口。
  private(set) var touchCharacterCount = 0

  /// 此刻是否开着 VoiceOver：读屏的双击落在键的激活点，不是手指的落点。测试替换它来模拟读屏。
  var isVoiceOverRunning: () -> Bool = { UIAccessibility.isVoiceOverRunning }

  /// 键区里此刻 26 个字母键和显示中的 `;` 键（微软双拼、粤语双拼）的布局矩形，坐标系与按键记下的按下点相同；字母行没在显示时为 nil。
  private var currentKeyLayout: TouchPointPolicy.Layout? {
    guard let root = keyboardRoot as? KeyAreaStackView, glideLetterKeys.count == GlideTyping.letters.count,
          !letterRowViews.isEmpty, letterRowViews.allSatisfy({ !$0.isHidden }) else { return nil }
    let letters = glideLetterKeys.map { KeyAreaStackView.layoutFrame(of: $0, in: root) }
    let semicolon = microsoftFinalKey.flatMap { $0.isHidden ? nil : KeyAreaStackView.layoutFrame(of: $0, in: root) }
    return TouchPointPolicy.Layout(letters: letters, semicolon: semicolon)
  }

  /// 保证会话里注册着键区此刻的几何，返回它的代次。旋转、单手、分离、键距和行距调整、微软 `;` 显隐都会改变布局矩形，这时换一个代次重新注册；没变、会话里也还是这份几何时不发。拼不出几何或引擎拒绝时为 nil，这一键不带落点。
  private func registeredKeyGeometryID() -> UInt32? {
    guard let layout = currentKeyLayout else { return nil }
    guard keyGeometryRegistration.needsRegistering(layout, sessionID: session.keyGeometryID) else {
      return keyGeometryRegistration.id
    }
    let id = keyGeometryRegistration.advance(to: layout)
    guard let request = TouchPointPolicy.request(id: id, layout: layout), session.setKeyGeometry(request, id: id) else {
      keyGeometryRegistration.reset()
      return nil
    }
    registeredKeyGeometryRequest = request
    exportKeyGeometry()
    return id
  }

  /// 最近一次注册成功的几何 JSON，原样留给调试导出。
  private var registeredKeyGeometryRequest: Data?
  /// 上次写进导出文件的内容，没变就不重写。
  private var exportedKeyGeometryRequest: Data?

  /// 键位几何调试导出的目录：诊断日志（`diagnostic_log.server`）开着时是共享状态目录下的 `debug`，关着时为 nil（`configureDiagnosticLog` 设它，测试可以直接指向临时目录）。
  var keyGeometryExportDirectory: URL?
  /// 导出文件名，与 Android 的 `debug/key-geometry.json` 相同。
  static let keyGeometryExportFile = "key-geometry.json"

  /// 调试导出（#10018）：把键盘真正注册给引擎的几何 JSON 原样写进 `keyGeometryExportDirectory`，评测拿它替换参考布局。只在注册成功或刚打开诊断日志时写，内容没变不重写；写不了（没有完全访问权限进不了 App Group）只记一行日志，不影响打字。
  private func exportKeyGeometry() {
    guard let directory = keyGeometryExportDirectory, let request = registeredKeyGeometryRequest,
          request != exportedKeyGeometryRequest else { return }
    do {
      try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
      try request.write(to: directory.appendingPathComponent(Self.keyGeometryExportFile), options: .atomic)
      exportedKeyGeometryRequest = request
    } catch {
      DiagnosticLog.shared.write("key_geometry_export_failed")
    }
  }

  /// 键区此刻会注册给引擎的键位几何 JSON（代次记为 1），XCTest 据此在模拟器上按指定宽度和特征导出几何；键盘真正注册的那份由 `exportKeyGeometry` 写出。字母行没在显示时为 nil。
  var keyGeometryExport: Data? {
    currentKeyLayout.flatMap { TouchPointPolicy.request(id: 1, layout: $0) }
  }

  /// 给数字行、三排字母、三排符号和底部功能键那一排各插入一段中缝，默认隐藏，分离时由 `updateKeyboardLayout` 显示。
  ///
  /// 中缝要有自己的宽度，所以原来 `.fillEqually` 的排改成 `.fill` 再给字符键加上等宽约束，不分离时排出来和原来一样。字符键的中缝位置按 `KeyboardSplitLayout.leftKeyCount` 定，是固定的，不随 `;` 等键的显隐移动；底部那一排的中缝在两个空格键之间。
  private func installSplitGaps(in root: UIStackView) {
    let rows = ([numberRowView as UIView?, symbolExtraRowView as UIView?].compactMap { $0 } + letterRowViews + symbolRowViews + symbolLayerRowViews).compactMap { $0 as? UIStackView }
    for row in rows {
      let characterKeys = row.arrangedSubviews.filter { ($0 as? KeyboardKeyButton)?.isFunctionKey != true }
      guard !characterKeys.isEmpty else { continue }
      if row.distribution == .fillEqually {
        row.distribution = .fill
        for key in characterKeys.dropFirst() {
          let width = key.widthAnchor.constraint(equalTo: characterKeys[0].widthAnchor)
          // `;` 只在微软双拼和粤语双拼下显示：等宽约束低于 required，让 stack view 隐藏它时的零宽约束胜出。
          if key === microsoftFinalKey { width.priority = .init(999) }
          width.isActive = true
        }
      }
      insertSplitGap(into: row, after: characterKeys[KeyboardSplitLayout.leftKeyCount(characterKeys.count) - 1], root: root)
      // 第三排字母：手机键面没有 ，。，按看得见的七个字母另插一段中缝（见 `tabletOnlySplitGap`）。
      if let comma = letterRowPunctuationKeys.first, characterKeys.contains(where: { $0 === comma }) {
        tabletOnlySplitGap = splitGaps.last
        let letters = characterKeys.filter { key in !letterRowPunctuationKeys.contains { $0 === key } }
        insertSplitGap(into: row, after: letters[KeyboardSplitLayout.leftKeyCount(letters.count) - 1], root: root)
        phoneOnlySplitGap = splitGaps.last
      }
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
        handleSymbol(symbol)
      }
      Self.drawBareInSidebar(button)
      punctuationStack.addArrangedSubview(button)
    }
    // 「右侧大回车」去掉右列的 ！，把它接在符号栏默认标点的末尾，四个键平分符号栏（`applyNineKeyTallReturn`）；其他样式隐藏。
    let closingMark = Self.nineKeyClosingMark
    let sidebarClosing = makeKey(title: closingMark, accessibilityLabel: "符号 \(closingMark)") { [weak self] in
      guard let self else { return }
      countKeyPress(TypingKeyID.punctuation)
      handleSymbol(closingMark)
    }
    Self.drawBareInSidebar(sidebarClosing)
    sidebarClosing.accessibilityIdentifier = "nineKeySidebarClosingMark"
    sidebarClosing.isHidden = true
    nineKeySidebarClosingMark = sidebarClosing
    punctuationStack.addArrangedSubview(sidebarClosing)
    numberPadSidebar.axis = .vertical
    numberPadSidebar.spacing = 7
    let operators = numberPadOperators
    operators.axis = .vertical
    operators.spacing = 7
    let operatorScroll = CandidateScrollView()
    operatorScroll.accessibilityIdentifier = "numberPadOperatorScroll"
    operatorScroll.showsVerticalScrollIndicator = true
    operators.translatesAutoresizingMaskIntoConstraints = false
    operatorScroll.addSubview(operators)
    NSLayoutConstraint.activate([
      operators.leadingAnchor.constraint(equalTo: operatorScroll.contentLayoutGuide.leadingAnchor),
      operators.trailingAnchor.constraint(equalTo: operatorScroll.contentLayoutGuide.trailingAnchor),
      operators.topAnchor.constraint(equalTo: operatorScroll.contentLayoutGuide.topAnchor),
      operators.bottomAnchor.constraint(equalTo: operatorScroll.contentLayoutGuide.bottomAnchor),
      operators.widthAnchor.constraint(equalTo: operatorScroll.frameLayoutGuide.widthAnchor),
    ])
    numberPadSidebar.addArrangedSubview(operatorScroll)
    for (index, symbol) in NineKeySidebar.defaultDigitSymbols.enumerated() {
      operators.addArrangedSubview(makeDigitSidebarKey(symbol, index: index, custom: false))
    }
    let more = makeKey(title: "#+=", accessibilityLabel: "更多符号", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.layer)
      self?.toggleMoreSymbols()
    }
    Self.drawBareInSidebar(more)
    more.accessibilityIdentifier = "numberPadMoreSymbols"
    operators.addArrangedSubview(more)
    let symbols = makeKey(title: "符", accessibilityLabel: "符号", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.symbol)
      self?.showSymbolPanel()
    }
    symbols.configuration?.contentInsets = .zero
    symbols.accessibilityIdentifier = "numberPadSymbolPanelKey"
    Self.drawBareInSidebar(symbols)
    numberPadSidebar.addArrangedSubview(symbols)
    numberPadSymbolsKey = symbols
    customSidebarScroll.accessibilityIdentifier = "nineKeyCustomSidebar"
    customSidebarScroll.isHidden = true
    for content in [punctuationStack, customSidebarScroll, numberPadSidebar, makeSpellingStrip()] {
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
    // 九格都显示数字提示并提供长按数字；1 键短按把常用标点放进候选条（`showNineKeyQuickSymbols`），不把 1 送进拼音引擎，符号面板从底行的 符 打开。分词仍由右列中间键承担。
    for rowIndex in 0..<3 {
      let row = makeRow()
      for column in 0..<3 {
        let digit = rowIndex * 3 + column + 1
        let letters = Self.nineKeyLetters[digit]
        let button = makeKey(
          title: letters ?? Self.nineKeySymbolsFace,
          accessibilityLabel: letters.map { "\(digit) \($0)" } ?? "1 符号"
        ) { [weak self] in
          guard let self else { return }
          self.countKeyPress(self.numberPadKeyID(digit))
          if self.showsSymbols {
            self.typeNumberPadText(String(digit))
          }
          else if letters == nil && self.inputScheme != .nineKeyShuangpin { self.showNineKeyQuickSymbols() }
          else { self.handleCharacter(String(digit)) }
        }
        // 连续输入时，按压状态直接跟随手指，不在抬指后继续缩放。
        (button as? KeyboardKeyButton)?.animatesPressFeedback = false
        button.accessibilityIdentifier = "nineKey\(digit)"
        if var configuration = button.configuration {
          configuration.contentInsets = .zero
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
        let number = UILabel()
        number.text = String(digit)
        number.font = .systemFont(ofSize: 10)
        number.textColor = KeyboardTheme.current.secondary
        number.accessibilityIdentifier = "keyNumberHint"
        number.translatesAutoresizingMaskIntoConstraints = false
        number.isAccessibilityElement = false
        button.addSubview(number)
        // 数字提示画在右上角，与 26 键的符号角标同一位置和颜色（距上边 3pt、右边 5pt，皮肤的次要色，`attachCornerHintLabel`），键面文字因此居中，不再为提示下移。
        NSLayoutConstraint.activate([
          number.topAnchor.constraint(equalTo: button.topAnchor, constant: 3),
          number.trailingAnchor.constraint(equalTo: button.trailingAnchor, constant: -5),
        ])
        button.tag = digit
        let hold = UILongPressGestureRecognizer(target: self, action: #selector(handleNineKeyHold(_:)))
        // 0.5 秒与 UIKit 自己的默认值、HarmonyOS `LongPressGesture` 的默认 500ms 一致；Android 这里用系统长按时长 `ViewConfiguration.getLongPressTimeout()`，12 起默认 400ms、之前 500ms。原来的 0.3 秒比哪一端都短，主线程稍一卡顿，一次普通的点按就会被当成长按、弹出菜单而不出字。
        hold.minimumPressDuration = Self.nineKeyHoldDuration
        hold.name = "nineKeyDigitHold"
        hold.delegate = self
        button.addGestureRecognizer(hold)
        button.accessibilityHint = letters.map { "长按输入 \(digit) 或 \($0)" } ?? "长按输入 1"
        if let key = button as? KeyboardKeyButton {
          key.nineKeySwipeDigit = digit
          key.nineKeySwipeHasLetters = letters != nil
          key.nineKeySwipeHint = number
          key.nineKeySwipeMode = { [weak self] in self?.activeNineKeySwipeDigits ?? .off }
          key.onNineKeySwipe = { [weak self, weak key] gesture in
            guard let self, let key else { return }
            self.handleNineKeySwipe(gesture, from: key, digit: digit)
          }
        }
        nineKeyGridKeys.append(
          NineKeyGridKey(button: button, digit: digit, letters: letters, numberHint: number))
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
    nineKeyDeleteKey = delete
    let middle = makeKey(title: "", accessibilityLabel: "", function: true) { [weak self] in
      self?.handleNineKeyMiddleKey()
    }
    middle.configuration?.contentInsets = .zero
    middle.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    middle.accessibilityIdentifier = "nineKeyMiddleKey"
    nineKeyMiddleButton = middle
    controls.addArrangedSubview(middle)
    let closing = Self.nineKeyClosingMark
    // 写出的是此刻的键面：英文和写 ASCII 标点的方案借来的数字九宫格上是 !（`nineKeyClosingFace`，`updateKeyboardLayout` 换键面）。
    let exclamation = makeKey(title: closing, accessibilityLabel: "符号 \(closing)", function: true) { [weak self] in
      guard let self else { return }
      countKeyPress(TypingKeyID.punctuation)
      handleSymbol(nineKeyClosingFace)
    }
    exclamation.configuration?.contentInsets = .zero
    exclamation.accessibilityIdentifier = "nineKeyClosingMark"
    nineKeyClosingButton = exclamation
    controls.addArrangedSubview(exclamation)
    nineKeyContainer.addArrangedSubview(controls)
    // 符固定在第三排，运算符按同一排高滚动，避免五项挤在三排高度里。排高取右列的 ⌫：「右侧大回车」的数字层把这个 符 移到底行、在这里隐藏，它的排高约束因此略低于 required，隐藏时让给 stack view 的零高约束。
    for button in operators.arrangedSubviews {
      button.heightAnchor.constraint(equalTo: delete.heightAnchor).isActive = true
    }
    let symbolsHeight = symbols.heightAnchor.constraint(equalTo: delete.heightAnchor)
    symbolsHeight.priority = .init(999)
    symbolsHeight.isActive = true
    controls.widthAnchor.constraint(equalTo: sidebar.widthAnchor).isActive = true
    return nineKeyContainer
  }

  /// 数字九宫格左列的一项。出厂表（`custom` 为 false）照原来的运算符键：读作「运算符 +」，按数字页的路径上屏（`typeNumberPadText`）；半角语境在出厂表后追加的符号（`extra`，`NineKeySidebar.asciiDigitExtras`）走同一条路径，读作「符号 @」；自定义表的项读作「符号 …」，按 `insertSidebarLiteral` 上屏，标识是 `numberPadSidebarSymbol` 加下标。
  private func makeDigitSidebarKey(_ symbol: String, index: Int, custom: Bool, extra: Bool = false) -> UIButton {
    let button = makeKey(title: symbol, accessibilityLabel: custom || extra ? "符号 \(symbol)" : "运算符 \(symbol)") { [weak self] in
      guard let self else { return }
      countKeyPress(TypingKeyID.punctuation)
      if custom { insertSidebarLiteral(symbol, digits: true) } else { typeNumberPadText(symbol) }
    }
    Self.drawBareInSidebar(button)
    if custom { button.accessibilityIdentifier = "numberPadSidebarSymbol\(index)" }
    return button
  }

  /// 字母层自定义左列的一项：按 `insertSidebarLiteral` 上屏，标识是 `nineKeySidebarSymbol` 加下标。「……」这类两个字宽的符号缩小字号，不折行。
  private func makeLetterSidebarKey(_ symbol: String, index: Int) -> UIButton {
    let button = makeKey(title: symbol, accessibilityLabel: "符号 \(symbol)") { [weak self] in
      guard let self else { return }
      countKeyPress(TypingKeyID.punctuation)
      insertSidebarLiteral(symbol, digits: false)
    }
    Self.drawBareInSidebar(button)
    button.titleLabel?.adjustsFontSizeToFitWidth = true
    button.titleLabel?.minimumScaleFactor = 0.5
    button.accessibilityIdentifier = "nineKeySidebarSymbol\(index)"
    return button
  }

  /// 按两张符号表换左列的键，表和语境没变时不做事。字母层：空表时左列是原来的 ，。？（`punctuationStack`，右侧大回车时末尾接 ！），自定义表换成 `customSidebarScroll`。数字层：空表是原来的四个运算符，半角语境（数字九宫格右列是 ASCII `!` 的那些，`symbolLayerIsChinese` 为 false）再按 `@ : ; ( ) ' " $` 的顺序追加还没有的半角符号；自定义表换成用户的表，不追加（`TouchKeyLayoutStyle.numberPadSidebarSymbols`）。几种都在同一个滚动栈里、#+= 总在最后，每项与右列的 ⌫ 同高；切中英文、进出本地模式或换方案时随 `updateKeyboardLayout` 重建。
  private func applySidebarSymbols(_ style: TouchKeyLayoutStyle) {
    if !NineKeySidebar.same(style.nineKeySidebar, builtLetterSidebar) {
      builtLetterSidebar = style.nineKeySidebar
      customSidebarScroll.buttons.forEach { $0.removeFromSuperview() }
      customSidebarScroll.buttons = style.nineKeySidebar.enumerated().map { index, symbol in
        let button = makeLetterSidebarKey(symbol, index: index)
        customSidebarScroll.addSubview(button)
        return button
      }
      customSidebarScroll.setContentOffset(.zero, animated: false)
    }
    let custom = !style.digitSidebar.isEmpty
    let extras = !custom && !symbolLayerIsChinese
    if !NineKeySidebar.same(style.digitSidebar, builtDigitSidebar) || extras != builtDigitSidebarExtras,
       let delete = nineKeyDeleteKey {
      builtDigitSidebar = style.digitSidebar
      builtDigitSidebarExtras = extras
      for key in numberPadOperators.arrangedSubviews where key.accessibilityIdentifier != "numberPadMoreSymbols" {
        key.removeFromSuperview()
      }
      let defaultCount = NineKeySidebar.defaultDigitSymbols.count
      for (index, symbol) in style.numberPadSidebarSymbols(ascii: extras).enumerated() {
        let key = makeDigitSidebarKey(symbol, index: index, custom: custom, extra: extras && index >= defaultCount)
        numberPadOperators.insertArrangedSubview(key, at: index)
        key.heightAnchor.constraint(equalTo: delete.heightAnchor).isActive = true
      }
    }
  }

  private func applyNineKeyDigitLayer(_ digits: Bool) {
    let shuangpin = inputScheme == .nineKeyShuangpin
    for key in nineKeyGridKeys {
      if shuangpin {
        let face = nineKeyShuangpinLayout?.keys.first { $0.key == String(key.digit) }
        key.button.configuration?.title = String(key.digit)
        key.button.configuration?.subtitle = digits ? nil : face?.compactHint
        key.button.configuration?.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
          var attributes = attributes
          attributes.font = .systemFont(ofSize: 11)
          return attributes
        }
        key.button.accessibilityLabel = digits ? "数字 \(key.digit)" : (face?.accessibilityLabel ?? "数字 \(key.digit)")
        key.button.accessibilityHint = nil
        key.numberHint?.isHidden = true
      } else {
        key.button.configuration?.subtitle = nil
        key.button.configuration?.title = digits ? String(key.digit) : (key.letters ?? Self.nineKeySymbolsFace)
        key.button.accessibilityLabel = digits
          ? "数字 \(key.digit)"
          : (key.letters.map { "\(key.digit) \($0)" } ?? "1 符号")
        key.numberHint?.isHidden = digits
        key.button.accessibilityHint = digits ? nil
          : (key.letters.map { "长按输入 \(key.digit) 或 \($0)" } ?? "长按输入 1")
      }
      key.button.gestureRecognizers?.filter { $0.name == "nineKeyDigitHold" }.forEach { $0.isEnabled = !shuangpin && !digits }
    }
    let zero = nineKeyShuangpinLayout?.keys.first { $0.key == "0" }
    nineKeyZeroButton?.configuration?.titleTextAttributesTransformer = digits ? Self.keyLabelTransformer : Self.functionLabelTransformer
    nineKeyZeroButton?.accessibilityLabel = shuangpin && !digits ? (zero?.accessibilityLabel ?? "数字 0") : "数字 0"
  }

  /// 九键「右侧大回车」的回车：从右列第三格（`nineKeyClosingButton` 让出的位置）一直伸到底行，两倍高、中间无缝。它不在任何一行里，而是叠在按键列上，按右列的左右边、第三格的上边和底行的下边约束；底行在它下面留出同宽的一格（`tallReturnSlot`）。外观和行为与底行的回车相同（`returnKeys`），按下记 `Return`。只在这个样式的九键外框上显示。
  private func installTallReturnKey() {
    guard let closing = nineKeyClosingButton, let controls = nineControls, let actionRow else { return }
    let enter = makeKey(title: "换行", accessibilityLabel: "换行", function: true) { [weak self] in
      self?.countKeyPress(TypingKeyID.enter)
      self?.handleReturn()
    }
    enter.accessibilityIdentifier = "tallReturnKey"
    enter.titleLabel?.adjustsFontSizeToFitWidth = true
    enter.titleLabel?.minimumScaleFactor = 0.65
    enter.isHidden = true
    enter.translatesAutoresizingMaskIntoConstraints = false
    keyColumn.addSubview(enter)
    NSLayoutConstraint.activate([
      enter.leadingAnchor.constraint(equalTo: controls.leadingAnchor),
      enter.trailingAnchor.constraint(equalTo: controls.trailingAnchor),
      enter.topAnchor.constraint(equalTo: closing.topAnchor),
      enter.bottomAnchor.constraint(equalTo: actionRow.bottomAnchor),
    ])
    tallReturnKey = enter
  }

  /// 数字键盘「空格在右列」时右列第三格的空格：与底行空格同一个工厂（`makeSpaceKey`），轻点、拖动移光标和长按语音都相同；叠在 ！ 上，按 ！ 的四边约束，不在任何一行里。只在数字层、不是右侧大回车、选了 `rightColumn` 时显示（`applyNumberPadColumnSpace`）。
  private func installNumberPadColumnSpace() {
    guard let closing = nineKeyClosingButton else { return }
    let space = makeSpaceKey()
    space.accessibilityIdentifier = "numberPadColumnSpaceKey"
    space.isHidden = true
    space.translatesAutoresizingMaskIntoConstraints = false
    // 它的四边只跟着 ！ 走：键面（麦克风加「空格」）比右列宽时也不能反过来撑开右列，所以不要固有尺寸的抗压缩和抗拉伸。
    for axis in [NSLayoutConstraint.Axis.horizontal, .vertical] {
      space.setContentCompressionResistancePriority(.init(1), for: axis)
      space.setContentHuggingPriority(.init(1), for: axis)
    }
    keyColumn.addSubview(space)
    NSLayoutConstraint.activate([
      space.leadingAnchor.constraint(equalTo: closing.leadingAnchor),
      space.trailingAnchor.constraint(equalTo: closing.trailingAnchor),
      space.topAnchor.constraint(equalTo: closing.topAnchor),
      space.bottomAnchor.constraint(equalTo: closing.bottomAnchor),
    ])
    numberPadColumnSpace = space
  }

  /// 换上或撤下数字层右列的空格：显示时底行的空格隐藏、右列的 ！ 让位（留着占位、不画不接触摸）。
  private func applyNumberPadColumnSpace(_ shown: Bool) {
    if numberPadColumnSpace?.isHidden == shown { numberPadColumnSpace?.isHidden = !shown }
    if spaceButton?.isHidden != shown { spaceButton?.isHidden = shown }
  }

  /// 右列第三格的 ！ 让位：「右侧大回车」伸下来的回车或数字层右列的空格盖在它上面时，留着占位、不画不接触摸。
  private func applyNineKeyClosingCovered(_ covered: Bool) {
    guard let closing = nineKeyClosingButton else { return }
    closing.alpha = covered ? 0 : 1
    closing.isUserInteractionEnabled = !covered
    closing.isAccessibilityElement = !covered
  }

  /// 换上或撤下九键「右侧大回车」的外框：右列第三格的 ！ 让位给伸下来的回车（`applyNineKeyClosingCovered`），！ 接到符号栏默认标点的末尾。底行由 `TouchBottomRowLayout.nineKeyTallReturn` 排，数字层符号栏里的 符 随底行一起挪走（`updateKeyboardLayout` 按底行有没有 符 决定）。
  private func applyNineKeyTallReturn(_ shown: Bool) {
    if tallReturnKey?.isHidden == shown { tallReturnKey?.isHidden = !shown }
    if nineKeySidebarClosingMark?.isHidden == shown { nineKeySidebarClosingMark?.isHidden = !shown }
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

  /// 拼音网格的 1 键：候选条换成当前模式的常用标点（与 ， 长按的那一组相同），点选即上屏；组字中点选与按侧栏标点一样，先上屏首选再写标点。按其他键时 `render` 换回 Engine 的候选。
  private func showNineKeyQuickSymbols() {
    playInputClick()
    nineKeyQuickSymbols = quickPunctuationSymbols
    updateCandidateStrip(preedit: visiblePreedit, candidates: nineKeyQuickSymbols)
  }

  private func handleNineKeyMiddleKey() {
    switch nineKeyMiddleKey {
    case .separator:
      countKeyPress(TypingKeyID.character("'"))
      handleCharacter("'")
    case .period:
      countKeyPress(TypingKeyID.character("."))
      typeNumberPadText(".")
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
    guard gesture.state == .began, !showsSymbols, inputScheme != .nineKeyShuangpin, let key = gesture.view as? UIButton,
      (1...9).contains(key.tag) else { return }
    // 长按替代点按，只统计一次；菜单选项经字面输入路径上屏，1 不进入拼音引擎。
    countKeyPress(TypingKeyID.nineKey(key.tag))
    showKeyHoldOptions(from: key, options: Self.nineKeyHoldOptions(key.tag))
  }

  /// 九宫格的键此刻按哪种「九键滑动输入数字」判定：只在拼音九键的字母层生效；数字层（包括别的键面借来的数字九宫格）、九键双拼、长按菜单已经弹出、横滑移光标接管了触摸、读屏开着时一律按关闭，点按和长按照旧。
  var activeNineKeySwipeDigits: NineKeySwipeDigits {
    guard nineKeySwipeDigits != .off, !showsSymbols, inputScheme == .nineKey, nineKeyHoldPopup == nil,
          !cursorSwipeActive, !isVoiceOverRunning() else { return .off }
    return nineKeySwipeDigits
  }

  /// 九键滑动的结果（#10229）。沿设定方向松手：记一次这个键（与点按同一个统计 id），按长按菜单里选数字的同一条路径上屏（先结束组字再原样上屏，按键反馈也在那里出）。反方向滑过：与长按一样记一次这个键，弹出同样的数字和字母，之后从菜单里选的不再记。
  private func handleNineKeySwipe(_ gesture: NineKeySwipePolicy.Gesture, from key: UIButton, digit: Int) {
    switch gesture {
    case .digit:
      countKeyPress(numberPadKeyID(digit))
      commitNineKeyHoldOption(String(digit))
    case .letters:
      countKeyPress(TypingKeyID.nineKey(digit))
      showKeyHoldOptions(from: key, options: Self.nineKeyHoldOptions(digit))
    case .none:
      break
    }
  }

  static func nineKeyHoldOptions(_ digit: Int) -> [String] {
    guard (1...9).contains(digit) else { return [] }
    return [String(digit)] + (nineKeyLetters[digit] ?? "").lowercased().map(String.init)
  }

  /// 长按菜单：九键 1 弹出数字，2–9 弹出数字和字母，14 键弹出这一键的字母。选中的一项先结束组字再原样上屏（`commitNineKeyHoldOption`）。
  private func showKeyHoldOptions(from key: UIButton, options optionTexts: [String]) {
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

    for option in optionTexts {
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
    let container = CandidateCanvasHostView()
    container.minimumContentHeight = 28
    compositionContainer = container
    container.accessibilityIdentifier = "candidateStrip"
    container.backgroundColor = Self.stripBackground(KeyboardTheme.current, palette: nil)

    // 读音默认并入候选行，占它最左边的一段（最多三成宽，长了截掉尾部），换来顶栏少一行 14pt；代价是候选少了这一段宽度。全拼 14 键仍单独一行，见 `separateReadingRow`。
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
    // 读音并入候选行时它和候选滚动区排在同一行，只占读音自己的宽度，余下的都给候选。
    preeditButton.setContentHuggingPriority(.defaultHigh, for: .horizontal)
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
    let spellingStripEdges = installReadingSpellingStrip(in: compositionRow)

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
      equalToConstant: Self.readingRowHeight(preeditScale: preeditFontScale, barScale: candidateBarScale))
    readingRowHeightConstraint = compositionHeight
    // 略低于 required：收起的顶栏（显示方式「隐藏」）比读音行还矮，这时由隐藏的候选行让步，而不是破坏这一行的高度。
    let contentBottom = content.bottomAnchor.constraint(equalTo: container.candidateContentGuide.bottomAnchor)
    contentBottom.priority = .init(999)
    // 读音并入候选行时与候选行同高，收起的顶栏里同样由它让步。
    let inlineReadingBottom = compositionRow.bottomAnchor.constraint(equalTo: container.candidateContentGuide.bottomAnchor)
    inlineReadingBottom.priority = .init(999)
    let preeditGap = content.topAnchor.constraint(equalTo: compositionRow.bottomAnchor)
    candidatePreeditGapConstraint = preeditGap
    separateReadingConstraints = [
      compositionRow.trailingAnchor.constraint(equalTo: container.candidateContentGuide.trailingAnchor, constant: -2),
      compositionHeight,
      preeditButton.trailingAnchor.constraint(lessThanOrEqualTo: compositionRow.trailingAnchor),
      content.leadingAnchor.constraint(equalTo: container.candidateContentGuide.leadingAnchor, constant: 2),
      preeditGap,
    ] + spellingStripEdges
    inlineReadingConstraints = [
      inlineReadingBottom,
      preeditButton.trailingAnchor.constraint(equalTo: compositionRow.trailingAnchor),
      compositionRow.widthAnchor.constraint(lessThanOrEqualTo: container.widthAnchor, multiplier: Self.inlineReadingMaximumShare),
      content.leadingAnchor.constraint(equalTo: compositionRow.trailingAnchor),
      content.topAnchor.constraint(equalTo: container.candidateContentGuide.topAnchor),
    ]
    inlineReadingCollapse = compositionRow.widthAnchor.constraint(equalToConstant: 0)
    // 设计稿里这一行两端各有 2pt 内边距（dc.html `padding: 0 2px`），工具栏和候选列都一样。
    NSLayoutConstraint.activate([
      stripHeight,
      compositionRow.leadingAnchor.constraint(equalTo: container.candidateContentGuide.leadingAnchor, constant: 2),
      compositionRow.topAnchor.constraint(equalTo: container.candidateContentGuide.topAnchor),
      preeditButton.leadingAnchor.constraint(equalTo: compositionRow.leadingAnchor),
      preeditButton.centerYAnchor.constraint(equalTo: compositionRow.centerYAnchor),
      content.trailingAnchor.constraint(equalTo: container.candidateContentGuide.trailingAnchor, constant: -2),
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
    NSLayoutConstraint.activate(separateReadingRow ? separateReadingConstraints : inlineReadingConstraints)
    return container
  }

  /// 读音并入候选行时最多占这一行的这一份宽度，更长的读音截掉尾部。
  static let inlineReadingMaximumShare: CGFloat = 0.3

  /// 按当前方案把读音放到单独一行或候选行最左边，顶栏和键盘高度跟着变；顶栏还没建好时（`viewDidLoad` 里先读方案）什么都不做，建顶栏时直接按当前方案装约束。
  private func applyReadingRowPlacement() {
    guard let collapse = inlineReadingCollapse else { return }
    let separate = separateReadingRow
    collapse.isActive = false
    NSLayoutConstraint.deactivate(separate ? inlineReadingConstraints : separateReadingConstraints)
    NSLayoutConstraint.activate(separate ? separateReadingConstraints : inlineReadingConstraints)
    topRowHeightConstraint?.constant = shownTopRowHeight
    applyKeyboardMetrics()
    updatePreferredKeyboardHeight()
    updatePreeditButton()
    updateCandidateCanvasMetrics()
  }

  /// 14 键的拼音选择条：读音右边、横向滚动的一行按钮，内容与九键左列相同（完整音节，再是下一键上能作声母的大写字母）。它只占读音行，不盖住下面的候选行，三端放在同一个位置。
  /// 返回把选择条左右两端接到读音行上的两条约束，由调用方随读音单独成行时才启用。
  private func installReadingSpellingStrip(in row: UIView) -> [NSLayoutConstraint] {
    readingSpellingScrollView.accessibilityIdentifier = "readingSpellingStrip"
    readingSpellingScrollView.showsHorizontalScrollIndicator = false
    readingSpellingScrollView.disableEdgeEffects()
    readingSpellingScrollView.isHidden = true
    readingSpellingScrollView.translatesAutoresizingMaskIntoConstraints = false
    readingSpellingStack.axis = .horizontal
    readingSpellingStack.spacing = 4
    readingSpellingStack.translatesAutoresizingMaskIntoConstraints = false
    readingSpellingScrollView.addSubview(readingSpellingStack)
    row.addSubview(readingSpellingScrollView)
    readingSpellingMinimumWidth = readingSpellingScrollView.widthAnchor.constraint(
      greaterThanOrEqualTo: row.widthAnchor, multiplier: Self.readingSpellingMinimumShare)
    NSLayoutConstraint.activate([
      readingSpellingScrollView.topAnchor.constraint(equalTo: row.topAnchor),
      readingSpellingScrollView.bottomAnchor.constraint(equalTo: row.bottomAnchor),
      readingSpellingStack.leadingAnchor.constraint(equalTo: readingSpellingScrollView.contentLayoutGuide.leadingAnchor),
      readingSpellingStack.trailingAnchor.constraint(equalTo: readingSpellingScrollView.contentLayoutGuide.trailingAnchor),
      readingSpellingStack.topAnchor.constraint(equalTo: readingSpellingScrollView.contentLayoutGuide.topAnchor),
      readingSpellingStack.bottomAnchor.constraint(equalTo: readingSpellingScrollView.contentLayoutGuide.bottomAnchor),
      readingSpellingStack.heightAnchor.constraint(equalTo: readingSpellingScrollView.frameLayoutGuide.heightAnchor),
    ])
    return [
      readingSpellingScrollView.leadingAnchor.constraint(equalTo: preeditButton.trailingAnchor, constant: 4),
      readingSpellingScrollView.trailingAnchor.constraint(equalTo: row.trailingAnchor),
    ]
  }

  /// 拼音选择条显示时至少占读音行的这一份宽度；读音更长时由读音截断让出来。
  static let readingSpellingMinimumShare: CGFloat = 0.5

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
    // 26 键「长空格」的符号入口紧跟品牌键，平时隐藏（`updateToolbarSymbolsEntry`）。
    symbolsShortcut.isHidden = true
    let shortcuts: [UIButton] = [
      moreShortcut, symbolsShortcut, emojiShortcut, phrasesShortcut, clipboardShortcut, skinShortcut, schemeButton,
      aiShortcut, characterSetShortcut, fullwidthShortcut, punctuationShortcut,
      replyShortcut, scriptShortcut, dismissShortcut,
    ]
    for button in shortcuts {
      shortcutBar.addArrangedSubview(button)
      // 长按任何一个按钮（含品牌键和收起键）打开 App 的「键盘工具栏」设置页，在那里选择显示哪些按钮，与 Android 的 `bindEditor` 一致。长按会取消这次触摸，所以不会再触发按钮自己的点按动作。
      let hold = UILongPressGestureRecognizer(target: self, action: #selector(handleToolbarEditorHold(_:)))
      hold.name = "toolbarEditorHold"
      hold.minimumPressDuration = Self.nineKeyHoldDuration
      hold.cancelsTouchesInView = true
      button.addGestureRecognizer(hold)
      button.accessibilityCustomActions = [
        UIAccessibilityCustomAction(name: "编辑工具栏") { [weak self] _ in
          self?.openApp(KeyboardAppLauncher.toolbarSettingsURL)
          return true
        }
      ]
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
      startVoiceFromSpace(held: false)
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
    symbolsShortcut.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      if symbolPanel != nil { closeKeyboardPicker(); return }
      countKeyPress(TypingKeyID.symbol)
      showSymbolPanel()
    }, for: .primaryActionTriggered)
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
    // iOS 的语音入口是长按空格，工具栏不放语音按钮：共享文档里的 `touch_voice_shortcut` 是 Android 的「启动方式」，iOS 设置里没有对应的开关，按它显示就成了关不掉的按钮。关掉长按空格时从功能菜单的「语音输入」开始说话。简繁是设一次就很少再动的开关，和其他开关一起放在功能菜单里。
    configure(scriptShortcut, title: nil, symbol: "waveform", label: "语音输入", id: "layoutVoiceShortcut")
    scriptShortcut.isEnabled = true
    scriptShortcut.accessibilityValue = nil
    scriptShortcut.isHidden = true
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
    let hasTools = optionalShortcuts.contains { !$0.isHidden }
    if hasTools != toolbarHasTools {
      toolbarHasTools = hasTools
      // 顶栏建好之后，最后一个可选按钮被关掉或重新打开一个时，空闲工具栏行跟着消失或出现；建顶栏时由之后的 `renderCandidateStrip` 读它。
      if topRowHeightConstraint != nil { renderCandidateStrip() }
    }
    // 26 键「长空格」把 符 挪到了这里；显示方式为「隐藏」时这一行仍然显示，但只放品牌、符号和收起，切到英文 26 键时只剩品牌和收起，与 Android、鸿蒙一致。
    configure(symbolsShortcut, title: "符", symbol: nil, label: "符号", id: "symbolsShortcut")
    symbolsShortcut.isHidden = !toolbarSymbolsEntryShown
    if toolbarRowKept && toolbarHidden {
      for button in optionalShortcuts where !button.isHidden { button.isHidden = true }
    }
    updateToolbarActiveState()
    updateMorePickerPage()
  }

  /// 长按工具栏按钮（见 `installShortcutBar`）：响一下按键反馈，再打开 App 的「键盘工具栏」设置页，对应 Android 的 `ImeToolbar.openEditor`。
  @objc private func handleToolbarEditorHold(_ gesture: UILongPressGestureRecognizer) {
    guard gesture.state == .began else { return }
    playInputClick()
    openApp(KeyboardAppLauncher.toolbarSettingsURL)
  }

  /// 工具栏上可以在「键盘工具栏」设置里打开或关掉的按钮，即品牌键和收起键之外的全部按钮。
  private var optionalShortcuts: [UIButton] {
    [emojiShortcut, phrasesShortcut, clipboardShortcut, skinShortcut, schemeButton,
     aiShortcut, characterSetShortcut, fullwidthShortcut, punctuationShortcut, replyShortcut, scriptShortcut]
  }

  /// 空闲时不显示工具栏内容：显示方式为「隐藏」，或者可选按钮一个都没打开，只剩品牌键和收起键，与 Android 的 `toolbarRowHidden` 一致。iOS 保留顶栏高度，组字时候选行在同一位置出现。26 键「长空格」把 符 挪到工具栏后，这一行总要显示（`toolbarRowKept`），同一块键盘切到英文 26 键时也显示品牌和收起，与 Android、鸿蒙相同。
  private var toolbarRowHidden: Bool { (toolbarHidden || !toolbarHasTools) && !toolbarRowKept }

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
        self?.toggleHandwritingScheme()
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
        KeyboardFeedbackPreference.set(!KeyboardFeedbackPreference.soundEnabled,
                                       forKey: KeyboardFeedbackPreference.soundKey)
        self?.synchronizeKeySoundPack()
        if KeyboardFeedbackPreference.soundEnabled {
          // 系统只给开了完全访问的键盘播放按键音（`playInputClick` 和音效包都是），没有权限时开关打开了也一声不响，要当场说明原因。
          if self?.hasFullAccess == false { self?.showDiagnostic("按键音需要开启键盘的“允许完全访问”。") }
          UIDevice.current.playInputClick()
        }
        self?.updateShortcutButtons()
      },
    ]
    if KeyboardFeedbackPreference.hapticsAvailable {
      tools.append(KeyboardTool(id: "vibration", title: "振动", accessibilityLabel: "按键振动", face: .icon(.vibration),
                                isToggle: true, selected: KeyboardFeedbackPreference.hapticsEnabled) { [weak self] in
        KeyboardFeedbackPreference.set(!KeyboardFeedbackPreference.hapticsEnabled,
                                       forKey: KeyboardFeedbackPreference.hapticsKey)
        if let self, hapticsEnabled {
          hapticStrength.impact(keyFeedback)
          prepareKeyFeedback()
        }
        self?.updateShortcutButtons()
      })
    }
    tools += [
      // 与 Android 的磁贴一样：轻点在关闭和靠右之间切换，长按换边。磁贴显示存储的模式；全宽的 iPad 键盘也画单手模式（手机大小的一块贴到一侧），只在画横屏分离式键盘时让位，所以磁贴不再变暗。
      KeyboardTool(id: "oneHand", title: "单手模式", face: .icon(.oneHand), isToggle: true, selected: oneHandedMode != .off) { [weak self] in
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
      KeyboardTool(id: "voiceInput", title: "语音输入", face: .icon(.voiceResult)) { [weak self] in
        self?.startVoiceFromSpace(held: false)
      },
    ]
    if (try? VoiceTextHandoffStore().read()) != nil {
      tools.append(KeyboardTool(id: "voiceResult", title: "插入语音文字", face: .icon(.voiceResult)) { [weak self] in
        self?.closeKeyboardPicker(); self?.showKeyboardVoice()
      })
    }
    if KeyboardFeedbackPreference.hapticsAvailable {
      let strength = KeyboardFeedbackPreference.hapticStrength
      tools.append(KeyboardTool(id: "vibrationStrength", title: "振动强度 \(strength.title)", accessibilityLabel: "振动强度",
                                face: .icon(.vibrationStrength), enabled: KeyboardFeedbackPreference.hapticsEnabled,
                                caption: strength.title) { [weak self] in
        guard let self else { return }
        let strengths = KeyboardHapticStrength.allCases
        let current = strengths.firstIndex(of: KeyboardFeedbackPreference.hapticStrength) ?? 0
        let next = strengths[(current + 1) % strengths.count]
        KeyboardFeedbackPreference.set(next.rawValue, forKey: KeyboardFeedbackPreference.strengthKey)
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
      KeyboardTool(id: "promotion", title: "宣传", face: .glyph("吆")) { [weak self] in
        guard let self else { return }
        closeKeyboardPicker()
        if hasComposition { render(session.finishComposition()) }
        insertOwnText(PromotionText.text, source: .local)
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
    KeyboardAppLauncher.open(url, from: self) { [weak self] success in
      guard !success else { return }
      DispatchQueue.main.async { [weak self] in
        self?.showDiagnostic("未能打开水杉，请手动打开应用。")
      }
    }
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
    spellingScrollView.accessibilityIdentifier = "nineKeySpellingStrip"
    spellingScrollView.disableEdgeEffects()
    return spellingScrollView
  }

  /// 拼音选择条：九键画在网格左侧的侧栏里（竖排），全拼 14 键画在读音行右侧（横排）。两处按钮各自复用，只有当前布局的那一处有内容。
  private func updateSpellingStrip() {
    // 布局依赖仍然每次核对，但拼音条本身在连续输入中通常没有任何变化；重复给每个按钮写配置会把布局工作塞进每个按键的主线程路径。
    updateKeyboardLayoutIfNeeded()
    let spellings = currentNineKeySpellings
    let inReadingRow = readingRowCarriesSpellings
    let inputs = SpellingStripInputs(spellings: spellings, inReadingRow: inReadingRow)
    guard appliedSpellingStripInputs != inputs else { return }
    appliedSpellingStripInputs = inputs
    fillSpellingButtons(&spellingButtons, in: nil, spellings: inReadingRow ? [] : spellings, readingRow: false)
    fillSpellingButtons(&readingSpellingButtons, in: readingSpellingStack, spellings: inReadingRow ? spellings : [],
                        readingRow: true)
    spellingScrollView.buttons = spellingButtons
    let readingStripShown = inReadingRow && !spellings.isEmpty
    if readingSpellingScrollView.isHidden == readingStripShown { readingSpellingScrollView.isHidden = !readingStripShown }
    readingSpellingMinimumWidth?.isActive = readingStripShown
    if spellingScrollView.contentOffset != .zero { spellingScrollView.setContentOffset(.zero, animated: false) }
    if readingSpellingScrollView.contentOffset != .zero { readingSpellingScrollView.setContentOffset(.zero, animated: false) }
  }

  /// 拼音选择条是否在读音行里：14 键和 18 键是，九键的在侧栏里。
  private var readingRowCarriesSpellings: Bool { typesLetterGrid }

  /// 九键侧栏此刻画拼音选择条，而不是标点列。14 键的选择条在读音行里，不占这个侧栏。
  private var sidebarCarriesSpellings: Bool { !showsSymbols && !readingRowCarriesSpellings && !currentNineKeySpellings.isEmpty }

  private func fillSpellingButtons(_ buttons: inout [UIButton], in stack: UIStackView?, spellings: [String], readingRow: Bool) {
    while buttons.count < spellings.count {
      let button = UIButton(type: .system)
      button.addAction(UIAction { [weak self, weak button] _ in
        guard let self, let button else { return }
        self.playInputClick()
        self.chooseVisibleNineKeySpelling(at: UInt(button.tag))
      }, for: .primaryActionTriggered)
      buttons.append(button)
      if let stack { stack.addArrangedSubview(button) } else { spellingScrollView.addSubview(button) }
    }
    for (index, button) in buttons.enumerated() {
      guard spellings.indices.contains(index) else {
        button.isHidden = true
        continue
      }
      let spelling = spellings[index]
      if button.isHidden { button.isHidden = false }
      // 列表变化不代表每一格都变化；复用同位置的拼写，避免整列重新排版。皮肤由统一的换肤路径更新。
      guard button.configuration?.title != spelling else { continue }
      // 与共用侧栏的标点一样，只画文字，强调色留给读音和首选词块。
      // 读音行里的拼音条同样只画文字，字号比读音大一号，左右留出间距好点中，高度就是读音行的高度。
      var configuration = UIButton.Configuration.plain()
      configuration.title = spelling
      configuration.titleLineBreakMode = .byClipping
      configuration.contentInsets = readingRow
        ? NSDirectionalEdgeInsets(top: 0, leading: 6, bottom: 0, trailing: 6)
        : NSDirectionalEdgeInsets(top: 6, leading: 2, bottom: 6, trailing: 2)
      let size: CGFloat = readingRow ? Self.preeditFontSize + 1 : 14
      configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
        var attributes = attributes
        attributes.font = .systemFont(ofSize: size)
        return attributes
      }
      configuration.baseForegroundColor = KeyboardTheme.current.keyForeground
      configuration.background.backgroundColor = .clear
      configuration.background.customView = nil
      configuration.background.strokeWidth = 0
      button.configuration = configuration
      button.layer.shadowOpacity = 0
      button.tag = index
      button.isHidden = false
      button.accessibilityLabel = KeyboardNineKeyPanelColumns.spellingAccessibilityLabel(spelling)
      button.accessibilityIdentifier = (readingRow ? "readingSpelling_" : "nineKeySpelling_") + spelling
    }
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

  /// 14 键或 18 键的一排（`FourteenKeyLayout`、`EighteenKeyLayout`）。点一下把这一组的组码交给引擎（`handleLetterGridKey`），长按弹出这一键的字母；18 键两个字母的键还挂着双拼横滑（`handleEighteenKeySwipe`）。键都用 `makeKey` 建，不进 `letterButtons` 和滑行的 `glideLetterKeys`，所以 Shift 的大小写、双拼键位提示、角标滑动和滑行输入都碰不到它们。第三排两端是分词键和 ⌫。14 键第二排不缩进，两端键与 26 键的 ⇧、⌫ 同宽；18 键第一排和第三排都是七个等宽的键（第三排两端键与字母键同宽），第二排六个键平分整行，A 与 Q 左对齐、L 与 P 右对齐，与 Android、鸿蒙相同。
  private func makeLetterGridRow(_ keys: [FourteenKeyLayout.Key], grid: LetterKeyGrid, includesEdges: Bool) -> UIStackView {
    let prefix = grid == .eighteenKey ? "eighteenKey" : "fourteenKey"
    let row = makeRow()
    row.accessibilityIdentifier = "\(prefix)Row"
    for key in keys {
      let button = makeKey(title: key.face, accessibilityLabel: key.accessibilityLabel) { [weak self] in
        self?.countKeyPress(key.keyID)
        self?.handleLetterGridKey(key)
      }
      button.accessibilityIdentifier = "\(prefix)\(key.face)"
      // 不画按键预览气泡，与九键和 Android、鸿蒙的 14 键相同：一键是一组字母，气泡里放大的键面说不出这一下打的是哪个字母。
      applyLetterFont(to: button)
      if grid == .eighteenKey {
        // 七个键一排，窄屏上两个字母的键面贴着键边；与 Android 的 12–22 自适应字号一样只缩小、不换行。
        button.configuration?.contentInsets = .zero
        button.titleLabel?.adjustsFontSizeToFitWidth = true
        button.titleLabel?.minimumScaleFactor = 12 / 22
      }
      if !key.holdLetters.isEmpty {
        let hold = UILongPressGestureRecognizer(target: self, action: #selector(handleFourteenKeyHold(_:)))
        // 与九键的长按同一个时长，理由见 `makeNineKeyLayout`。
        hold.minimumPressDuration = Self.nineKeyHoldDuration
        button.addGestureRecognizer(hold)
        button.accessibilityHint = "长按输入 " + key.holdLetters.joined(separator: " 或 ")
      }
      // 横滑总是挂上，接手时再看当前方案（`gestureRecognizerShouldBegin`）：键面先于方案配置建好，切到双拼后不必重建按键。
      if grid == .eighteenKey && key.letters.count == 2 {
        let swipe = EighteenKeySwipeGestureRecognizer(letters: key.letters, target: self,
                                                      action: #selector(handleEighteenKeySwipe(_:)))
        swipe.delegate = self
        button.addGestureRecognizer(swipe)
      }
      letterGridButtons.append((button, key))
      row.addArrangedSubview(button)
    }
    guard includesEdges else { return row }
    let separator = makeKey(title: "", accessibilityLabel: "", function: true) { [weak self] in
      self?.handleFourteenKeySeparator()
    }
    separator.configuration?.contentInsets = .zero
    separator.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    separator.accessibilityIdentifier = "\(prefix)Separator"
    letterGridSeparatorButtons.append(separator)
    row.insertArrangedSubview(separator, at: 0)
    updateFourteenKeySeparator()
    let delete = makeDeleteKey()
    delete.accessibilityIdentifier = "\(prefix)Delete"
    row.addArrangedSubview(delete)
    // 18 键第三排的七个键按 `fillEqually` 等分，与上面两排的键同宽。
    guard grid == .fourteenKey else { return row }
    row.distribution = .fill
    let letters = Array(row.arrangedSubviews.dropFirst().dropLast())
    NSLayoutConstraint.activate(letters.dropFirst().map { $0.widthAnchor.constraint(equalTo: letters[0].widthAnchor) })
    fourteenKeyEdgeWidths = [separator, delete].map { edge in
      (phone: edge.widthAnchor.constraint(equalToConstant: Self.phoneLetterEdgeWidth),
       tablet: edge.widthAnchor.constraint(equalTo: row.widthAnchor, multiplier: Self.tabletFourteenKeyEdgeRatio))
    }
    return row
  }

  /// iPad 上 14 键第三排两端键占这一排的比例：26 键第三排是 ⇧、七个字母、，。、⇧，两个 ⇧ 各 `TabletLetterLayout.shiftWeight` 个键宽，一个 ⇧ 占这一排的这么多。
  static let tabletFourteenKeyEdgeRatio: CGFloat = TabletLetterLayout.shiftWeight / (9 + 2 * TabletLetterLayout.shiftWeight)

  /// 字母网格的一键：把这一组的组码交给引擎。设置在键盘开着时被改掉、方案已经不是这种网格时，这次过期的点按什么也不做，组码不会落进别的方案或另一种网格。
  private func handleLetterGridKey(_ key: FourteenKeyLayout.Key) {
    playInputClick()
    guard isChineseMode else { return }
    synchronizeInputSchemePreference()
    guard typesLetterGrid, letterGrid == key.grid else { return }
    render(session.gridKey(key.code))
  }

  /// 14 键和 18 键第三排的分词键，照搬九键的 1 键：组字时送 `'`，在已打的组码末尾定一个音节分界，只定在哪里断、不定是哪个拼音；空闲时打开符号面板。五笔 14 键没有音节可分，引擎也不收 `'`，这个键总是打开符号面板。
  private func handleFourteenKeySeparator() {
    // 一个键一个键位 id：不论组字与否都记作符号键，与 Android、鸿蒙的 14 键相同。
    countKeyPress(TypingKeyID.symbol)
    if separatorSplitsSyllables {
      handleCharacter("'")
    } else {
      showSymbolPanel()
    }
  }

  /// 第三排左端键此刻是不是「分词」：拼音的字母网格正在组字时是，空闲时和五笔 14 键是「符」。
  private var separatorSplitsSyllables: Bool { hasComposition && !typesWubiGrid }

  /// 分词键的键面跟着组字状态在「符」和「分词」之间换，render 时调用。
  private func updateFourteenKeySeparator() {
    let splits = separatorSplitsSyllables
    let face = splits ? "分词" : "符"
    for button in letterGridSeparatorButtons {
      if button.configuration?.title != face { button.configuration?.title = face }
      button.accessibilityLabel = splits ? "拼音分词" : "符号"
    }
  }

  /// 长按 14 键或 18 键的一键：弹出这一键的字母，选中后与九键一样先结束组字，再上屏这个字母。长按取代这次点按，所以按键在这里记一次，菜单里选的那一项不再记。
  @objc private func handleFourteenKeyHold(_ gesture: UILongPressGestureRecognizer) {
    guard gesture.state == .began, let button = gesture.view as? UIButton,
          let key = letterGridButtons.first(where: { $0.button === button })?.key, !key.holdLetters.isEmpty else { return }
    countKeyPress(key.keyID)
    showKeyHoldOptions(from: button, options: key.holdLetters)
  }

  /// 18 键双拼的横滑：拖动时键面换成选中的字母，松手时把它作为精确组码（大写）交给当前双拼网格；回到起点附近或被取消时什么也不输入。这一下记在这个键上，与点按同一个统计 id。
  @objc private func handleEighteenKeySwipe(_ gesture: EighteenKeySwipeGestureRecognizer) {
    guard let button = gesture.view as? UIButton,
          let key = letterGridButtons.first(where: { $0.button === button })?.key else { return }
    switch gesture.state {
    case .began, .changed:
      let face = gesture.selected.map(String.init) ?? key.face
      if button.configuration?.title != face { button.configuration?.title = face }
    case .ended:
      button.configuration?.title = key.face
      guard let letter = gesture.selected else { return }
      countKeyPress(key.keyID)
      playInputClick()
      guard isChineseMode else { return }
      synchronizeInputSchemePreference()
      guard typesEighteenKeyShuangpin else { return }
      render(session.gridKey(String(letter)))
    default:
      button.configuration?.title = key.face
    }
  }

  /// 中文模式的全拼或双拼 14 键、18 键和五笔 14 键把触摸交给引擎网格；英文模式和本地输入模式画 26 键字母。
  private var typesLetterGrid: Bool { isChineseMode && usesLetterGrid && !isInLocalMode }

  /// 五笔 14 键：组码由五笔方案按组展开码表，没有拼音选择，候选照五笔展开；组码字母（`editing_text`）不显示。
  private var typesWubiGrid: Bool { typesLetterGrid && inputScheme == .wubi }

  /// 18 键双拼：两个字母的键可以横滑选字母。全拼 18 键只点按，与 Android 相同。
  private var typesEighteenKeyShuangpin: Bool { typesLetterGrid && letterGrid == .eighteenKey && usesShuangpin }

  private func makeLetterRow(_ letters: [Character], includesShift: Bool) -> UIStackView {
    let row = makeRow()
    if includesShift {
      let button = makeIconKey(Self.shiftIcon, accessibilityLabel: "大写") { [weak self] in
        self?.handleLetterShift(keyID: TypingKeyID.shift)
      }
      button.accessibilityIdentifier = "shiftButton"
      shiftButton = button
      row.addArrangedSubview(button)
    }
    for letter in letters {
      let text = String(letter)
      let button = makeTouchKey(title: text, accessibilityLabel: text.uppercased()) { [weak self] key in
        self?.countKeyPress(TypingKeyID.character(text))
        self?.handleCharacter(text, touchDown: key?.takeTouchDownLocation())
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
        self?.handleLetterShift(keyID: TypingKeyID.shiftRight)
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
      // 读屏标签随方案变化，由 `updateLetterCaseControls` 按当前方案写入。
      let key = makeTouchKey(title: ";", accessibilityLabel: inputScheme.semicolonFinalKeyAccessibilityLabel) { [weak self] key in
        self?.countKeyPress(TypingKeyID.character(";"))
        self?.handleCharacter(";", touchDown: key?.takeTouchDownLocation())
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
      // 字母保持等宽，`;`（微软双拼、粤语双拼）以低于 required 的约束与它们等宽，隐藏时 stack view 的零宽约束胜出；回车宽 1.75 个键，同样低于 required，让手机能把它隐藏。
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
      let size = Self.letterFontSize(self?.keyFaceFormFactor ?? .phone)
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

  /// 补充行只在源字母页有数字行时显示，使用相同等高和分体规则。
  private func makeSymbolExtraRow() -> UIStackView {
    let row = makeRow()
    row.accessibilityIdentifier = "symbolExtraRow"
    row.isHidden = true
    for column in 0..<10 {
      let key = makeKey(title: "", accessibilityLabel: "") { [weak self] in
        guard let self else { return }
        let face = symbolExtraRowFaces[column]
        if numberPadShown {
          countKeyPress(TypingKeyID.character(face))
          typeNumberPadText(face)
        } else {
          typeSymbolLayerFace(face)
        }
      }
      key.accessibilityIdentifier = "symbolExtraKey\(column)"
      (key as? KeyboardKeyButton)?.showsPressPreview = true
      applySymbolLayerFont(to: key, size: 18)
      symbolExtraRowKeys.append(key)
      row.addArrangedSubview(key)
    }
    return row
  }

  private var symbolExtraRowFaces: [String] {
    SymbolLayerLayout.extraRow(more: showsMoreSymbols, numberPad: numberPadShown,
                              chinese: symbolLayerIsChinese && symbolLayerEngineWritesChinesePunctuation)
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

  /// 设计稿的 123 / #+= 层此刻是否画着，对应 Android 的 `drawsDesignLayer`。数字入口在所有键面都打开数字九宫格（`drawsNumberPad`），所以这一层只剩 #+= 那一页，除大千注音外所有键面（含假名九键）的 #+= 都是它。大千的 `， . / ; -` 被注音键占用，它的 #+= 保留原来每排十键的共享符号行（`symbolRowViews`，有 `; : / - ( ) @`），与 Android、HarmonyOS 相同。不是 private：布局测试会固定它。
  static func drawsSymbolLayer(symbols: Bool, numberPad: Bool, dachen: Bool) -> Bool {
    symbols && !numberPad && !dachen
  }

  /// 数字入口（123）一律打开数字九宫格：中文、英文、本地模式，以及日文（罗马字与假名）、韩文、越南文、藏文、大千注音、笔画、手写、14/18 键和 26 键一族都一样，iPhone 与 iPad 全尺寸键盘都一样；#+= 页仍是完整的符号层（大千注音是共享的每排十键符号行），从那里按 123 回到数字九宫格。这是维护者的产品决定，取代原来「独立语言、本地模式和英文保留各自 123 层」的约定。不是 private：布局测试会固定它。
  static func drawsNumberPad(symbols: Bool, moreSymbols: Bool) -> Bool {
    symbols && !moreSymbols
  }

  /// 数字九宫格上回到字母的那个键印什么：键面是进入前的那块键盘，与 Android `KeyboardActionRow.layerTitle` 的短名一致。只用在数字九宫格上，#+= 页的这个键印的字见 `moreSymbolsReturnTitle`。英文和本地模式是英文字母页的「ABC」（`SymbolLayerLayout.lettersTitle(chinese: false)`），日文罗马字、越南文和藏文的字母键也是拉丁字母，同样印「ABC」；韩文印「한」，假名九键印「あいう」，大千注音印「注」；中文里 14 键、18 键（含双拼 18 键，`letterGrid`）和笔画印各自的短名，拼音九键一族印「九键」，26 键一族和手写印中文字母页的「拼音」。不是 private：测试会固定它。
  static func numberPadReturnTitle(scheme: ChineseInputScheme, chinese: Bool, localMode: Bool,
                                   letterGrid: LetterKeyGrid? = nil) -> String {
    guard chinese && !localMode else { return SymbolLayerLayout.lettersTitle(chinese: false) }
    if let letterGrid { return letterGrid.shortTitle }
    switch scheme {
    case .fourteenKey: return LetterKeyGrid.fourteenKey.shortTitle
    case .eighteenKey: return LetterKeyGrid.eighteenKey.shortTitle
    case .stroke: return "笔"
    case .nineKey, .nineKeyShuangpin: return "九键"
    case .japaneseNineKey: return "あいう"
    case .zhuyin: return "注"
    default:
      if scheme.isKorean { return "한" }
      return SymbolLayerLayout.lettersTitle(chinese: !scheme.isJapanese && !scheme.writesAsciiPunctuation)
    }
  }

  /// #+= 页底行那个键印什么，保持数字入口改成数字九宫格之前的字样：设计稿的 #+= 层按这一层画中文版还是英文版印「拼音」或「ABC」（`SymbolLayerLayout.lettersTitle`，规则同 `symbolLayerIsChinese`），不按进入前的键面区分，所以 14/18 键、笔画、拼音九键和日文罗马字的中文 #+= 页也印「拼音」，韩文、越南文、藏文、英文和本地模式印「ABC」。假名九键原来没有 #+= 页，沿用它原来符号层的「あいう」。大千注音的 #+= 是共享符号行，这个键回到数字九宫格，印「123」（`SymbolLayerLayout.toggleTitle(more: true)`），与 HarmonyOS 相同。不是 private：测试会固定它。
  static func moreSymbolsReturnTitle(scheme: ChineseInputScheme, chinese: Bool, localMode: Bool) -> String {
    guard chinese && !localMode else { return SymbolLayerLayout.lettersTitle(chinese: false) }
    if scheme == .japaneseNineKey { return "あいう" }
    if scheme.isZhuyin { return SymbolLayerLayout.toggleTitle(more: true) }
    return SymbolLayerLayout.lettersTitle(chinese: !scheme.writesAsciiPunctuation)
  }

  /// 大千注音的 #+= 页（共享符号行）此刻是否画着：底行的切换键这时回到数字九宫格，不回字母层。
  private var dachenMoreSymbolsShown: Bool {
    typesZhuyin && showsSymbols && showsMoreSymbols
  }

  /// 数字九宫格此刻是否画着。26 键「长空格」也用它：数字九宫格底行（或「空格在右列」时右列）有空格，长按空格语音照常可用。
  private var numberPadShown: Bool {
    Self.drawsNumberPad(symbols: showsSymbols, moreSymbols: showsMoreSymbols)
  }

  /// 数字九宫格右列第三格的结束标点：中文和日文写全角的 ！，英文、本地模式和写 ASCII 标点的方案（韩文、越南文、藏文）写 ASCII 的 !，与 #+= 层选中文版还是英文版是同一条规则（`symbolLayerIsChinese`）。拼音九键一族和笔画自己的字母层只在中文下出现，仍是 ！。
  private var nineKeyClosingFace: String {
    symbolLayerIsChinese ? Self.nineKeyClosingMark : "!"
  }

  /// 手机上画的是中文 26 键字母页（全拼、26 键双拼、五笔 26 键、粤拼），「按键布局」的 26 键样式作用于它和它的 #+= 层。与 Android `TouchKeyStyle.Styles.forLayout` 和 HarmonyOS 一样只认中文方案：日文罗马字、韩文、越南文和藏文的 26 键保留标准底行。英文和本地输入模式画标准的英文底行；九键、14 键、18 键、笔画、手写、假名、大千和 iPad 全尺寸键盘有各自的底行。
  private var drawsChineseTwentySixKey: Bool {
    isChineseMode && chineseTwentySixKeyboard
  }

  /// 这块键盘的中文键面是中文 26 键：与 `drawsChineseTwentySixKey` 同一套判断，但不看此刻的中/英，同一块键盘切到英文 26 键时仍为真，与 Android `TouchKeyStyle.Styles.keepsToolbarRow` 和鸿蒙 `chineseTwentySixKeyboard` 相同。
  private var chineseTwentySixKeyboard: Bool {
    keyFaceFormFactor == .phone && !isInLocalMode && !usesLetterGrid
      && !inputScheme.isMandarinNineKey && !inputScheme.isJapanese && !inputScheme.writesAsciiPunctuation
      && !inputScheme.isZhuyin && !inputScheme.isStroke && inputScheme != .handwriting
  }

  /// 长空格样式让工具栏那一行一直显示：看用户的 26 键样式和这块键盘的中文键面（`chineseTwentySixKeyboard`），不看此刻的中/英；切到英文 26 键时 符 不在工具栏上（`longSpaceActive`），这一行照样显示品牌和收起。密码框和只收数字的框退回标准底行，不留。
  private var longSpaceToolbarRowKept: Bool {
    activeKeyLayoutStyle.twentySixKey == .longSpace && chineseTwentySixKeyboard
  }

  /// 26 键选了「长空格」并且正画着中文 26 键（含它的数字九宫格和 #+= 层）：符 移到工具栏，#+= 层的表情或符号入口换成逗号，与 Android 原来的 `longSpaceLayoutActive` 相同。数字入口与其他样式一样是数字九宫格，它自带空格。
  private var longSpaceActive: Bool {
    activeKeyLayoutStyle.twentySixKey == .longSpace && drawsChineseTwentySixKey
  }

  /// 「按键布局」此刻作用的样式：密码框和只收数字的输入框里三类键面都退回标准底行（`keepsStandardBottomRows`），与 Android 的 `bottomRowStyle` 相同；顶部数字行不受影响。
  private var activeKeyLayoutStyle: TouchKeyLayoutStyle {
    guard Self.keepsStandardBottomRows(keyboardType: inputContext.keyboardType, secure: textDocumentProxy.isSecureTextEntry,
                                       contentType: textDocumentProxy.textContentType ?? nil) else { return keyLayoutStyle }
    // 「符」与「123」互换、数字键盘空格位置、左侧符号栏的两张表和中/英键在空格哪一边是键位习惯，不随输入框退回：标准底行同样有 符 和中/英，数字框里的数字键盘也按用户选的排法和符号表画。
    return TouchKeyLayoutStyle(numberRow: keyLayoutStyle.numberRow, symbolFirst: keyLayoutStyle.symbolFirst,
                               numberPadSpace: keyLayoutStyle.numberPadSpace, nineKeySidebar: keyLayoutStyle.nineKeySidebar,
                               digitSidebar: keyLayoutStyle.digitSidebar, languageKeySide: keyLayoutStyle.languageKeySide)
  }

  /// 输入框是否保留标准底行：安全输入或内容类型标成密码的框，以及数字键盘、小数键盘和电话键盘这些只收数字的框，对应 Android 的 `EditorPolicy.password` 与 `numberInput`。系统通常不在安全输入框和电话键盘里调用第三方键盘，宿主仍然调用时也按这条退回。不是 private：测试会固定它。
  static func keepsStandardBottomRows(keyboardType: UIKeyboardType, secure: Bool?, contentType: UITextContentType?) -> Bool {
    if secure == true { return true }
    if let contentType, contentType == .password || contentType == .newPassword { return true }
    return [.numberPad, .decimalPad, .asciiCapableNumberPad, .phonePad].contains(keyboardType)
  }

  /// 顶部数字行此刻放得下：手机横屏时键盘已经占了大半个屏幕，再加一排会把输入框挤没，所以不画，存着的设置不变，转回竖屏就回来，对应 Android 的 480 dp 窗口高度门槛。iPad 上浮动键盘或窄窗口用手机键盘时照画。
  private var numberRowFits: Bool {
    !(traitCollection.userInterfaceIdiom == .phone && isLandscape)
  }

  /// 借用网格不改变统计布局：拼音九键一族自己的网格记 Nine，从其他键面（含拼音九键切到英文或本地模式时的 26 键）进来的数字九宫格记 Digit。
  private func numberPadKeyID(_ digit: Int) -> String? {
    isChineseMode && inputScheme.isMandarinNineKey && !isInLocalMode
      ? TypingKeyID.nineKey(digit) : TypingKeyID.character(String(digit))
  }

  private func typeSymbolLayerKey(row: Int, column: Int) {
    let chinese = symbolLayerIsChinese
    let face = SymbolLayerLayout.characterRows(more: showsMoreSymbols, chinese: chinese,
                                               cornerQuotes: currentCornerQuotes)[row][column]
    typeSymbolLayerFace(face)
  }

  private func typeSymbolLayerFace(_ face: String) {
    switch SymbolLayerLayout.input(for: face, chinese: symbolLayerIsChinese, chinesePunctuation: symbolLayerEngineWritesChinesePunctuation) {
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

  /// 自定义左侧符号栏的一项（`touch_nine_key_sidebar_symbols` / `touch_nine_key_digit_sidebar_symbols`）：先结束组字，组字的处理与点同一层出厂项时相同（字母层按当前首选上屏，与 `handleSymbol` 遇到 Engine 没有路由的符号时一样；数字层与 `insertNumberPadLiteral` 一样原样结束），再把这一项按字面上屏，不做全半角转换。表项正好是一对成对符号（`NineKeySidebar.literalPairClosing`，如 `（）`、`()`）时光标退到两者中间，与 Android `commitLiteralText` 相同。
  private func insertSidebarLiteral(_ text: String, digits: Bool) {
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    playInputClick()
    if digits {
      render(session.commitRaw())
    } else if isChineseMode {
      render(session.finishComposition())
    }
    insertOwnText(text)
    if let closing = NineKeySidebar.literalPairClosing(text) {
      textDocumentProxy.adjustTextPosition(byCharacterOffset: -closing.count)
      noteOwnEdit()
    }
    if !isChineseMode { refreshEnglishSuggestions() }
  }

  /// 数字九宫格上的数字、0、小数点、左列出厂项（运算符和半角语境追加的 `@ : ; ( ) ' " $`）和补充行的键。本地模式和越南文、藏文里，Engine 此刻把这个键列为拼写符号时（U 模式的十六进制数字，计算、网址模式的数字与符号，VNI 越南文正在拼的词的声调数字，藏文威利转写正在拼时的 `+ - . /`），与原来的 123 层一样作为字符交给会话，这些输入不能因为换成数字九宫格就被截断。藏文的 `'`（achung，如 `'od`）没有组字时也能开头一个音节，所以不看拼写符号表，总是先交给会话，与原来 123 页经 `handleSymbol` 的藏文分支相同：会话收下就留在组字里，不先上屏组字。会话不收时，以及其余所有情况（中文各方案、日文、韩文、大千注音和英文），按 `insertNumberPadLiteral` 原样结束组字再写出这个键：中文拼写里的 `$ %`、`www` 后的 `.` 在数字九宫格上仍按字面，大千把 1–0 列为拼写符号是因为它的数字行就是注音键，数字九宫格上的数字只是数字。
  private func typeNumberPadText(_ text: String) {
    if isInLocalMode || typesCasedLetters {
      // 拼写符号看的是会话此刻的组字，先画完此前异步投递的按键，与 `handleSymbol` 一样。
      session.flushPendingCharacters()
      flushDeferredInputSnapshot()
      if typesTibetan && text == "'" {
        let snapshot = session.handleCharacter(text)
        if snapshot.isHandled {
          playInputClick()
          render(snapshot)
          return
        }
      } else if session.engineSpellsWhileComposing(text) {
        let snapshot = session.handleCharacter(text)
        if snapshot.isHandled || snapshot.commitText?.isEmpty == false {
          playInputClick()
          render(snapshot)
          return
        }
      }
    }
    insertNumberPadLiteral(text)
  }

  /// 数字页原样结束组字，与 Android、鸿蒙一致；普通符号页仍沿用选词后插入的路径。
  private func insertNumberPadLiteral(_ text: String) {
    playInputClick()
    let literal = FullWidthInputPolicy.output(text, enabled: fullWidthInput && !typesKorean && !typesCasedLetters)
    render(session.commitRaw(), appendingLiteral: literal)
    if !isChineseMode { refreshEnglishSuggestions() }
  }

  private func toggleMoreSymbols() {
    playInputClick()
    showsMoreSymbols.toggle()
    updateKeyboardLayout()
  }

  private func updateSymbolLayerFaces() {
    for (key, face) in zip(symbolExtraRowKeys, symbolExtraRowFaces) where key.configuration?.title != face {
      key.configuration?.title = face
      key.accessibilityLabel = "符号 \(face)"
    }
    let rows = SymbolLayerLayout.characterRows(more: showsMoreSymbols, chinese: symbolLayerIsChinese,
                                               cornerQuotes: currentCornerQuotes)
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
    // 26 键「长空格」时 123 / #+= 层用它代替表情或符号入口（`updateKeyboardLayout`）：轻点输入逗号，上滑、下滑或长按输入句号，与 Android 那两层的逗号键相同。键面和角标随层的中英文更新。
    let layerComma = makeKey(title: ",", accessibilityLabel: "符号 ,") { [weak self] in
      guard let self, let face = layerCommaButton.configuration?.title else { return }
      typeSymbolLayerFace(face)
    }
    layerComma.configuration?.contentInsets = .zero
    layerComma.accessibilityIdentifier = "layerCommaKey"
    layerComma.accessibilityHint = "轻点输入逗号，上滑、下滑或长按输入句号"
    layerComma.isHidden = true
    if let key = layerComma as? KeyboardKeyButton {
      key.cornerHintSwipesEitherWay = true
      key.onCornerHint = { [weak self] hint in self?.typeSymbolLayerFace(hint) }
    }
    layerCommaButton = layerComma
    row.addArrangedSubview(layerComma)

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
      // 统计实际按下的标点键；键面和输出仍由当前方案及标点模式决定。
      countKeyPress(TypingKeyID.quickPunctuation)
      // 手机 26 键底行上这个键是设计稿的 ，：发逗号键，由 Engine 的标点路由选出标点，与符号行一样。日语画的是 、，发的也是 、（经 `KeyboardPunctuationContext` 换成 `\` 键）：Engine 的日语方案走中文标点表，逗号键写出的是 ，，与键面不符。
      handleSymbol(usesCustomQuickPunctuation ? quickPunctuationSymbols[0] : phoneBottomRowShown || nineKeyPunctuationRowShown ? (writesJapaneseComma ? "、" : ",") : quickPunctuationSymbols[0])
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
    // 14 键和手写等底行的独立句号；26 键字母层只在英文时显示，排在空格左边（`arrangeActionRow`）。
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

    // 手写的「重写」和 ⌫：书写区铺满整行后从它两侧的列搬到底行，排在 中 和回车之间；⌫ 有笔迹时撤回一笔，没有时删除光标前的字符（`handleBackspace`）。其他底行隐藏。
    let rewrite = makeKey(title: "重写", accessibilityLabel: "清空手写", function: true) { [weak self] in
      self?.handwriting.clear()
    }
    rewrite.configuration?.contentInsets = .zero
    rewrite.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
    rewrite.titleLabel?.adjustsFontSizeToFitWidth = true
    rewrite.titleLabel?.minimumScaleFactor = 0.7
    rewrite.accessibilityIdentifier = "handwritingRewrite"
    rewrite.isHidden = true
    handwritingRewriteButton = rewrite
    row.addArrangedSubview(rewrite)
    let handwritingDelete = makeDeleteKey()
    handwritingDelete.accessibilityIdentifier = "handwritingDelete"
    handwritingDelete.isHidden = true
    handwritingDeleteButton = handwritingDelete
    row.addArrangedSubview(handwritingDelete)
    // 原来书写区左侧的 ？ ！ 由 ， 和 。 的长按输入，角标与 26 键字母的符号角标画法相同。
    handwritingCommaHint = attachCornerHintLabel(to: punctuation)
    handwritingCommaHint.accessibilityIdentifier = "handwritingCommaHint"
    handwritingCommaHint.text = Self.chineseSymbolFaces["?"]
    handwritingPeriodHint = attachCornerHintLabel(to: period)
    handwritingPeriodHint.accessibilityIdentifier = "handwritingPeriodHint"
    handwritingPeriodHint.text = Self.chineseSymbolFaces["!"]

    // 九键网格数字层底行的 0：网格放 1-9，这个键补上 0。拼音网格和笔画键上它的位置让给最左边的 符。
    let zero = makeKey(title: "0", accessibilityLabel: "数字 0") { [weak self] in
      guard let self else { return }
      countKeyPress(numberPadKeyID(0))
      if inputScheme == .nineKeyShuangpin && !showsSymbols { handleCharacter("0") }
      else { typeNumberPadText("0") }
    }
    zero.configuration?.contentInsets = .zero
    zero.configuration?.titleTextAttributesTransformer = Self.functionLabelTransformer
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
    // 九键「右侧大回车」在回车的位置留空，右列伸下来的回车（`tallReturnKey`）盖在这里。
    tallReturnSlot.accessibilityIdentifier = "tallReturnSlot"
    tallReturnSlot.isUserInteractionEnabled = false
    tallReturnSlot.isHidden = true
    row.addArrangedSubview(tallReturnSlot)

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
    // 字母层保留大空格；九键双拼的 0 仍占一个功能键的宽度。
    nineKeyActionWidths = [
      weight(nineKeySymbolsButton, 1.05, space: Self.nineKeySpaceWeight, optional: true),
      weight(layoutToggle, 1.25, space: Self.nineKeySpaceWeight),
      weight(language, 1.05, space: Self.nineKeySpaceWeight),
      weight(globe, 1, space: Self.nineKeySpaceWeight, optional: true),
      weight(zero, 1.05, space: Self.nineKeySpaceWeight, optional: true),
      weight(enter, 1.6, space: Self.nineKeySpaceWeight),
    ]
    // 数字页把中央的大键位交给 0；空格保留在它右边，隐藏语言键时也把余量给 0。
    func numberPadWidths(hidesLanguage: Bool) -> [NSLayoutConstraint] {
      let zeroWeight: CGFloat = Self.nineKeySpaceWeight + (hidesLanguage ? 1.05 : 0)
      return [
        weight(layoutToggle, 1.25, space: 1.05),
        weight(globe, 1, space: 1.05, optional: true),
        weight(zero, zeroWeight, space: 1.05),
        weight(enter, 1.6, space: 1.05),
      ] + (hidesLanguage ? [] : [weight(language, 1.05, space: 1.05)])
    }
    numberPadActionWidths = numberPadWidths(hidesLanguage: false)
    numberPadExpandedZeroWidths = numberPadWidths(hidesLanguage: true)
    // 数字页底行沿用上方的五列；隐藏中/英时，0 合并前两格，空格留在第三格。
    if let sidebar = nineSidebarWidth?.firstItem as? UIView, let controls = nineControls,
       let digit = nineKeyGridKeys.first?.button {
      let edges = [
        layoutToggle.widthAnchor.constraint(equalTo: sidebar.widthAnchor),
        enter.widthAnchor.constraint(equalTo: controls.widthAnchor),
      ]
      numberPadGridWidths = edges + [
        zero.widthAnchor.constraint(equalTo: digit.widthAnchor),
        language.widthAnchor.constraint(equalTo: digit.widthAnchor),
      ]
      numberPadExpandedGridWidths = edges + [space.widthAnchor.constraint(equalTo: digit.widthAnchor)]
    }
    layerActionWidths = [
      weight(layoutToggle, SymbolLayerLayout.lettersWeight, space: SymbolLayerLayout.spaceWeight),
      weight(emoji, SymbolLayerLayout.panelWeight, space: SymbolLayerLayout.spaceWeight, optional: true),
      weight(nineKeySymbolsButton, SymbolLayerLayout.panelWeight, space: SymbolLayerLayout.spaceWeight, optional: true),
      weight(layerComma, SymbolLayerLayout.panelWeight, space: SymbolLayerLayout.spaceWeight, optional: true),
      weight(globe, SymbolLayerLayout.globeWeight, space: SymbolLayerLayout.spaceWeight, optional: true),
      weight(enter, SymbolLayerLayout.returnWeight, space: SymbolLayerLayout.spaceWeight),
    ]
    let phoneSpace = Self.phoneSpaceWeight
    phoneActionWidths = [
      weight(nineKeySymbolsButton, 1.05, space: phoneSpace, optional: true),
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
    // 隐藏语言键时把它的份额交给空格，避免仅隐藏视图后其他功能键也一起变宽。
    func expandedWidths(spaceWeight: CGFloat, entries: [(UIView, CGFloat, Bool)]) -> [NSLayoutConstraint] {
      entries.map { key, value, optional in weight(key, value, space: spaceWeight, optional: optional) }
    }
    phoneExpandedSpaceWidths = expandedWidths(spaceWeight: phoneSpace + 1.05, entries: [
      (nineKeySymbolsButton, 1.05, true), (layoutToggle, 1.25, false), (globe, 1, true), (delete, 1, true),
      (punctuation, 1, true), (period, 1, true), (enter, 1.9, false),
    ])
    let englishSpace = Self.englishPhoneSpaceWeight
    let englishEntries: [(UIView, CGFloat, Bool)] = [
      (nineKeySymbolsButton, 1.05, true), (layoutToggle, 1.25, false), (globe, 1, true),
      (punctuation, 1, true), (period, 1, true), (enter, 1.9, false),
    ]
    phoneEnglishActionWidths = expandedWidths(spaceWeight: englishSpace, entries: englishEntries)
      + [weight(language, 1.05, space: englishSpace)]
    phoneEnglishExpandedSpaceWidths = expandedWidths(spaceWeight: englishSpace + 1.05, entries: englishEntries)
    let handwritingSpace = Self.handwritingSpaceWeight
    let handwritingEntries: [(UIView, CGFloat, Bool)] = [
      (layoutToggle, Self.handwritingLayerWeight, false), (globe, 1, true),
      (punctuation, Self.handwritingPunctuationWeight, true), (period, Self.handwritingPunctuationWeight, true),
      (rewrite, Self.handwritingRewriteWeight, true), (handwritingDelete, Self.handwritingDeleteWeight, true),
      (enter, Self.handwritingReturnWeight, false),
    ]
    handwritingActionWidths = expandedWidths(spaceWeight: handwritingSpace, entries: handwritingEntries)
      + [weight(language, Self.handwritingLanguageWeight, space: handwritingSpace)]
    handwritingExpandedSpaceWidths = expandedWidths(
      spaceWeight: handwritingSpace + Self.handwritingLanguageWeight, entries: handwritingEntries)
    nineKeyExpandedSpaceWidths = expandedWidths(spaceWeight: Self.nineKeySpaceWeight + 1.05, entries: [
      (nineKeySymbolsButton, 1.05, true), (layoutToggle, 1.25, false), (globe, 1, true), (zero, 1.05, true), (enter, 1.6, false),
    ])
    let tabletEntries: [(UIView, CGFloat, Bool)] = [
      (layoutToggle, TabletLetterLayout.layerWeight, false),
      (globe, TabletLetterLayout.globeWeight, true),
      (tabletLayer, TabletLetterLayout.layerWeight, false),
      (dismiss, TabletLetterLayout.dismissWeight, false),
    ]
    tabletExpandedSpaceWidths = expandedWidths(
      spaceWeight: tabletSpace + TabletLetterLayout.languageWeight, entries: tabletEntries)
    tabletSplitExpandedSpaceWidths = expandedWidths(
      spaceWeight: tabletSpace + TabletLetterLayout.languageWeight / 2, entries: tabletEntries)
    actionGlobeButton = globe
    return row
  }

  /// 手机 26 键底行上空格键的弹性权重；其他键的权重都相对它来算。
  static let phoneSpaceWeight: CGFloat = 4
  /// 英文 26 键底行在空格左边多放一个句点（份额 1），这一份从空格里让出，123、符、逗号、中/英、回车保持中文 26 键的宽度。与 Android `KeyboardActionRow.DESIGN_ENGLISH_SPACE_WEIGHT`、HarmonyOS `ENGLISH_SPACE_SHARE` 相同。
  static let englishPhoneSpaceWeight: CGFloat = phoneSpaceWeight - 1
  /// 手写底行 123 |（地球键 1）| ， | 空格 | 。 | 中 | 重写 | ⌫ | 回车 的弹性权重，三个移动端相同（Android `KeyboardActionRow.HANDWRITING_*_WEIGHT`、HarmonyOS `HANDWRITING_*_SHARE`，`scripts/test-mobile-handwriting-pad-parity.py` 比对）。空格从 4 让到 2.4、回车从 1.9 让到 1.6（与九键底行的回车相同），腾出「重写」和 ⌫ 的位置：320pt 宽时 ， 。 约 25pt，与 26 键字母键相当，空格仍放得下麦克风和「手写」。
  static let handwritingLayerWeight: CGFloat = 1.25
  static let handwritingPunctuationWeight: CGFloat = 1
  static let handwritingSpaceWeight: CGFloat = 2.4
  static let handwritingLanguageWeight: CGFloat = 1.05
  static let handwritingRewriteWeight: CGFloat = 1.25
  static let handwritingDeleteWeight: CGFloat = 1.25
  static let handwritingReturnWeight: CGFloat = 1.6
  /// 九键字母层空格键及需要底行地球键时数字层 0 键的弹性权重。
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

  /// 空格键的 VoiceOver 操作。关掉长按空格语音后不再提供「语音输入」；提供时它是用户明确的请求，所以不看完全访问权限，没有权限时由语音入口说明原因。
  private func spaceAccessibilityActions() -> [UIAccessibilityCustomAction] {
    var actions = [
      UIAccessibilityCustomAction(name: "光标左移") { [weak self] _ in self?.moveCursor(by: -1); return self != nil },
      UIAccessibilityCustomAction(name: "光标右移") { [weak self] _ in self?.moveCursor(by: 1); return self != nil },
    ]
    if spaceVoiceEnabled {
      actions.append(UIAccessibilityCustomAction(name: "语音输入") { [weak self] _ in
        guard let self, voiceInsertionReady else { return false }
        startVoiceFromSpace(held: false)
        return true
      })
    }
    return actions
  }

  /// 底部那一排的两个空格键和数字层右列的空格；第二个只在分离式键盘里显示，第三个只在数字键盘「空格在右列」时显示，标题随第一个一起更新。
  private var spaceKeys: [UIButton] {
    [spaceButton, splitSpaceButton, numberPadColumnSpace].compactMap { $0 }
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

  private func flushDeferredInputSnapshot() {
    guard let snapshot = deferredInputSnapshot else {
      deferredInputRenderScheduled = false
      return
    }
    deferredInputSnapshot = nil
    deferredInputRenderScheduled = false
    render(snapshot)
  }

  /// 投递之后换了方案（九键、全拼、双拼之间切换）、切到英文、换了输入框或经过焦点边界时，旧结果不再画出。
  private func deliverAsynchronousSnapshot(_ snapshot: MetasequoiaInputSnapshot, epoch: UInt64,
                                           scheme: ChineseInputScheme, document: UUID?) {
    guard inputDeliveryEpoch == epoch, isChineseMode, inputScheme == scheme,
          KeyboardHostContext.documentIdentifier(for: textDocumentProxy) == document else { return }
    if snapshot.commitText != nil {
      flushDeferredInputSnapshot()
      render(snapshot)
      return
    }
    deferredInputSnapshot = snapshot
    guard !deferredInputRenderScheduled else { return }
    deferredInputRenderScheduled = true
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.flushDeferredInputSnapshot()
    }
  }

  /// 这一键能否异步交给会话线程：普通全拼九键的数字 2–9，26 键全拼的小写字母，26 键双拼的小写字母和 `;`。调用方已排除本地模式、韩语、越南语、藏文和辅助码大写；五笔、粤拼、日语等其他 26 键方案仍走同步入口。
  private func typesAsynchronously(_ character: String) -> Bool {
    guard character.utf8.count == 1, let byte = character.utf8.first else { return false }
    let letter = (97...122).contains(byte)
    switch inputScheme {
    case .nineKey: return (50...57).contains(byte)
    case .quanpin: return !usesLetterGrid && letter
    default: return typesShuangpinLetters && (letter || byte == 59)
    }
  }

  /// `touchDown` 是这一键按下时手指在键区里的位置（`KeyboardKeyButton.touchDownLocation`），只有 26 键的字母键和 `;` 的真实触摸才有；它只用于 26 键双拼的纠错计价（`TouchPointPolicy.sendsTouch`），其余情况照常输入这个字符。
  private func handleCharacter(_ character: String, touchDown: CGPoint? = nil) {
    playInputClick()
    if isChineseMode {
      synchronizeInputSchemePreference()
      // A host setting can change while this view is open. Do not start an alphabetic composition
      // from a stale 26-key tap after switching to nine keys; local utilities still need letters.
      let gridDigits = inputScheme == .nineKeyShuangpin ? "0"..."9" : "2"..."9"
      if inputScheme.isMandarinNineKey && !isInLocalMode
        && !gridDigits.contains(character) && character != "'" {
        return
      }
      // 14 键和 18 键同理：它们的键经 `gridKey` 送组码，这里只收分词键的 `'`；切到字母网格之前按下的 26 键字母不再开始一段 26 键组字。
      if typesLetterGrid && character != "'" { return }
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
      // 26 键双拼的字母和 `;` 带上按下点，引擎按落点给按错邻键的纠错计价（#10018）。拿不到几何时退回不带落点，与 `sendActions` 和读屏的激活一样。几何要变时 `registeredKeyGeometryID` 先同步注册，排在此前已投递的按键之后、这一键之前。
      var touch: MetasequoiaInputSessionBridge.TouchPoint?
      if let touchDown,
         TouchPointPolicy.sendsTouch(character: character, shuangpinLetters: typesShuangpinLetters, hasTouchDown: true,
                                     voiceOver: isVoiceOverRunning()),
         let geometryID = registeredKeyGeometryID() {
        touch = .init(geometryID: geometryID, x: Float(touchDown.x), y: Float(touchDown.y))
      }
      // 普通全拼九键数字、26 键全拼和 26 键双拼的字母交给会话线程查询，主线程只投递（#10031）；结果按投递顺序回到主线程，空格、删除、Shift、符号和焦点边界先排空此前的按键。
      if asynchronousInputEnabled, !isInLocalMode, typesAsynchronously(character) {
        let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy)
        asynchronousInputDocument = document
        let epoch = inputDeliveryEpoch
        let scheme = inputScheme
        if session.enqueueCharacter(character, touch: touch, completion: { [weak self] snapshot in
          self?.deliverAsynchronousSnapshot(snapshot, epoch: epoch, scheme: scheme, document: document)
        }) {
          if touch != nil { touchCharacterCount += 1 }
          return
        }
      }
      if let touch {
        touchCharacterCount += 1
        render(session.handleTouchCharacter(character, geometryID: touch.geometryID, x: touch.x, y: touch.y))
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

  /// 从宿主读取当前词，跟随光标和外部编辑；非空选区不参与纠错，避免退格删掉用户选中的内容。
  private var englishWordBeforeCursor: String {
    guard englishCorrectionsEnabled, textDocumentProxy.selectedText?.isEmpty != false else { return "" }
    let before = textDocumentProxy.documentContextBeforeInput ?? ""
    return EnglishSuggestionPolicy.currentWord(before: before)
  }

  private func refreshEnglishSuggestions() {
    let prefix = englishWordBeforeCursor
    if prefix != englishCorrectionRejectedPrefix {
      englishCorrectionRejectedPrefix = ""
      englishCorrectionRejectedDocument = nil
    }
    guard !prefix.isEmpty else {
      englishSuggestionCorrected = false
      updateCandidateStrip(preedit: "", candidates: [])
      return
    }
    let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy)
    // 新词先显示自己的原文，避免异步查询期间留下上一词的可点纠错或旧邮箱后缀；同一词已有的答案可以保留。
    if englishSuggestionPrefix != prefix || englishSuggestionDocument != document {
      englishSuggestionCorrected = false
      updateCandidateStrip(preedit: "", candidates: [prefix])
      if visibleEmailSuffixes.isEmpty {
        englishSuggestionPrefix = prefix
        englishSuggestionDocument = document
      }
    }
    // 纠错在输入线程上算。回来时候选条已被别的路径改过、换了输入框或光标前的词已变，这次结果就作废。
    englishSuggestionStripGeneration &+= 1
    let generation = englishSuggestionStripGeneration
    session.englishSuggestions(forPrefix: prefix) { [weak self] suggestions in
      guard let self, generation == englishSuggestionStripGeneration,
            document == KeyboardHostContext.documentIdentifier(for: textDocumentProxy),
            prefix == englishWordBeforeCursor else { return }
      englishSuggestionCorrected = suggestions.corrected
      updateCandidateStrip(preedit: "", candidates: suggestions.items)
      if visibleEmailSuffixes.isEmpty {
        englishSuggestionPrefix = prefix
        englishSuggestionDocument = document
      }
    }
  }

  private var englishCorrectionsEnabled: Bool {
    !isChineseMode && EnglishSuggestionsPreference.isEnabled && !isCredentialField
      && textDocumentProxy.keyboardType != .URL && textDocumentProxy.keyboardType != .emailAddress
      && textDocumentProxy.autocorrectionType != .no
  }

  @discardableResult
  private func commitEnglishBoundary(_ boundary: String) -> Bool {
    guard englishCorrectionsEnabled, " ,.!?:;".contains(boundary), boundary.count == 1 else { return false }
    let typed = englishWordBeforeCursor
    guard !typed.isEmpty else { return false }
    let rejected = typed == englishCorrectionRejectedPrefix
      && englishCorrectionRejectedDocument == KeyboardHostContext.documentIdentifier(for: textDocumentProxy)
    let items = rejected ? [typed] : session.englishCorrections(forPrefix: typed)
    let candidate = items.first == typed && items.count == 2 ? items[1] : typed
    guard typed == englishWordBeforeCursor else { return false }
    let replacement = EnglishSuggestionPolicy.replacement(
      typed: typed, candidate: candidate, startedCapitalized: typed.first?.isUppercase ?? false)
    if let replacement {
      for _ in 0..<replacement.deleteCount { deleteOwnBackward() }
    }
    let suffix = boundary == " " ? " " : boundary + " "
    let word = FullWidthInputPolicy.output(replacement?.insert ?? "",
      enabled: fullWidthInput && !typesKorean && !typesCasedLetters)
    insertOwnText(word + suffix)
    englishCorrectionRejectedPrefix = ""
    updateCandidateStrip(preedit: "", candidates: [])
    return true
  }

  private func useEnglishSuggestion(at index: Int) {
    guard visibleCandidates.indices.contains(index) else { return }
    let typed = englishWordBeforeCursor
    guard !typed.isEmpty, typed == englishSuggestionPrefix,
          englishSuggestionDocument == KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else {
      updateCandidateStrip(preedit: "", candidates: [])
      return
    }
    let startedCapitalized = typed.first?.isUppercase ?? false
    guard let replacement = EnglishSuggestionPolicy.replacement(
      typed: typed, candidate: visibleCandidates[index], startedCapitalized: startedCapitalized)
    else {
      // 只有展示了纠错时点原文才是拒绝纠错；只有补全时点原文仅收起候选。
      if englishSuggestionCorrected {
        englishCorrectionRejectedPrefix = typed
        englishCorrectionRejectedDocument = KeyboardHostContext.documentIdentifier(for: textDocumentProxy)
      }
      englishSuggestionCorrected = false
      updateCandidateStrip(preedit: "", candidates: [])
      finishTemporaryEnglish()
      return
    }
    for _ in 0..<replacement.deleteCount { deleteOwnBackward() }
    insertDirectText(replacement.insert)
    updateCandidateStrip(preedit: "", candidates: [])
    finishTemporaryEnglish()
  }

  /// 候选条这时该给的邮箱后缀：没在组字、不在本地模式、候选条没被九键 1 键的常用标点或手写结果占着、不是密码或验证码输入框时，按光标前的文字匹配（`EmailSuffixPolicy`）。上文只在这里读一次、在内存里匹配，不记录、不保存。组字时候选条归 Engine 的候选。
  private func emailSuffixesForStrip(preedit: String) -> [String] {
    guard preedit.isEmpty, !hasComposition, !isInLocalMode, nineKeyQuickSymbols.isEmpty,
          inputScheme != .handwriting || handwritingResults.isEmpty, !isCredentialField else { return [] }
    return EmailSuffixPolicy.match(textBeforeCursor: textDocumentProxy.documentContextBeforeInput)?.suffixes ?? []
  }

  /// 按这时光标前的文字重新决定候选条给不给邮箱后缀；结果变了才重画。后缀不再匹配时，英文模式换回英文建议，中文模式回到空闲。
  private func refreshEmailSuffixes() {
    let next = emailSuffixesForStrip(preedit: visiblePreedit)
    guard next != visibleEmailSuffixes else { return }
    if next.isEmpty && !isChineseMode {
      refreshEnglishSuggestions()
    } else {
      updateCandidateStrip(preedit: "", candidates: [])
    }
  }

  /// 键盘自己经 `textDocumentProxy` 插入或删除文字时，iOS 不回调 `textDidChange`（见 `OwnEditEchoWindow`），而符号面板、中文模式下的符号层按键这类路径上屏之后不再更新候选条。所以每次自己改了文档都在下一轮主线程上重新匹配一次：那时这一键的所有改动和候选条更新都已做完，一轮里的几次改动只匹配一次。
  private func scheduleEmailSuffixRefresh() {
    guard !emailSuffixRefreshScheduled else { return }
    emailSuffixRefreshScheduled = true
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      emailSuffixRefreshScheduled = false
      refreshEmailSuffixes()
    }
  }

  /// 补全邮箱后缀：按点下这一刻的上文重新匹配，删掉已经打的域名，上屏完整后缀（不带 `@`，ASCII，不随全角开关变）。上文已经变了、对不上这个后缀时只收起。补全后上文以完整后缀结尾，不再匹配，候选条回到空闲。
  private func useEmailSuffix(at index: Int) {
    let suffix = visibleEmailSuffixes[index]
    let match = EmailSuffixPolicy.match(textBeforeCursor: textDocumentProxy.documentContextBeforeInput)
    if let replacement = EmailSuffixPolicy.replacement(match, suffix: suffix) {
      for _ in 0..<replacement.deleteCount { deleteOwnBackward() }
      insertOwnText(replacement.insert)
    }
    updateCandidateStrip(preedit: "", candidates: [])
  }

  /// A digit past the chips on show while composing. The session answers it with an unhandled diagnostic carrying no preedit, which the path below would take for "no composition" and type the digit into the document mid-word, so it does nothing instead. Not private: the page-size tests pin it.
  static func digitHasNoChip(_ digit: String, chips: Int, composing: Bool) -> Bool {
    guard composing, let number = Int(digit) else { return false }
    return number > chips
  }

  private func handleSymbol(_ symbol: String) {
    // 符号的去向看组字、候选条和本地模式，先画完此前异步投递的按键，不按旧画面判断。
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    playInputClick()
    if !isChineseMode {
      defer {
        if symbol.contains(where: { !$0.isLetter && !$0.isNumber }) { finishTemporaryEnglish() }
      }
      if session.sharedPreferences?["punctuation_lock"] as? String != "chinese",
         commitEnglishBoundary(symbol) { return }
      // 标点锁定为中文 keeps Chinese marks in English mode too; the shared route decides, as it does in Chinese mode.
      if session.sharedPreferences?["punctuation_lock"] as? String == "chinese",
         let punctuation = Self.lockedEnglishPunctuationInput(
           for: symbol, enumerationCommaBackslash: currentEnumerationCommaBackslash,
           enumerationCommaSlash: currentEnumerationCommaSlash) {
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
      insertDirectText(Self.zhuyinSymbolText(
        symbol, chinesePunctuation: zhuyinWritesChinesePunctuation,
        cornerQuotes: currentCornerQuotes, hollowBrackets: currentHollowBrackets,
        enumerationCommaBackslash: currentEnumerationCommaBackslash,
        enumerationCommaSlash: currentEnumerationCommaSlash))
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
      // 候选条上的邮箱后缀不归 Engine，`abc@1` 的数字照常上屏。
      let composing = !visiblePreedit.isEmpty || (!visibleCandidates.isEmpty && visibleEmailSuffixes.isEmpty)
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
      for: symbol, japanese: inputScheme.isJapanese, asciiMarks: typesKorean || typesCasedLetters,
      enumerationCommaBackslash: currentEnumerationCommaBackslash,
      enumerationCommaSlash: currentEnumerationCommaSlash) else {
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
    cancelCursorSwipeIfDocumentChanged()
    guard let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else { return }
    applyInputContext(keyboardType: textDocumentProxy.keyboardType ?? .default, documentIdentifier: document)
  }

  // Explicit UIKit-trait boundary also allows layout/state regression tests without a fake engine.
  func applyInputContext(keyboardType: UIKeyboardType, documentIdentifier: UUID) {
    let language = inputContext.languageOverride(for: keyboardType, document: documentIdentifier,
                                                 isChinese: isChineseMode, temporaryEnglish: temporaryEnglishFromScheme)
    // 换了输入框：密码框和只收数字的框保留标准底行（`activeKeyLayoutStyle`），底行跟着输入框重算。
    defer { if actionRow != nil { updateKeyboardLayoutIfNeeded() } }
    guard let chinese = language else { return }
    temporaryEnglishFromScheme = false
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

  private func toggleInputMode(rememberChoice: Bool = true) {
    temporaryEnglishFromScheme = false
    englishCorrectionRejectedPrefix = ""
    // Shift 和片段结束键各自播放按键音；自动切回不再追加一次切换音。
    if rememberChoice { playInputClick() }
    let snapshot = isChineseMode ? endComposition(at: .modeSwitch) : session.cancel()
    render(snapshot)
    isChineseMode.toggle()
    if rememberChoice && !inputContext.isInLatinField {
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

  /// 仅替换中文全拼字母页，密码框、本地模式和其他方案保留大小写。共享偏好 `touch_quanpin_shift` 打开后 ⇧ 保留，用来臂辅助码。
  private var usesQuanpinSeparator: Bool {
    !quanpinShiftEnabled && inputScheme == .quanpin && isChineseMode && !showsSymbols && !isInLocalMode
      && !isCredentialField && !session.initializationFailed
  }

  private func handleLetterShift(keyID: String) {
    // 组字中 Shift 是辅助码、空闲时切英文，要看此前异步投递的字母画完后是否在组字。
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    if usesQuanpinSeparator {
      countKeyPress(TypingKeyID.character("'"))
      if hasComposition { handleCharacter("'") }
      else { playInputClick() }
      return
    }
    countKeyPress(keyID)
    toggleLetterCase()
  }

  private func toggleLetterCase() {
    // 韩语的 Shift 是布局自己的 Shift：选出 ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ，而不是切到英文。越南语和藏文的 Shift 与英文一样给出大写字母。
    if isChineseMode && !helpcodeCompositionEligible && !typesKorean && !typesCasedLetters {
      toggleInputMode(rememberChoice: false)
      temporaryEnglishFromScheme = true
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
    if letterCaseState == .lowercase { finishTemporaryEnglish(force: true) }
    updateLetterCaseControls()
  }

  /// 大写锁定跨单词保持，手动解除或离开编辑上下文时才退出；临时进入和返回都不写「沿用上次」。
  private func finishTemporaryEnglish(force: Bool = false) {
    guard temporaryEnglishFromScheme, force || letterCaseState != .capsLock else { return }
    toggleInputMode(rememberChoice: false)
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
    let capitalization = (englishCorrectionsEnabled || inputContext.keyboardType == .URL || inputContext.keyboardType == .emailAddress)
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
    guard !letterButtons.isEmpty else { return }
    let inputs = LetterCaseInputs(
      chinese: isChineseMode, scheme: inputScheme, state: letterCaseState,
      localMode: currentLocalMode, entersHelpcode: entersHelpcode,
      casedLetters: typesCasedLetters, korean: typesKorean,
      swipeSymbols: swipeSymbolsEnabled, shuangpinHints: shuangpinKeyHintsEnabled)
    guard appliedLetterCaseInputs != inputs else { return }
    appliedLetterCaseInputs = inputs
    // 中文模式的字母键面画大写（与搜狗、微信、百度、豆包的拼音 26 键一致），输入的仍是小写拼写；英文模式画小写，只有下一键真的会输入大写时键面才变大写：英文的 Shift 或大写锁定，组字中的辅助码 Shift（无障碍标签里也会体现，并以大写辅助码交给 Engine），以及越南语和藏文区分大小写的字母（它们按 Shift 画大小写，不跟中文模式画大写）。日语罗马字照日文键盘的惯例画小写；本地输入模式（英文补全、Unicode、快捷短语等）按字面输入，也画小写。
    let casedLetters = typesCasedLetters
    let shifted = letterCaseState != .lowercase && (!isChineseMode || entersHelpcode || casedLetters)
    // 只读一次，不要每个键读一次。`isInLocalMode` 看起来像属性，其实是一次完整的 C ABI 往返：Rust 把整个视图（预编辑、每个候选及其编码和释义）序列化成 JSON，Swift 再解析回来。放在下面的循环里调用，每次按键都会做二十七遍。
    let inLocalMode = isInLocalMode
    let usesUppercase = shifted || (isChineseMode && !casedLetters && !inputScheme.isJapanese && !inLocalMode)
    let korean = typesKorean
    let koreanShifted = korean && letterCaseState != .lowercase
    for (button, lowercase, hintLabel, cornerLabel) in letterButtons {
      // 角标符号属于所有模式和方案下的 QWERTY 字母键面，与 Android 的 `standardLetters` 一致；韩文字母键面没有，微软双拼的 `;` 也没有。
      let corner = korean ? nil : LetterHintTable.hint(for: lowercase)
      cornerLabel.text = corner
      cornerLabel.isHidden = corner == nil
      (button as? KeyboardKeyButton)?.cornerHint = corner
      (button as? KeyboardKeyButton)?.swipesCornerHint = swipeSymbolsEnabled
      (button as? KeyboardKeyButton)?.cornerHintSwipeDirection = swipeSymbolsDirection
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
      // `;` 键的标签已经说出它在当前方案里的韵母，不再按字母读，也不重复读键位提示。
      if lowercase == ";" {
        button.accessibilityLabel = inputScheme.semicolonFinalKeyAccessibilityLabel
        button.accessibilityValue = nil
        continue
      }
      button.accessibilityLabel =
        shifted
        ? "大写 \(lowercase.uppercased())" : "字母 \(lowercase.uppercased())"
      button.accessibilityValue = hint
    }

    // iPad 键盘第三排两端各有一个 ⇧，全拼分词时两个位置都就地更新。
    for button in [shiftButton, rightShiftButton].compactMap({ $0 }) {
      if usesQuanpinSeparator {
        button.accessibilityLabel = "拼音分词"
        button.accessibilityValue = nil
        button.accessibilityHint = "分隔当前组合中的拼音音节"
        continue
      }
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
      button.accessibilityHint = temporaryEnglishFromScheme
        ? (letterCaseState == .capsLock ? "轻点解除大写锁定并返回原输入方案" : "空格或标点后返回原输入方案") : nil
    }
    styleShiftKeys()
  }

  /// 设计稿的 ⇧ 及其带下划线的大写锁定样式，画法与 Android 的 `styleShiftKey` 一致：关闭时用功能键底色、图标为按键颜色；打开时（单次或锁定）用字母键底色、图标为强调色。皮肤处理会给所有功能键套上关闭时的颜色；它调用的 `updateSchemeButton` 再经 `updateLetterCaseControls` 重新设置这两个键的样式。
  private func styleShiftKeys() {
    let skin = KeyboardTheme.current
    let separator = usesQuanpinSeparator
    let on = !separator && letterCaseState != .lowercase
    for button in [shiftButton, rightShiftButton].compactMap({ $0 }) {
      guard var configuration = button.configuration else { continue }
      configuration.image = separator ? nil : (letterCaseState == .capsLock ? Self.capsLockIcon : Self.shiftIcon)
      configuration.title = separator ? "分词" : nil
      configuration.titleTextAttributesTransformer = Self.functionLabelTransformer
      // 默认内边距让 44pt 宽的键只剩十几点给标题，「分词」会折成两行；与 14 键的分词键一样不留内边距，图标本来就居中。
      configuration.contentInsets = .zero
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
    case .cantonese, .cantoneseShuangpin: return "粤"
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
    case .cantoneseShuangpin: return "粤语双拼输入"
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

  /// 设计稿的空格键面：中文下是麦克风和方案简称，英文下是 `space`，符号层上是空格；手写板与其他方案一样显示方案名（手写），与 Android 的 SpaceKeyFace 一致。日语按按键的作用命名，空闲时是空白，选候选时是変換。麦克风只在 `voice`（「长按空格语音输入」）开着时画：关掉后长按空格改为移动光标，键面不能再暗示能按住说话（#10143），与 Android 的 `SpaceKeyFace.setShowsMic` 一致。不是 private：布局测试会固定它。
  static func spaceKeyFace(scheme: ChineseInputScheme, chinese: Bool, symbols: Bool, composing: Bool,
                           voice: Bool) -> SpaceKeyFace {
    if chinese && scheme.isJapanese { return SpaceKeyFace(label: composing ? "変換" : "空白", mic: false) }
    if symbols { return SpaceKeyFace(label: "空格", mic: voice) }
    if !chinese { return SpaceKeyFace(label: "space", mic: voice) }
    return SpaceKeyFace(label: scheme.shortLabel, mic: voice)
  }

  private func updateSpaceKeyTitle() {
    let japaneseTitle = hasComposition ? "変換" : "空白"
    if var configuration = japaneseSpaceButton?.configuration,
       configuration.title != japaneseTitle {
      configuration.title = japaneseTitle
      japaneseSpaceButton?.configuration = configuration
      japaneseSpaceButton?.accessibilityLabel = japaneseTitle
    }
    let face = Self.spaceKeyFace(scheme: inputScheme, chinese: isChineseMode, symbols: showsSymbols, composing: hasComposition,
                                 voice: spaceVoiceEnabled)
    let japanese = isChineseMode && inputScheme.isJapanese
    // 数字页的空格只有一个功能键宽，留出完整的文字空间，长按语音手势仍由原按键处理。
    let mic = face.mic && !numberPadShown
    let color = KeyboardTheme.current.secondary
    for space in spaceKeys {
      guard var configuration = space.configuration else { continue }
      let hasMic = configuration.image != nil
      guard configuration.title != face.label || hasMic != mic || configuration.baseForegroundColor != color else { continue }
      configuration.title = face.label
      configuration.image = mic ? KeyboardIcon.mic.image(pointSize: Self.spaceMicSide) : nil
      configuration.imagePlacement = .leading
      // 麦克风与方案短名之间 4pt、两侧各 4pt，与 Android 的 `SpaceKeyFace`（GAP_DP 4、两侧共 8dp）相同。手写底行的空格只有 2.4 份，320pt 宽时约 60pt，按系统默认的 12pt 内边距放不下麦克风和「手写」两个字；内容居中，宽的空格键看不出差别。
      configuration.imagePadding = Self.spaceLabelGap
      configuration.contentInsets.leading = Self.spaceLabelGap
      configuration.contentInsets.trailing = Self.spaceLabelGap
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
  /// 空格键上麦克风与短名之间、以及键面两侧的留白，取 Android 的 `SpaceKeyFace.GAP_DP`。
  private static let spaceLabelGap: CGFloat = 4

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

  /// 底行的回车、iPad 第二排字母上的回车（`TabletLetterLayout`）和九键「右侧大回车」的回车（`tallReturnKey`）；同一时间只显示其中一个，键面相同。
  private var returnKeys: [UIButton] {
    [enterButton, tabletReturnKey, tallReturnKey].compactMap { $0 }
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
      + (pronunciationLineShown ? 1 : 0)
  }

  /// 候选下面要不要为英文音标单独留一行：开关打开、音标表在，而且是走释义的方案（`glossesCandidates`）。按设置而不是按音标到没到来留，顶栏高度在打字时不变。
  private var pronunciationLineShown: Bool {
    CandidatePronunciationPreference.enabled && candidatePronunciationTableInstalled && glossesCandidates
  }

  private func refreshPronunciationTable() {
    let installed = CandidatePronunciationPreference.enabled && CandidatePronunciationPreference.tableInstalled(
      resources: session.candidateGlossResources(), stateRoot: MetasequoiaInputSessionBridge.sharedStateDirectory,
      language: CandidateTranslationPreference.primary.code.lowercased())
    guard installed != candidatePronunciationTableInstalled else { return }
    candidatePronunciationTableInstalled = installed
    invalidateCandidatePronunciations()
  }

  private func invalidateCandidatePronunciations() {
    candidatePronunciationEpoch &+= 1
    candidatePronunciations = [:]
    candidatePronunciationsPending = []
  }

  /// 一个候选查读音用的键：英文候选是它本身，中文候选是画出来的释义，按它的语言读。
  private func pronunciationLookupText(word: String, readingGloss: (language: String, gloss: String?)?) -> String? {
    CandidatePronunciationModel.lookupText(candidate: word, gloss: readingGloss?.gloss,
                                           language: readingGloss?.language ?? "en")
  }

  /// 读音跟着的那一行释义和它的语言（小写代码）：两种目标语言里有英语时读英语那一行，否则读主目标语言那一行；释义关着时为 nil。只有一行读音，所以只读一种语言。
  private func readingGloss(word: String, offline: String, source: Int, translated: Bool) -> (language: String, gloss: String?)? {
    guard CandidateGlossPreference.enabled, !CandidateTranslationStore.sourceGetsNoGloss(source) else { return nil }
    var languages = [CandidateTranslationPreference.primary]
    if let secondary = CandidateTranslationPreference.secondary { languages.append(secondary) }
    let language = languages.first(where: { $0.code == "EN" }) ?? CandidateTranslationPreference.primary
    return (language.code.lowercased(), gloss(word: word, language: language, offline: offline, translated: translated))
  }

  /// 已缓存的音标；还没查过的文字记进 `missing`，由调用方一并交给 `requestCandidatePronunciations`。
  private func cachedPronunciation(_ text: String?, missing: inout [String]) -> String {
    guard let text else { return "" }
    if let reading = candidatePronunciations[text] { return reading }
    if !candidatePronunciationsPending.contains(text) { missing.append(text) }
    return ""
  }

  /// 在释义工作队列上查还没缓存的音标，回答回到主线程后重画候选条和展开网格。表在本机，查询不联网；失败只是不画音标，不影响输入，也不记录查询的文字。
  private func requestCandidatePronunciations(_ texts: [String]) {
    guard pronunciationLineShown, let resources = session.candidateGlossResources(), !resources.isEmpty else { return }
    var batch: [String] = []
    for text in texts where candidatePronunciations[text] == nil && !candidatePronunciationsPending.contains(text)
      && !batch.contains(text) {
      batch.append(text)
      if batch.count >= CandidatePronunciationModel.maxItems { break }
    }
    guard !batch.isEmpty, let request = try? CandidatePronunciationModel.request(
      keys: batch, supplementaryResources: CandidatePronunciationPreference.supplementaryResources(
        stateRoot: MetasequoiaInputSessionBridge.sharedStateDirectory)) else { return }
    candidatePronunciationsPending.formUnion(batch)
    let epoch = candidatePronunciationEpoch
    candidateGlossQueue.async { [weak self] in
      let readings = (try? MetasequoiaInputSessionBridge.candidatePronunciations(request: request, resources: resources))
        .flatMap { try? CandidatePronunciationModel.decode($0) }
      DispatchQueue.main.async { [weak self] in
        guard let self, self.candidatePronunciationEpoch == epoch else { return }
        self.candidatePronunciationsPending.subtract(batch)
        // 整份失败时不缓存，下一次重画再查；成功时查不到的文字记作 ""，不再重复查。
        guard let readings else { return }
        if self.candidatePronunciations.count + batch.count > CandidatePronunciationModel.maxItems * 8 {
          self.candidatePronunciations = [:]
        }
        for text in batch { self.candidatePronunciations[text] = readings[text] ?? "" }
        guard batch.contains(where: { !(readings[$0] ?? "").isEmpty }) else { return }
        self.renderCandidateStrip()
        if self.candidatePanel != nil { self.refreshCandidatePanelAnnotations() }
      }
    }
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
      glossLines: glossLineCount, candidateScale: candidateFontScale, preeditScale: preeditFontScale,
      separateReadingRow: separateReadingRow, barScale: candidateBarScale)
  }

  /// 实际画出的顶栏高度。iOS 键盘扩展在一次键盘会话里保持这个高度，避免组字开始或结束时触发系统键盘重排。
  private var shownTopRowHeight: CGFloat { currentTopRowHeight }

  /// 读本机的工具栏设置；工具栏和顶栏在下一次 `updateShortcutButtons` 和 `renderCandidateStrip` 时跟着更新。
  private func readToolbarPreferences() {
    toolbarPhrases = TouchToolbarLocalPreference.phrases
    toolbarScheme = TouchToolbarLocalPreference.scheme
    toolbarHidden = TouchToolbarLocalPreference.hidden
  }

  /// 释义行数、候选和编码字号以及「候选栏大小」都决定顶栏有多高，所以在这里一起应用。
  private func applyCandidateGlossLayout() {
    refreshPronunciationTable()
    let lines = currentGlossLines()
    let tablet = formFactor == .tablet
    // 「候选栏大小」连同字号一起缩放候选栏（#10137），字号倍数在这里乘上它，下游画字的地方不用再管。
    let barScale = CandidateFontPreference.barScale(in: session.sharedPreferences)
    let candidateScale = CandidateFontPreference.candidateScale(in: session.sharedPreferences, tablet: tablet) * barScale
    let preeditScale = CandidateFontPreference.preeditScale(in: session.sharedPreferences, tablet: tablet) * barScale
    let families = CandidateFontPreference.families(in: session.sharedPreferences)
    guard lines != glossLineCount || candidateScale != candidateFontScale
      || preeditScale != preeditFontScale || barScale != candidateBarScale
      || families != candidateFontFamilies else { return }
    candidateFontFamilies = families
    candidateBarScale = barScale
    for constraint in candidateChipHeightConstraints {
      constraint.constant = Self.candidateChipMinimumSize.height * barScale
    }
    glossLineCount = lines
    if preeditScale != preeditFontScale {
      preeditFontScale = preeditScale
      preeditButton.configuration?.titleTextAttributesTransformer = Self.preeditTransformer(scale: preeditScale)
    }
    candidateFontScale = candidateScale
    updateCandidateCanvasMetrics()
    topRowHeightConstraint?.constant = shownTopRowHeight
    readingRowHeightConstraint?.constant = Self.readingRowHeight(preeditScale: preeditScale, barScale: barScale)
    updatePreferredKeyboardHeight()
    candidateButtonStyleRevision &+= 1
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
    case .nineKeyShuangpin: session.switchToNineKeyShuangpin()
    case .fourteenKey: session.switchToFourteenKey()
    case .eighteenKey: session.switchToEighteenKey()
    case .ziranma, .microsoft, .shoudao: switchShuangpinLayout(profile: inputScheme.rawValue)
    case .wubi: letterGrid == .fourteenKey ? session.switchToFourteenKeyWubi() : session.switchToWubi()
    case .japanese, .japaneseNineKey: session.switchToJapanese()
    case .korean: session.switchToKorean()
    case .cantonese, .cantoneseShuangpin: session.switchToCantonese()
    case .zhuyin: session.switchToZhuyin()
    case .vietnamese: session.switchToVietnamese()
    case .tibetan: session.switchToTibetan()
    case .stroke: session.switchToStroke()
    case .handwriting: session.switchToHandwriting()
    case .quanpin: session.switch(toShuangpin: usesShuangpin)
    case .shuangpin: switchShuangpinLayout(profile: "xiaohe")
    }
  }

  /// 双拼方案按已应用的布局开 26 键、14 键或 18 键。
  private func switchShuangpinLayout(profile: String) -> MetasequoiaInputSnapshot {
    switch letterGrid {
    case .fourteenKey: session.switchToFourteenKeyShuangpin(profile: profile)
    case .eighteenKey: session.switchToEighteenKeyShuangpin(profile: profile)
    case nil: session.switch(toShuangpinProfile: profile)
    }
  }

  /// 功能菜单的「手写」：切到手写，并记下进入前的方案和中英状态；正在手写时回到记下的那个（#9703）。原先这个图块只进不出，再点一次仍停在手写上，要从方案选择离开。与 Android、鸿蒙的磁贴一样来回切换。
  ///
  /// 英文盖在手写上时键盘画的是英文，这时点图块是要手写：只退出英文，进入前记下的方案仍然有效。
  private func toggleHandwritingScheme() {
    closeKeyboardPicker()
    if isChineseMode && inputScheme == .handwriting {
      let before = schemeBeforeHandwriting
      let back = InputSchemePreference.handwritingReturn(before: before, offered: InputSchemePreference.offeredSchemes)
      let english = back == before && englishBeforeHandwriting
      selectInputScheme(back)
      if english && isChineseMode && inputScheme == back { toggleInputMode() }
      return
    }
    let previous = inputScheme
    let english = !isChineseMode
    // 与在方案选择里选手写的步骤相同。
    if !isChineseMode { toggleInputMode() }
    selectInputScheme(.handwriting)
    if previous != .handwriting && inputScheme == .handwriting {
      schemeBeforeHandwriting = previous
      englishBeforeHandwriting = english
    }
  }

  private func selectInputScheme(_ scheme: ChineseInputScheme, persistShared: Bool = true) {
    guard InputSchemePreference.offeredSchemes.contains(scheme) else { return }
    if scheme == inputScheme { return }
    let previous = inputScheme
    // 换到别的方案：「手写」图块记下的方案作废，图块自己在这之后重新记。
    schemeBeforeHandwriting = nil
    playInputClick()
    let source = typingSource
    // An open Korean syllable, Zhuyin conversion or Vietnamese word is text the user already wrote, so leaving the scheme commits it; the switch below would otherwise discard it along with the marked text.
    if composesInPlace && hasComposition { render(session.finishComposition(), source: source) }
    handwriting.clear()
    inputScheme = scheme
    // 双拼入口必须先写选中标识，接下来的会话切换才会按双拼解释同形粤拼码。
    let savedCantoneseShuangpin = persistShared && scheme == .cantoneseShuangpin
    if savedCantoneseShuangpin && !session.setTouchKeyboardScheme(scheme, enabledSchemes: InputSchemePreference.enabledSchemes) {
      inputScheme = previous
      return
    }
    let snapshot = applyInputScheme()
    InputSchemePreference.scheme = scheme
    if persistShared && !savedCantoneseShuangpin {
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
    if hold.state == .began {
      // 路由在按下时固定，跳到主 App 后也不把同一次长按转成光标拖动。
      spaceHoldMovesCursor = !spaceVoiceEnabled
      if !spaceHoldMovesCursor {
        countKeyPress(TypingKeyID.space)
        playInputClick(.space)
        startVoiceFromSpace(held: true, originY: hold.location(in: view).y)
      }
    }
    if spaceHoldMovesCursor {
      handleSpaceCursorMovement(hold.state, at: hold.location(in: view).x,
                                velocity: 0, space: hold.view as? UIButton)
    } else if let dictation = voiceDictation, let originY = voiceHoldOriginY {
      // 按住说话：松开结束，上滑超过键盘高度的四分之一后松开取消（Android 的 `VoiceHoldPolicy`）。
      switch hold.state {
      case .changed:
        voicePanel?.setCancelling(Self.voiceHoldCancels(originY: originY, y: hold.location(in: view).y,
                                                        height: view.bounds.height))
      case .ended:
        voiceHoldOriginY = nil
        if voicePanel?.isCancelling == true { closeKeyboardPicker() } else { dictation.finish() }
      case .cancelled, .failed:
        closeKeyboardPicker()
      default:
        break
      }
    }
    if [.ended, .cancelled, .failed].contains(hold.state) { spaceHoldMovesCursor = false }
  }

  /// 对应 Android 的 `VoiceHoldPolicy.CANCEL_ZONE_FRACTION`。
  static let voiceCancelFraction: CGFloat = 0.25

  /// 按住说话时手指从按下处上滑超过键盘高度的四分之一就进入取消区。
  static func voiceHoldCancels(originY: CGFloat, y: CGFloat, height: CGFloat) -> Bool {
    originY - y > height * voiceCancelFraction
  }

  /// 长按空格说话。键盘扩展不能录音，录音识别由后台的主 App 做（`VoiceSessionService`）：会话开着时在键区显示聆听界面并开始听写，结果回来直接上屏；会话还没开或已经过期时打开 App 开启它，用户回来再按住空格。`held` 为假时由工具栏、功能菜单或读屏操作开始听写，点「说完了」结束。
  private func startVoiceFromSpace(held: Bool, originY: CGFloat? = nil) {
    guard hasFullAccess else {
      showDiagnostic("语音输入需要开启键盘的“允许完全访问”。")
      renderCandidateStrip()
      return
    }
    guard voiceInsertionReady else { return }
    closeKeyboardService()
    closeKeyboardPicker()
    let dictation = KeyboardVoiceDictation()
    guard dictation.sessionAvailable else {
      openVoiceSession()
      return
    }
    voiceDictationContext = KeyboardHostContext.documentIdentifier(for: textDocumentProxy).map {
      KeyboardDocumentContext(document: $0, before: textDocumentProxy.documentContextBeforeInput,
                              selected: textDocumentProxy.selectedText, after: textDocumentProxy.documentContextAfterInput)
    }
    voiceHoldOriginY = held ? originY : nil
    let panel = KeyboardVoicePanelView(dictationHeld: held, skin: KeyboardTheme.current)
    panel.onCancel = { [weak self] in self?.closeKeyboardPicker() }
    panel.onFinish = { [weak self] in self?.voiceDictation?.finish() }
    dictation.onEvent = { [weak self] event in self?.handleVoiceDictation(event) }
    voiceDictation = dictation
    voicePanel = panel
    // 按着的手指属于空格键，面板盖住键区时不能让这一行停止接收触摸，否则松开和上滑都收不到。
    installPanel(panel, keepsKeyTouches: held)
    dictation.start()
  }

  /// 打开 App 的语音会话页。打不开时说明原因，不让长按空格看起来毫无反应。
  private func openVoiceSession() {
    KeyboardAppLauncher.open(KeyboardAppLauncher.voiceSessionURL, from: self) { [weak self] opened in
      guard !opened else { return }
      DispatchQueue.main.async {
        self?.showDiagnostic("未能打开水杉，请手动打开水杉 App 开启语音。")
        self?.renderCandidateStrip()
      }
    }
  }

  private func handleVoiceDictation(_ event: KeyboardVoiceDictation.Event) {
    switch event {
    case .listening(let text):
      voicePanel?.showListening(text)
    case .level(let level):
      voicePanel?.setLevel(level)
    case .recognizing(let text, let message):
      voicePanel?.showRecognizing(text: text, message: message)
    case .finished(let text):
      let context = voiceDictationContext
      endVoiceDictation()
      insertVoiceResult(text, context: context)
    case .failed(let message, let reopensSession):
      endVoiceDictation()
      showDiagnostic(message)
      renderCandidateStrip()
      if reopensSession { openVoiceSession() }
    }
  }

  /// 听写已经结束：收起面板，不再向 App 发取消。
  private func endVoiceDictation() {
    voiceDictation = nil
    voiceDictationContext = nil
    voiceHoldOriginY = nil
    closeKeyboardPicker()
  }

  /// 输入位置还和听写开始时一样就直接上屏；变了就把结果留在语音交接里，可以在功能菜单的「语音结果」里插入（Android 留在语音结果面板里）。
  private func insertVoiceResult(_ text: String, context: KeyboardDocumentContext?) {
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
      showDiagnostic("没有听到内容，请靠近麦克风再试。")
      renderCandidateStrip()
      return
    }
    if hasFullAccess, voiceInsertionReady, let context,
       context.matches(document: KeyboardHostContext.documentIdentifier(for: textDocumentProxy),
                       before: textDocumentProxy.documentContextBeforeInput, selected: textDocumentProxy.selectedText,
                       after: textDocumentProxy.documentContextAfterInput) {
      insertOwnText(text, source: .voice)
      return
    }
    do {
      try VoiceTextHandoffStore().save(text)
      showDiagnostic("输入位置已变化，结果已保留，可在功能菜单的“语音结果”里插入。")
    } catch {
      showDiagnostic(error.localizedDescription)
    }
    renderCandidateStrip()
  }

  private func closeKeyboardService() {
    guard let panel = servicePanel else { return }
    servicePanel = nil
    panel.willMove(toParent: nil)
    panel.view.removeFromSuperview()
    panel.removeFromParent()
  }

  private func synchronizePersonalDictionary(force: Bool, fullAccess: Bool? = nil) {
    guard !hasComposition, !isInLocalMode, !synchronizingPersonalDictionary,
          fullAccess ?? hasFullAccess else { return }
    let store = PersonalDictionaryStore()
    // 这一轮有编辑写进了 Engine：结束后让词库同步记下来，包括后面一步失败的情况。
    var applied = false
    defer { if applied { recordDictionaryChange() } }
    do {
      let state = try store.read()
      guard force || state.needsKeyboardSynchronization || store.listingIsStale() else { return }
      synchronizingPersonalDictionary = true
      defer { synchronizingPersonalDictionary = false }
      try store.synchronize(apply: { request in
        try session.applyPersonalPrevious(request.previous?.bridgeValue, replacement: request.replacement?.bridgeValue,
                                          requestID: request.id)
        applied = true
      }, page: { request in
        try PersonalWordPage(bridgeResult: session.personalEntries(atOffset: UInt(request.offset), kind: request.kind,
                                                                   query: request.query))
      }, export: { request in
        try self.session.personalExport(kind: request.kind, format: request.format)
      })
    } catch PersonalDictionaryStore.StoreError.busy {
      // Another process owns this short transaction; the timer retries without interrupting typing.
    } catch {
      showDiagnostic(error.localizedDescription)
    }
  }

  /// 「我的词条」刚在键盘里变了（应用了 App 排队的编辑、候选栏删除了词条）。只在主线程拿会话准备好的选项，记录在后台做。
  private func recordDictionaryChange() {
    dictionarySync.recordLocalChange(options: try? session.dictionaryStateVersionOptions(), fullAccess: hasFullAccess,
                                     deviceName: IOSCloudClipboardDriver.deviceName(UIDevice.current))
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
    TouchShuangpinLayoutPreference.mirror(preferences)
    TouchWubiLayoutPreference.mirror(preferences)
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
    // 缺键按关闭，与共享默认相同；变了的话候选栏布局（`applyCandidateGlossLayout`）重查音标表、重算顶栏高度。
    let pronunciationEnabled = preferences[CandidatePronunciationPreference.sharedKey] as? Bool ?? false
    if pronunciationEnabled != CandidatePronunciationPreference.enabled {
      CandidatePronunciationPreference.enabled = pronunciationEnabled
      invalidateCandidatePronunciations()
    }
    // 缺键按「仅当前高亮」，与会话读到的范围一致（`InputHabitPreference.settings`）。
    CandidateTranslationPreference.translationScope = CandidateTranslationScope(
      sharedValue: preferences[InputHabitPreference.translationScopeKey])
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
    let skinChanged = KeyboardTheme.reload(preferences, dark: drawsDark) != previousTheme
    // 高度也抄进 App Group：原先只抄键距、行距和语音入口，高度只进了这个控制器，于是布局面板打开时显示、提交时写回的都是 App Group 里可能过时的高度。
    KeyboardLayoutPreference.mirrorGeometry(preferences)
    if let adjustment = KeyboardLayoutPreference.sharedHeightAdjustment(preferences["touch_keyboard_height_adjustment"]) {
      sharedKeyboardHeightAdjustment = CGFloat(adjustment)
    }
    KeyboardLayoutPreference.shuangpinKeyHints = KeyboardLayoutPreference.sharedShuangpinKeyHints(in: preferences)
    if shuangpinKeyHintsEnabled != KeyboardLayoutPreference.shuangpinKeyHints {
      shuangpinKeyHintsEnabled = KeyboardLayoutPreference.shuangpinKeyHints
      updateLetterCaseControls()
    }
    // 分词键的键面和点击都按它分支（usesQuanpinSeparator），变了立刻重画。
    KeyboardLayoutPreference.quanpinShift = KeyboardLayoutPreference.sharedQuanpinShift(in: preferences)
    if quanpinShiftEnabled != KeyboardLayoutPreference.quanpinShift {
      quanpinShiftEnabled = KeyboardLayoutPreference.quanpinShift
      updateLetterCaseControls()
    }
    // 「九键滑动输入数字」不改键面，下一次触摸判定时就用新值，不用重画。
    NineKeySwipeDigits.mirror(preferences)
    nineKeySwipeDigits = NineKeySwipeDigits(in: preferences)
    // 「按键布局」：样式或数字行变了，下面的 `updateKeyboardLayoutIfNeeded` 当场重排底行和数字行。只动键面，不碰会话，正在组的字照常继续。
    TouchKeyLayoutStyle.mirror(preferences)
    keyLayoutStyle = TouchKeyLayoutStyle(in: preferences)
    // 「大屏单手时侧边显示数字键盘」：变了当场重排单手那一行（`applyOneHanded` 只在排法变了时才动），同样不碰会话。
    oneHandedSidePanel = TouchOneHandedSidePanel(in: preferences)
    applyOneHanded()

    var selectedScheme: ChineseInputScheme?
    if let schemes = preferences["touch_keyboard_schemes"] as? [String: Any] {
      let enabled = (schemes["enabled"] as? [String] ?? []).compactMap(Self.sharedInputScheme)
      if !enabled.isEmpty { InputSchemePreference.enabledSchemes = enabled }
      selectedScheme = (schemes["selected"] as? String).flatMap(Self.sharedInputScheme)
    }
    if !hasComposition, let selectedScheme, InputSchemePreference.offeredSchemes.contains(selectedScheme) {
      selectInputScheme(selectedScheme, persistShared: false)
    }
    synchronizeShuangpinLayoutPreference()
    if inputScheme == .nineKeyShuangpin {
      nineKeyShuangpinLayout = try? .current(in: preferences)
    }
    if skinChanged { applyKeyboardSkin() }
    applyLayoutPreferences()
    updateKeyboardLayoutIfNeeded()
    updateShortcutButtons()
    updatePreferredKeyboardHeight()
    scheduleCandidateGlosses()
    renderCandidateStrip()
    return translationSettingsChanged
  }

  /// 组字结束后再切布局，与 Host API 延迟应用设置的时机一致；相同方案的 26/14 切换也要重画。双拼和五笔的布局各自保存，只有当前方案的网格变了才重新切会话。
  private func synchronizeShuangpinLayoutPreference() {
    guard !hasComposition, let preferences = session.sharedPreferences else { return }
    let nextShuangpin = TouchShuangpinLayoutPreference.layout(in: preferences)
    let nextWubi = TouchWubiLayoutPreference.layout(in: preferences)
    guard nextShuangpin != shuangpinLayout || nextWubi != wubiLayout else { return }
    let previousGrid = letterGrid
    shuangpinLayout = nextShuangpin
    wubiLayout = nextWubi
    if letterGrid != previousGrid {
      let source = typingSource
      render(applyInputScheme(), source: source)
      synchronizeReplyKeyboard()
    }
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

  /// 候选网格关闭时箭头朝下，打开时立即朝上。
  private func updateExpandRotation() {
    let transform: CGAffineTransform = candidatePanel != nil ? CGAffineTransform(rotationAngle: .pi) : .identity
    expandCandidatesButton.accessibilityLabel = candidatePanel != nil ? "收起候选" : "展开全部候选"
    guard expandCandidatesButton.transform != transform else { return }
    expandCandidatesButton.transform = transform
  }

  private func showCandidatePanel() {
    closeKeyboardService()
    closeKeyboardPicker()
    playInputClick()
    do {
      let allCandidates = try session.allCandidates()
      let snapshot = try CandidatePanelSnapshot.decode(allCandidates)
      guard !snapshot.entries.isEmpty else { return }
      let columns = opensNineKeyPanel ? makeNineKeyPanelColumns() : nil
      let panel = KeyboardCandidatePanelView(
        candidates: candidatePanelTexts(snapshot), preedit: snapshot.preedit,
        annotations: candidatePanelAnnotations(snapshot),
        markers: candidatePanelMarkers(snapshot),
        aiPlaceholders: candidatePanelAIPlaceholders(snapshot),
        confidentCorrections: candidatePanelConfidentCorrections(snapshot),
        candidateScale: candidateFontScale, preeditScale: preeditFontScale, candidateFamilies: candidateFontFamilies,
        showsHeader: false, palette: candidatePalette, canvas: candidateCanvasSkin,
        display: { $0 },
        menuElements: { [weak self] index in
          guard let self, let snapshot = candidatePanelSnapshot, snapshot.entries.indices.contains(index) else { return [] }
          let entry = snapshot.entries[index]
          return candidateMenuElements(
            generation: snapshot.generation, globalIndex: entry.index, candidate: entry.text,
            offlineGloss: entry.translation, source: entry.source, highlighted: entry.highlighted,
            fixedPosition: entry.fixedPosition)
        },
        onSelect: { [weak self] index in
          guard let self, let snapshot = candidatePanelSnapshot, snapshot.entries.indices.contains(index) else { return }
          selectPanelCandidate(generation: snapshot.generation, globalIndex: snapshot.entries[index].index)
        },
        onClose: { [weak self] in self?.closeKeyboardPicker() },
        columns: columns.map { ($0.leading, $0.trailing) })
      candidatePanel = panel
      refreshDecorationWords()
      candidatePanelSnapshot = snapshot
      nineKeyPanelColumns = columns
      // 九键三栏面板和普通候选网格一样装在键区：上面的候选栏和读音留着，用户看得到选拼音、筛选之后读音和首选怎么变。
      installPanel(panel)
      updateExpandRotation()
      updateNineKeyPanelColumns()
      // 收起时候选条只查了自己那一页的释义，网格里其余候选的释义在打开时补查，用的就是上面刚读出的这份完整列表。
      if visibleEmailSuffixes.isEmpty {
        candidateGlossTimer?.invalidate()
        candidateGlossTimer = nil
        requestCandidateGlosses(allCandidates: allCandidates)
      }
    } catch {
      showDiagnostic("候选列表暂不可用")
    }
  }

  /// 面板里点中一个候选。只选掉前面一段、组字还在时面板留着，换上剩下组字的全部候选，用户接着在面板里往下选（#9193，规则在 `CandidatePanelSelectionPolicy`）；整句上屏、组字清空或引擎拒绝了这次选择就收起，组字还在的话照旧清掉九键筛选。九键面板开着笔画筛选时顺带清掉笔画：它比的是刚被选掉的那个字。
  ///
  /// 先选再决定收不收：先收起面板再清筛选会换一代候选，这次选择就成了过期的。画这次快照时 `candidatePanelFollowsSelection` 让普通网格也按新的一代原地重建，不在换代时先收起。
  private func selectPanelCandidate(generation: UInt64, globalIndex: UInt64) {
    playInputClick()
    let nineKeyPanel = nineKeyPanelColumns != nil
    let snapshot = session.selectAnyCandidate(generation: generation, globalIndex: globalIndex)
    candidatePanelFollowsSelection = true
    render(snapshot)
    candidatePanelFollowsSelection = false
    guard candidatePanel != nil,
          CandidatePanelSelectionPolicy.keepsOpen(handled: snapshot.isHandled, editingText: snapshot.editingText) else {
      closeKeyboardPicker()
      return
    }
    if nineKeyPanel, nineKeyPanelColumns != nil,
       let strokes = CandidatePanelSelectionPolicy.strokesAfterPartialSelection(currentNineKeyStrokes) {
      setNineKeyFilter(singleCharacter: currentNineKeySingleCharacter, strokes: strokes)
    }
  }

  /// 全拼九键、拼音的 14 键或 18 键正在组字时，展开的是三栏的九键面板（单字和笔画筛选照常可用）；其他方案、五笔 14 键（引擎不给它拼音选择和筛选）、注音九键和本地模式使用铺满键区的候选网格，由顶部箭头收起。
  private var opensNineKeyPanel: Bool {
    (inputScheme.isMandarinNineKey || (usesLetterGrid && inputScheme != .wubi))
      && isChineseMode && hasComposition && !isInLocalMode
  }

  /// 面板格子上的字：打开和换代重建都按繁体输出偏好转换，换代后不能退回引擎原文（部分选词后普通网格也走换代重建，#9193）。
  private func candidatePanelTexts(_ snapshot: CandidatePanelSnapshot) -> [String] {
    snapshot.entries.map { chineseOutput($0.text, source: $0.source) }
  }

  private func candidatePanelAnnotations(_ snapshot: CandidatePanelSnapshot) -> [KeyboardCandidateAnnotation] {
    var missing: [String] = []
    defer { if !missing.isEmpty { requestCandidatePronunciations(missing) } }
    return snapshot.entries.map {
      candidatePanelAnnotation(code: $0.code, gloss: $0.translation, word: $0.text, engine: $0.annotation,
                               source: $0.source, highlighted: $0.highlighted, typed: snapshot.preedit,
                               missingPronunciations: &missing)
    }
  }

  private func candidatePanelMarkers(_ snapshot: CandidatePanelSnapshot) -> [[CandidateMarker]] {
    snapshot.entries.map { CandidateMarker.markers(source: $0.source, fixedPosition: $0.fixedPosition) }
  }

  /// 网格里哪些格子是在线占位行，和候选条一样画成次要色。
  private func candidatePanelAIPlaceholders(_ snapshot: CandidatePanelSnapshot) -> Set<Int> {
    Set(snapshot.entries.indices.filter {
      snapshot.entries[$0].source == CandidateMarker.aiPendingSource ||
      snapshot.entries[$0].source == CandidateMarker.cloudPendingSource
    })
  }

  /// 网格里哪些格子是坐了首选的双拼纠错，和候选条一样用强调色画、读屏念「已纠正」。
  private func candidatePanelConfidentCorrections(_ snapshot: CandidatePanelSnapshot) -> Set<Int> {
    Set(snapshot.entries.indices.filter { snapshot.entries[$0].confidentCorrection })
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
      chooseVisibleNineKeySpelling(at: UInt(index))
    case .stroke(let key):
      playInputClick()
      setNineKeyFilter(singleCharacter: currentNineKeySingleCharacter, strokes: currentNineKeyStrokes + key)
    case .back:
      playInputClick()
      closeKeyboardPicker()
    case .delete:
      // 笔画模式下先删笔画，删完了才删拼音。
      if nineKeyPanelStrokeMode && !currentNineKeyStrokes.isEmpty {
        playInputClick(.backspace)
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
    updateExpandRotation()
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
    (trigger: "C", title: "拆字", key: "chaizi"),
    // 「/ 指令」不是 Shift 加字母：中文标点下没有输入时按 / 打开，`openLocalInputMode` 把它当标点交给会话。指令表插件在这里起作用。
    (trigger: "/", title: "指令", key: "command"),
  ]

  /// 默认关闭的模式，与 client-core 的 `LocalModePreferences` 相同：文档里没写时 / 指令是关的，其余是开的。
  static let localInputModesOffByDefault: Set<String> = ["command"]

  /// The modes the settings app leaves on. The Engine refuses to open a mode turned off there, so offering one would be a menu entry that does nothing.
  private var enabledLocalInputModes: [(trigger: String, title: String, key: String)] {
    Self.enabledLocalInputModes(in: session.sharedPreferences)
  }

  /// 文档里开着的本地输入模式，按菜单顺序。不是 private：测试会固定它。
  static func enabledLocalInputModes(in preferences: [String: Any]?) -> [(trigger: String, title: String, key: String)] {
    let stored = preferences?["local_modes"] as? [String: Any] ?? [:]
    return localInputModes.filter { stored[$0.key] as? Bool ?? !localInputModesOffByDefault.contains($0.key) }
  }

  private func updatePreeditButton() {
    let idle = visiblePreedit.isEmpty
    let modeName = Self.localInputModes.first { $0.trigger == localModeTrigger }?.title
    let title = isInLocalMode && visiblePreedit == localModeTrigger
      ? (modeName ?? visiblePreedit)
      : (idle ? (isChineseMode ? "水杉输入法" : "英文输入") : visiblePreedit)
    // 拼写写进输入框（`showInlineComposition`），候选栏不再重复画它，读音那一段收成零宽，候选从行首开始，不会随拼写变长被推着走。只改画出来的标题：`visiblePreedit` 仍表示正在组字，拼写还没有候选时候选栏照样显示，VoiceOver 仍读出完整标题。日语读音和就地组字的方案不是拼写，照常画。全拼 14 键不写输入框，读音仍单独一行画在候选上面。
    let style: CandidatePreeditStyle = separateReadingRow ? .pinyin : .empty
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
    // 读音并入候选行时，没有在组的内容、或「候选栏预编辑」选了不显示而标题为空时，这一段不占位置：单独一行时那里显示的「水杉输入法」和它的本地输入菜单，在候选行里只会挤掉联想候选，本地输入仍可从「更多」里打开。
    let collapsed = (idle || drawnTitle.isEmpty) && !separateReadingRow
    inlineReadingCollapse?.isActive = collapsed
    if preeditButton.isHidden != collapsed { preeditButton.isHidden = collapsed }

    let modes = idle && supportsLocalTools ? enabledLocalInputModes : []
    let offersModes = idle && supportsLocalTools && !modes.isEmpty
    let spellingMenu = idle ? nil : editableSpelling.map(Self.spellingEditMenu)
    let menuKey = offersModes
      ? "local|" + modes.map { "\($0.trigger):\($0.key)" }.joined(separator: ",")
      : spellingMenu.map { items in
        "spelling|" + items.map { "\($0.title):\($0.enabled ? 1 : 0)" }.joined(separator: ",")
      } ?? "none"
    if appliedPreeditMenuKey != menuKey {
      appliedPreeditMenuKey = menuKey
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
    // / 指令要中文标点、没有输入；英文标点下按 / 只会写出一个斜杠，所以先问会话 `/` 此刻能不能打开它。
    if trigger == "/" && !session.slashOpensCommandMode() {
      showDiagnostic("切到中文标点后才能打开指令（/ 模式）")
      renderCandidateStrip()
      return
    }
    localModeTrigger = trigger
    showsSymbols = false
    render(trigger == "/" ? session.handlePunctuation("/") : session.openLocalMode(trigger))
  }

  private func chineseOutput(_ text: String, source: Int? = nil) -> String {
    if source == 18 { return text }
    if !inputScheme.writesChinese || localModeTrigger == "R" { return text }
    return ChineseTextConversion.outputString(text, traditional: usesTraditionalOutput)
  }

  private func updateSchemeButton() {
    // The hints come from the engine's own profile for the scheme the session is running, so they
    // are refreshed wherever the scheme is, and cannot drift from what the keys actually produce.
    shuangpinKeyHints = session.shuangpinKeyHints()
    nineKeyShuangpinLayout = inputScheme == .nineKeyShuangpin ? try? .current(in: session.sharedPreferences) : nil
    updateLetterCaseControls()

    // 输入方式画设计稿的键盘线框图标；方案本身作为按钮的值朗读，并在它打开的选择器里显示。
    schemeButton.apply(skin: KeyboardTheme.current)
    schemeButton.accessibilityIdentifier = "schemeButton"
    schemeButton.accessibilityLabel = "选择输入方案"
    schemeButton.accessibilityValue = inputScheme.title
    updateKeyboardLayout()
  }

  private func toggleLayout() {
    // 大千注音 #+= 页（共享符号行）底行的这个键印 123，回到数字九宫格，与设计稿 #+= 层第三排的 123 相同。
    if dachenMoreSymbolsShown {
      toggleMoreSymbols()
      return
    }
    playInputClick()
    showsSymbols.toggle()
    updateKeyboardLayout()
    // 全拼的分词键只在字母层成立；数字页上的输入会把它按符号层重画成 ⇧，回到字母层时要按字母层重画。
    updateLetterCaseControls()
  }

  private var usesCustomQuickPunctuation: Bool {
    isChineseMode && !isInLocalMode && !inputScheme.writesAsciiPunctuation && !writesJapaneseComma
      && KeyboardLayoutPreference.quickPunctuationSymbols != KeyboardLayoutPreference.defaultQuickPunctuationSymbols
  }

  private var quickPunctuationSymbols: [String] {
    // Korean and Vietnamese write half-width ASCII marks, the same menu English offers.
    guard isChineseMode, !isInLocalMode, !inputScheme.writesAsciiPunctuation else { return [",", ".", "?", "!", ":", ";", "@"] }
    if inputScheme.isJapanese { return ["、", "。", "？", "！", "「", "」", "・"] }
    return KeyboardLayoutPreference.quickPunctuationSymbols
  }

  private func updateLetterRowInsets() {
    guard letterRowViews.count > 1, let row = letterRowViews[1] as? UIStackView else { return }
    // 手机的中间一排两侧各缩进半个键，显示 `;` 键时（微软双拼、粤语双拼的中文模式，见 `showsSemicolonKey`）用它填上这块空位，`;` 藏起来时缩进回来；iPad 的中间一排缩进 2.5% 开始，以回车结尾（`TabletLetterLayout`）。
    // 这个比例按按键区域本身的宽度计算，单手模式会把它收窄（iPad 单手时收到手机大小的一块里的按键宽度）；键面形态决定是 iPad 的缩进还是手机的半键缩进。
    let keysWidth = KeyboardOneHandLayout.keysWidth(
      available: view.bounds.width - formFactor.padding.leading - formFactor.padding.trailing, mode: appliedOneHanded ?? .off,
      formFactor: formFactor)
    let margins: UIEdgeInsets
    if keyFaceFormFactor == .tablet {
      margins = UIEdgeInsets(top: 0, left: keysWidth * TabletLetterLayout.middleRowLeadingInset, bottom: 0, right: 0)
    } else {
      let inset: CGFloat = KeyboardLayoutPreference.geometry.centeredLetters && !showsSemicolonKey
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
    keyColumn.spacing = keyRowSpacing
    nineGrid?.spacing = layout.rowSpacing
    nineControls?.spacing = layout.rowSpacing
    numberPadSidebar.spacing = layout.rowSpacing
    numberPadOperators.spacing = layout.rowSpacing
    // 侧边数字键盘的行距和键距与九宫格相同，三行才逐行对得齐。
    oneHandSidePadGrid.spacing = layout.rowSpacing
    oneHandSidePadStack.spacing = keyRowSpacing
    strokeKeys?.applySpacing(row: layout.rowSpacing, key: layout.keySpacing)
    nineKeyHeight.constant = layout.rowSpacing * 2
    japaneseHeight?.constant = Self.japaneseKeyBlockHeight
    // 分几步拼出来：写成一个长的 + 表达式时，CI 上的编译器会在类型推断上超时。
    var keyRows: [UIView] = letterRowViews
    if let numberRowView { keyRows.append(numberRowView) }
    if let symbolExtraRowView { keyRows.append(symbolExtraRowView) }
    keyRows += zhuyinRowViews
    keyRows += fourteenKeyRowViews
    keyRows += eighteenKeyRowViews
    keyRows += symbolRowViews
    keyRows += symbolLayerRowViews as [UIView]
    keyRows += nineKeyRows
    keyRows += oneHandSidePadRows as [UIView]
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

  /// `updateKeyboardLayout` 的全部依赖；任一状态变化都必须使布局缓存失效。
  /// 工具栏表情入口也决定数字页底行的按钮，不能只记录几何与输入方案。
  private struct KeyboardLayoutInputs: Equatable {
    let chinese: Bool
    let scheme: ChineseInputScheme
    /// 正在画的字母网格；同一双拼方案在 14 键和 18 键之间切换也要重新布局。
    let letterGrid: LetterKeyGrid?
    let localMode: String
    let symbols: Bool
    let moreSymbols: Bool
    let toolbarEmojiAvailable: Bool
    let globe: Bool
    let geometry: KeyboardGeometry
    let formFactor: KeyboardFormFactor
    /// 键面按哪种形态画（iPad 单手时是手机的），见 `keyFaceFormFactor`。
    let keyFace: KeyboardFormFactor
    let fullKeys: Bool
    let split: Bool
    /// 「按键布局」的样式与数字行，以及数字行此刻放不放得下（手机横屏不画）。
    let keyStyle: TouchKeyLayoutStyle
    let numberRowFits: Bool
  }

  /// 字母键面只有在这些输入变化时才需要重写。候选变化不会影响键面，连续打字时可以复用上一次配置。
  private struct LetterCaseInputs: Equatable {
    let chinese: Bool
    let scheme: ChineseInputScheme
    let state: LetterCaseState
    let localMode: String
    let entersHelpcode: Bool
    let casedLetters: Bool
    let korean: Bool
    let swipeSymbols: Bool
    let shuangpinHints: Bool
  }

  private struct SpellingStripInputs: Equatable {
    let spellings: [String]
    let inReadingRow: Bool
  }

  private var layoutInputs: KeyboardLayoutInputs {
    KeyboardLayoutInputs(
      chinese: isChineseMode, scheme: inputScheme, letterGrid: typesLetterGrid ? letterGrid : nil, localMode: currentLocalMode,
      symbols: showsSymbols, moreSymbols: showsMoreSymbols,
      toolbarEmojiAvailable: toolbarEmojiAvailable, globe: needsInputModeSwitchKey,
      geometry: KeyboardLayoutPreference.geometry,
      formFactor: formFactor, keyFace: keyFaceFormFactor, fullKeys: KeyboardLayoutPreference.tabletFullKeys,
      split: wantsSplitKeyboard, keyStyle: activeKeyLayoutStyle, numberRowFits: numberRowFits)
  }

  private var toolbarEmojiAvailable: Bool {
    !toolbarHidden && TouchToolbarPreference(in: session.sharedPreferences).emoji
  }

  /// 键盘此刻是否横屏。有窗口场景时按它的界面方向；没有时（例如测试里还没进窗口的控制器）退回到竖直 size class：手机横屏是 compact，iPad 两个方向都是 regular，于是按竖屏处理，不会误画分离式键盘。测试用 `traitOverrides` 把它设成 compact 来模拟 iPad 横屏。
  private var isLandscape: Bool {
    view.window?.windowScene?.interfaceOrientation.isLandscape ?? (traitCollection.verticalSizeClass == .compact)
  }

  /// 此刻该不该画横屏分离式键盘：iPad（平板形态、横屏、iPad 的开关打开）或 iPhone（横屏、屏幕够宽、iPhone 的开关打开，`KeyboardSplitLayout.phoneIsActive`），而且当前显示的是会分开的字母布局。开关放在最后读，竖屏不读 App Group。
  private var wantsSplitKeyboard: Bool {
    guard !numberPadShown, !typesLetterGrid else { return false }
    let formFactor = formFactor
    let landscape = isLandscape
    let active = KeyboardSplitLayout.isActive(
      formFactor: formFactor, landscape: landscape, enabled: KeyboardLayoutPreference.tabletSplit)
      || KeyboardSplitLayout.phoneIsActive(
        idiom: traitCollection.userInterfaceIdiom, formFactor: formFactor, landscape: landscape,
        rowWidth: view.bounds.width - formFactor.padding.leading - formFactor.padding.trailing,
        keySpacing: CGFloat(KeyboardLayoutPreference.keySpacing), enabled: KeyboardLayoutPreference.phoneLandscapeSplit)
    guard active else { return false }
    return KeyboardSplitLayout.splitsLayout(scheme: inputScheme, chinese: isChineseMode, localMode: isInLocalMode)
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
    updateNineKeySidebar()
    let inputs = layoutInputs
    guard appliedLayoutInputs != inputs else { return }
    appliedLayoutInputs = inputs
    updateKeyboardLayout()
  }

  /// 拼音与标点只交换侧栏内容，不改变网格和底行约束。字母层用自定义符号表时标点列换成 `customSidebarScroll`，组字时同样被拼音盖住。
  private func updateNineKeySidebar() {
    let hasSpellings = sidebarCarriesSpellings
    let numberPad = numberPadShown
    let custom = !builtLetterSidebar.isEmpty
    spellingScrollView.isHidden = !hasSpellings
    punctuationStack.isHidden = hasSpellings || numberPad || custom
    if customSidebarScroll.isHidden != (hasSpellings || numberPad || !custom) {
      customSidebarScroll.isHidden = hasSpellings || numberPad || !custom
    }
    numberPadSidebar.isHidden = !numberPad
  }

  private func updateSymbolKeyFaces() {
    // Chinese punctuation only appears in Chinese mode. Local utilities and dedicated English input send the literal ASCII key value, so their labels must follow their insertion path. Korean and Vietnamese write ASCII marks too. Zhuyin's panel writes its marks itself, by the punctuation switch, so its faces follow that switch too.
    let sendsChinesePunctuation = isChineseMode && !isInLocalMode && !inputScheme.writesAsciiPunctuation
      && (!typesZhuyin || zhuyinWritesChinesePunctuation)
    // 不在手机 26 键底行时，， 键是快捷标点键，键面是第一个快捷标点（`updateKeyboardLayout`）。
    // 日语的逗号写作 、，手机底行的逗号键就画 、，按下也发 、。
    let japaneseComma = writesJapaneseComma
    for face in symbolKeyFaces where face.key !== quickPunctuationButton
      || ((phoneBottomRowShown || nineKeyPunctuationRowShown) && !usesCustomQuickPunctuation) {
      let styled = Self.chineseSymbolFace(
        face.ascii, cornerQuotes: currentCornerQuotes, hollowBrackets: currentHollowBrackets,
        enumerationCommaBackslash: currentEnumerationCommaBackslash,
        enumerationCommaSlash: currentEnumerationCommaSlash) ?? face.chinese
      let chinese = japaneseComma && face.key === quickPunctuationButton ? "、" : styled
      let title = sendsChinesePunctuation ? chinese : face.ascii
      guard face.key.configuration?.title != title || face.key.accessibilityLabel != "符号 \(title)" else { continue }
      face.key.configuration?.title = title
      face.key.accessibilityLabel = "符号 \(title)"
    }
    // 26 键的 ， 长按输入的句号跟随句号键面；手写底行的 ， 长按输入的是 ？，不在这里改。
    if let key = quickPunctuationButton as? KeyboardKeyButton, key.cornerHint != nil, handwriting.isHidden {
      key.cornerHint = periodKey.configuration?.title ?? "."
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

  /// 大千注音、14 键和 18 键的各排第一次要画时才建，建好后一直留着。首次出现的键盘原来把每种方案的键都建好再藏起来，两百多个按键里这三种占了将近八十个，挡在第一帧前面，而大多数人从来不用它们。
  ///
  /// 在 `updateKeyboardLayout` 的最前面调用，后面的键距、iPad 宽度和显隐都照常落到新建的行上。
  private func ensureAlternateLetterRows() {
    guard actionRow != nil, isChineseMode, !isInLocalMode, !showsSymbols else { return }
    if inputScheme.isZhuyin && zhuyinRowViews.isEmpty {
      zhuyinRowViews = ZhuyinKeyLayout.rows.enumerated().map { index, row in
        makeZhuyinRow(row, includesDelete: index == ZhuyinKeyLayout.rows.count - 1)
      }
      insertAlternateLetterRows(zhuyinRowViews, before: fourteenKeyRowViews.first ?? eighteenKeyRowViews.first ?? nineKeyContainer)
    }
    switch letterGrid {
    case .fourteenKey where fourteenKeyRowViews.isEmpty:
      fourteenKeyRowViews = FourteenKeyLayout.rows.enumerated().map { index, row in
        makeLetterGridRow(row, grid: .fourteenKey, includesEdges: index == FourteenKeyLayout.rows.count - 1)
      }
      insertAlternateLetterRows(fourteenKeyRowViews, before: eighteenKeyRowViews.first ?? nineKeyContainer)
    case .eighteenKey where eighteenKeyRowViews.isEmpty:
      eighteenKeyRowViews = EighteenKeyLayout.rows.enumerated().map { index, row in
        makeLetterGridRow(row, grid: .eighteenKey, includesEdges: index == EighteenKeyLayout.rows.count - 1)
      }
      insertAlternateLetterRows(eighteenKeyRowViews, before: nineKeyContainer)
    default: break
    }
  }

  /// 把后建的几排按原来的顺序插进按键列（字母行之后、九键外框之前），先藏着，并补上原来在 `installKeyboard` 里给的东西：与底行等高的约束（`updateKeyboardLayout` 末尾按显隐启用）和当前键距（`applyLayoutPreferences` 只在几何变化时才重设）。
  private func insertAlternateLetterRows(_ rows: [UIView], before anchor: UIView) {
    var index = keyColumn.arrangedSubviews.firstIndex(of: anchor) ?? keyColumn.arrangedSubviews.count
    for row in rows {
      row.isHidden = true
      if let keySpacing = appliedLayout?.keySpacing { (row as? UIStackView)?.spacing = keySpacing }
      keyColumn.insertArrangedSubview(row, at: index)
      index += 1
      standardRowHeights.append((row, row.heightAnchor.constraint(equalTo: actionRow.heightAnchor)))
    }
    // `decorateKey` 建键时按键还不在窗口里，动态的阴影色按默认外观解析；首屏建的键由 `viewWillAppear` 的 `applyKeyboardSkin` 在窗口里重新解析，键盘开着时才建的行在这里补上。
    let skin = KeyboardTheme.current
    guard view.window != nil, skin.design == nil else { return }
    for case let button as UIButton in rows.flatMap(\.subviews) {
      button.layer.shadowColor = skin.shadowColor.resolvedColor(with: button.traitCollection).cgColor
    }
  }

  private func updateKeyboardLayout() {
    ensureAlternateLetterRows()
    // 分离与否先定下来：iPad 画横屏分离式键盘时单手让位，单手又决定下面各处按哪种键面画（`keyFaceFormFactor`）。是否分离只看方案、层和设备形态，不看键面。
    splitKeyboardShown = wantsSplitKeyboard
    applyLayoutPreferences()
    applyKeyboardMetrics()
    applyOneHanded()
    standardRowHeights.forEach { $0.1.isActive = false }
    microsoftFinalKey?.isHidden = !showsSemicolonKey
    // 假名九键的键面（`kanaFace`）只在字母层画假名网格（`kana`）；123 和别的键面一样打开数字九宫格，#+= 是设计稿的符号层，两者都用底行，假名网格自己的数字层不再出现。
    let kanaFace = isChineseMode && inputScheme == .japaneseNineKey && !isInLocalMode
    let kana = kanaFace && !showsSymbols
    japaneseKeys?.isHidden = !kana
    japaneseHeight?.constant = Self.japaneseKeyBlockHeight
    japaneseHeight?.isActive = kana
    japaneseKeys?.applyLayout()
    actionRow?.isHidden = kana
    japaneseGlobeButton?.isHidden = !needsInputModeSwitchKey
    japaneseKeys?.setModeColumnFull(needsInputModeSwitchKey)
    let nineKey = isChineseMode && inputScheme.isMandarinNineKey && !isInLocalMode
    let writes = isChineseMode && inputScheme == .handwriting && !showsSymbols && !isInLocalMode
    if !writes && !handwriting.isHidden { handwriting.deactivate() }
    handwriting.isHidden = !writes
    if writes { handwriting.activate() }
    handwritingActionHeight?.isActive = writes
    // 大千把数字行和四个标点键都用作注音键，所以注音在字母层画自己的四排，代替字母行和数字行；123 与别的键面一样打开数字九宫格，#+= 是原来每排十键的共享符号行（`symbolRowViews`），不是设计稿的符号层。
    let dachen = isChineseMode && inputScheme.isZhuyin && !isInLocalMode
    zhuyinRowViews.forEach { $0.isHidden = !dachen || showsSymbols }
    // 笔画字母层沿用九键外框，数字入口与其他中文方案共用数字九宫格。
    let strokes = isChineseMode && inputScheme.isStroke && !isInLocalMode
    let strokePad = strokes && !showsSymbols
    // 全拼和双拼 14 键、18 键只在字母层画自己的三排键，数字入口共用九宫格。
    let fourteenKey = typesLetterGrid
    let fourteenKeyRows = fourteenKey && !showsSymbols
    let shownGrid = fourteenKeyRows ? letterGrid : nil
    for row in fourteenKeyRowViews where row.isHidden == (shownGrid == .fourteenKey) { row.isHidden = shownGrid != .fourteenKey }
    for row in eighteenKeyRowViews where row.isHidden == (shownGrid == .eighteenKey) { row.isHidden = shownGrid != .eighteenKey }
    updateFourteenKeySeparator()
    let semicolon = fourteenKey && inputScheme == .microsoft
    for (button, key) in letterGridButtons {
      button.configuration?.title = key.face(semicolon: semicolon)
      button.accessibilityLabel = key.accessibilityLabel(semicolon: semicolon)
    }
    let numberPad = numberPadShown
    let nineKeyFrame = (nineKey && !showsSymbols) || strokePad || numberPad
    let symbolLayer = Self.drawsSymbolLayer(symbols: showsSymbols, numberPad: numberPad, dachen: dachen)
    let letterRowsShown = !(showsSymbols || nineKeyFrame || writes || kana || dachen || fourteenKey)
    letterRowViews.forEach { $0.isHidden = !letterRowsShown }
    keyColumn.spacing = keyRowSpacing
    oneHandSidePadStack.spacing = keyRowSpacing
    applyTabletLetterKeys()
    let fullKeys = keyFaceFormFactor.canShowFullKeys && KeyboardLayoutPreference.tabletFullKeys
    // 「数字行与 Tab 键」的数字行按键盘形态取：iPad 单手时键面是手机的、不画 Tab，1 到 0 这一排照画，排数与双手时相同，单手开关不改键盘高度。
    let fullKeyNumberRow = formFactor.canShowFullKeys && KeyboardLayoutPreference.tabletFullKeys
    let sourceHandwriting = isChineseMode && inputScheme == .handwriting && !isInLocalMode
    // 「按键布局」的顶部数字行（`touch_number_row`）：26 键、14 键和 18 键的字母层上方一排 1 到 0，手机横屏不画；这些键面的数字和符号页用补充行保持同样的五排高度，切层时键盘不跳。iPad 没开「数字行与 Tab 键」时与手机一样按它画，与 Android、HarmonyOS 的平板相同；开着时 iPad 全尺寸 26 键画自己的数字行和 Tab，14 键和 18 键仍按它画。
    let touchNumberRow = keyLayoutStyle.numberRow && numberRowFits
    let numberRowShown = !showsSymbols
      && ((letterRowsShown && (fullKeyNumberRow || touchNumberRow)) || (fourteenKeyRows && touchNumberRow))
    if numberRowView?.isHidden == numberRowShown { numberRowView?.isHidden = !numberRowShown }
    let extraRowShown = showsSymbols && !nineKey && !sourceHandwriting && !kanaFace && !dachen && !strokes
      && (fourteenKey ? touchNumberRow : fullKeyNumberRow || touchNumberRow)
    if symbolExtraRowView?.isHidden == extraRowShown { symbolExtraRowView?.isHidden = !extraRowShown }
    // 上下留白按排数算，要等数字行和补充行这一趟的显隐定下来再算，否则数字行刚出现或刚收起时会沿用上一趟的排数。
    applyPhoneLetterVerticalPadding()
    tabKey?.isHidden = !fullKeys
    let rowPunctuation = keyFaceFormFactor.showsLetterRowPunctuation
    for key in letterRowPunctuationKeys where key.isHidden == rowPunctuation { key.isHidden = !rowPunctuation }
    // 横屏分离式键盘：每排的中缝和右半的第二个空格一起显示或隐藏。九键、笔画、假名、大千和手写不分（wantsSplitKeyboard），它们的符号页也跟着不分。
    let split = splitKeyboardShown
    let tabletFace = keyFaceFormFactor == .tablet
    for gap in splitGaps {
      let shown = split && !(gap === tabletOnlySplitGap && !tabletFace) && !(gap === phoneOnlySplitGap && tabletFace)
      if gap.isHidden == shown { gap.isHidden = !shown }
    }
    if splitSpaceButton?.isHidden == split { splitSpaceButton?.isHidden = !split }
    // 数字页复用三列网格，原方案与 Engine 的组码布局不变。
    let nineKeyDigits = numberPad
    nineKeyContainer.isHidden = !nineKeyFrame
    nineGrid?.isHidden = strokePad
    strokeKeys?.isHidden = !strokePad
    nineKeyRows.forEach { $0.isHidden = !nineKeyFrame || strokePad }
    applyNineKeyDigitLayer(nineKeyDigits)
    // 九键「右侧大回车」：拼音九键、九键双拼和笔画自己的字母层和数字层；从别的键面借来的数字九宫格不跟九键的样式。
    let tallReturn = nineKeyFrame && (nineKey || strokes) && activeKeyLayoutStyle.nineKey == .tallReturn
    applyNineKeyTallReturn(tallReturn)
    // 数字键盘「空格在右列」（`touch_number_pad_space`）：右列第三格是空格，底行没有空格；右侧大回车有自己的数字层，不受它影响。
    let numberPadSpace = activeKeyLayoutStyle.numberPadSpace
    let columnSpace = numberPad && !tallReturn && numberPadSpace == .rightColumn
    applyNumberPadColumnSpace(columnSpace)
    applyNineKeyClosingCovered(tallReturn || columnSpace)
    // 「符」与「123」互换过的字母层底行：手机 26 键底行原来以 123 | 符 打头，换后 符 | 123。英文、本地模式、日文罗马字和写 ASCII 标点的方案的 26 键都是这样，中文 26 键只有标准和逗号句号在空格两侧两种样式是；长空格和表情键样式没有 符，14/18 键、手写、九键、笔画、假名和大千的字母层底行不以 123 | 符 打头。
    let letterFaceSymbolFirst = keyFaceFormFactor == .phone && activeKeyLayoutStyle.symbolFirst
      && !nineKey && !strokes && !fourteenKey && !sourceHandwriting && !kanaFace && !dachen
      && (!drawsChineseTwentySixKey || [.standard, .commaPeriod].contains(activeKeyLayoutStyle.twentySixKey))
    // 从这样的字母层借来的数字九宫格按九键一族的次序排（符 | 返回），返回键仍在字母层 123 那一格。
    let symbolFirstSource = numberPad && letterFaceSymbolFirst
    // 同一条规则用在 #+= 层：这一层的返回键排在字母层 123 那一格（😀 或 符号 | 返回）。
    let symbolLayerSymbolFirst = symbolLayer && letterFaceSymbolFirst
    updateNineKeyMiddleKey()
    applySidebarSymbols(activeKeyLayoutStyle)
    updateNineKeySidebar()
    if actionRow != nil {
      let phoneRow = Self.usesPhoneBottomRow(formFactor: keyFaceFormFactor, nineKeyFrame: nineKeyFrame, handwriting: writes,
                                             kana: kana, fourteenKey: fourteenKeyRows)
      let tabletRow = Self.usesTabletBottomRow(formFactor: keyFaceFormFactor, letterRows: letterRowsShown)
      let style: ActionRowStyle = symbolLayer ? .symbolLayer : nineKeyFrame ? .nineKey : tabletRow ? .tablet : phoneRow ? .phone : .standard
      phoneBottomRowShown = style == .phone
      nineKeyHeight.isActive = nineKeyFrame
      let showsLanguage = KeyboardLayoutPreference.geometry.showsBottomLanguage
      let styledRow = styledBottomRow(style: style, letterRows: letterRowsShown, letterGrid: fourteenKeyRows,
                                      numberPad: numberPad, nineKeyDigits: numberPad && (nineKey || strokes || symbolFirstSource),
                                      tallReturn: tallReturn, language: showsLanguage)
      // 「中/英键放在空格左边」（`touch_language_key_side`）：手机底行（26 键一族、14/18 键、大千、手写）和九键外框（字母层、数字层、借来的数字九宫格、右侧大回车）在其余排法之后把中/英排到空格左边；iPad 全尺寸底行、123 / #+= 层不动，隐藏了中/英键时不起作用。
      let languageLeft = activeKeyLayoutStyle.languageKeySide == .left && showsLanguage && (style == .phone || style == .nineKey)
      let styledSlots = Set(styledRow?.map(\.slot) ?? [])
      // 数字层底行带 符 时（拼音九键、九键双拼和笔画的数字层，含右侧大回车），符号栏里不再放第二个 符；借来的数字九宫格仍在符号栏第三排放 符。
      let sidebarSymbols = !(numberPad && styledSlots.contains(.symbols))
      if numberPadSymbolsKey?.isHidden == sidebarSymbols { numberPadSymbolsKey?.isHidden = !sidebarSymbols }
      nineKeyPunctuationRowShown = style == .nineKey && styledSlots.contains(.comma)
      arrangeActionRow(style, commaSecondary: style == .phone && letterRowsShown, numberPad: numberPad, styled: styledRow,
                       tallReturn: tallReturn, symbolFirst: symbolLayer ? symbolLayerSymbolFirst : activeKeyLayoutStyle.symbolFirst,
                       numberPadSpace: numberPadSpace, languageLeft: languageLeft)
      NSLayoutConstraint.deactivate(activeStyledActionWidths)
      activeStyledActionWidths = []
      NSLayoutConstraint.deactivate(standardActionWidths + nineKeyActionWidths + phoneActionWidths + layerActionWidths
        + tabletActionWidths + phoneExpandedSpaceWidths + nineKeyExpandedSpaceWidths
        + phoneEnglishActionWidths + phoneEnglishExpandedSpaceWidths
        + handwritingActionWidths + handwritingExpandedSpaceWidths
        + tabletExpandedSpaceWidths + tabletSplitExpandedSpaceWidths
        + numberPadActionWidths + numberPadExpandedZeroWidths
        + numberPadGridWidths + numberPadExpandedGridWidths)
      // iPad 的 26 键底行在回车的位置放第二个 123 和 ⌄，回车在 iPad 上排到第二排字母末尾。
      if tabletLayerButton.isHidden != (style != .tablet) { tabletLayerButton.isHidden = style != .tablet }
      if dismissKeyButton.isHidden != (style != .tablet) { dismissKeyButton.isHidden = style != .tablet }
      // 「右侧大回车」的回车是右列伸下来的那个键（`tallReturnKey`），底行在原来回车的位置留出同宽的空格（`tallReturnSlot`）。
      let rowReturnHidden = style == .tablet || tallReturn
      if enterButton?.isHidden != rowReturnHidden { enterButton?.isHidden = rowReturnHidden }
      if tallReturnSlot.isHidden == tallReturn { tallReturnSlot.isHidden = !tallReturn }
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
      // 符：数字页在顶部有表情入口时的符号键、#+= 层的符号键、假名标点菜单、字母行上的可选键，以及拼音网格和笔画键底行最左边的键，九键上打开符号面板只走它（网格的 @# 键把常用标点放进候选条）。
      // 手机 26 键在 123 之后放 符，空格右边放逗号；14 键第三排自带 符，手写不画字母行，都不加。
      let phoneLetterSymbols = style == .phone && letterRowsShown
      nineKeySymbolsButton.isHidden = !(kana || (symbolLayer ? (showsMoreSymbols || toolbarEmojiAvailable)
        : nineKeyFrame ? !showsSymbols : !showsSymbols && (layout.showsFullKeyboardSymbols || phoneLetterSymbols)))
      let symbolsTitle = symbolLayer ? "符号" : "符"
      if nineKeySymbolsButton.configuration?.title != symbolsTitle {
        nineKeySymbolsButton.configuration?.title = symbolsTitle
        nineKeySymbolsButton.configuration?.titleTextAttributesTransformer = symbolLayer ? Self.functionLabelTransformer : Self.keyLabelTransformer
      }
      fullSymbolsWidth?.isActive = !nineKeySymbolsButton.isHidden && (style == .standard || style == .tablet)
      layerEmojiButton.isHidden = !(symbolLayer && !showsMoreSymbols && !toolbarEmojiAvailable)
      // 26 键「长空格」的 123 / #+= 层：表情或符号入口那一格换成逗号，符号面板从工具栏打开（`updateToolbarSymbolsEntry`）。
      let layerComma = symbolLayer && longSpaceActive
      if layerComma {
        nineKeySymbolsButton.isHidden = true
        layerEmojiButton.isHidden = true
      }
      if layerCommaButton.isHidden == layerComma { layerCommaButton.isHidden = !layerComma }
      if let key = layerCommaButton as? KeyboardKeyButton {
        let face = symbolLayerIsChinese ? "，" : ","
        if layerComma && key.configuration?.title != face {
          key.configuration?.title = face
          key.accessibilityLabel = "符号 \(face)"
        }
        key.cornerHint = layerComma ? (symbolLayerIsChinese ? "。" : ".") : nil
      }
      nineKeyZeroButton.isHidden = style != .nineKey || (!showsSymbols && inputScheme != .nineKeyShuangpin)
      let hidesLanguage = !layout.showsBottomLanguage
      bottomLanguageButton?.isHidden = style == .symbolLayer || hidesLanguage
      bottomLanguageWidth?.isActive = style == .standard && !hidesLanguage
      // iPad 的 26 键从第三排字母输入 ，。，所以它的底行没有自己的 ，。
      // 手机 26 键独立逗号，长按输入句号；14 键和手写保留两个标点键。
      quickPunctuationButton.isHidden = nineKeyFrame || showsSymbols || kana || style == .tablet
      quickPunctuationWidth?.isActive = !quickPunctuationButton.isHidden && style == .standard
      // 英文 26 键在空格左边放独立句点、右边是逗号（中文 26 键仍只有逗号，长按输入句号）。
      let englishPeriod = phoneLetterSymbols && !isChineseMode
      periodKey.isHidden = style != .phone || showsSymbols || (phoneLetterSymbols && !englishPeriod)
      let punctuation = quickPunctuationSymbols
      if (phoneRow || nineKeyPunctuationRowShown) && !usesCustomQuickPunctuation {
        // 手机底行按当前标点模式更新键面。
        quickPunctuationButton.accessibilityValue = nil
      } else {
        quickPunctuationButton.configuration?.title = punctuation[0]
        quickPunctuationButton.accessibilityLabel = "常用标点"
        quickPunctuationButton.accessibilityValue = punctuation[0]
      }
      // 手写底行：「重写」和 ⌫ 排在 中 与回车之间；， 长按输入 ？、。 长按输入 ！，代替原来书写区左侧的两个键。自定义了常用标点时 ， 仍按那份菜单。
      let handwritingRow = style == .phone && writes
      let handwritingQuestion = handwritingRow && !usesCustomQuickPunctuation
      if handwritingRewriteButton.isHidden == handwritingRow { handwritingRewriteButton.isHidden = !handwritingRow }
      if handwritingDeleteButton.isHidden == handwritingRow { handwritingDeleteButton.isHidden = !handwritingRow }
      handwritingCommaHint.isHidden = !handwritingQuestion
      handwritingPeriodHint.isHidden = !handwritingRow
      // 26 键复用角标的长按取消机制，松手只输入句号；其他布局继续长按打开常用标点。「逗号句号在空格两侧」有独立的句号键，逗号与 14 键一样长按打开常用标点。
      let commaCarriesPeriod = phoneLetterSymbols && !styledSlots.contains(.period)
      let commaHoldsPeriod = commaCarriesPeriod && !usesCustomQuickPunctuation
      // 「长空格」的字母层底行没有句号键（26 键、14 键与 18 键），与 Android 的 `commaSwipesPeriod` 一样逗号上滑或下滑输入句号（Android #10087：只认下滑时，照字母键滑动输入符号的默认方向上滑什么也不出），自定义了常用标点也一样。长按仍按上面的规则：26 键默认标点时是句号，14 键、18 键和自定义了常用标点时打开常用标点菜单。
      let longSpaceComma = style == .phone && ((letterRowsShown && longSpaceActive)
        || (fourteenKeyRows && activeKeyLayoutStyle.fourteenKey == .longSpace))
      quickPunctuationButton.menu = handwritingQuestion || commaHoldsPeriod ? nil : UIMenu(children: punctuation.map { symbol in
        UIAction(title: symbol) { [weak self] _ in
          self?.countKeyPress(TypingKeyID.quickPunctuation)
          self?.handleSymbol(symbol)
        }
      })
      if let key = quickPunctuationButton as? KeyboardKeyButton {
        if handwritingQuestion {
          key.cornerHint = Self.chineseSymbolFaces["?"]
          key.onCornerHint = { [weak self] _ in
            self?.countKeyPress(TypingKeyID.quickPunctuation)
            self?.handleSymbol("?")
          }
          key.accessibilityHint = "轻点输入逗号，长按输入问号"
          key.holdsCornerHint = true
        } else {
          let typesPeriod = commaHoldsPeriod || longSpaceComma
          key.cornerHint = typesPeriod ? (periodKey.configuration?.title ?? ".") : nil
          key.onCornerHint = typesPeriod ? { [weak self] _ in
            self?.countKeyPress(TypingKeyID.quickPunctuation)
            self?.handleSymbol(".")
          } : nil
          // 只靠滑动输入句号时，按住留给常用标点菜单。
          key.holdsCornerHint = commaHoldsPeriod
          key.accessibilityHint = longSpaceComma
            ? (commaHoldsPeriod ? "轻点输入逗号，上滑、下滑或长按输入句号" : "轻点输入，上滑或下滑输入句号，长按选择常用标点")
            : commaHoldsPeriod ? "轻点输入逗号，长按输入句号" : "轻点输入，长按选择常用标点"
        }
        key.swipesCornerHint = longSpaceComma
        key.cornerHintSwipesEitherWay = true
      }
      if let key = periodKey as? KeyboardKeyButton {
        key.cornerHint = handwritingRow ? Self.chineseSymbolFaces["!"] : nil
        key.swipesCornerHint = false
        key.onCornerHint = handwritingRow ? { [weak self] _ in
          self?.countKeyPress(TypingKeyID.character("."))
          self?.handleSymbol("!")
        } : nil
        key.accessibilityHint = handwritingRow ? "轻点输入句号，长按输入叹号" : nil
      }
      // 底行的 ⌫ 只给大千 #+= 页的每排十键共享符号行用；设计稿的 #+= 层和九键外框（含数字九宫格）在各自的行里带 ⌫。
      actionDeleteButton.isHidden = !showsSymbols || kana || symbolLayer || nineKeyFrame
      symbolDeleteWidth?.isActive = !actionDeleteButton.isHidden && style == .standard
      // 「按键布局」的非标准样式：样式会增减的几个键按 `TouchBottomRowLayout` 显示或隐藏，宽度也按它约束。
      if let styledRow {
        let optionalKeys: [(TouchBottomRowLayout.Slot, UIButton?)] = [
          (.symbols, nineKeySymbolsButton), (.emoji, layerEmojiButton), (.comma, quickPunctuationButton),
          (.period, periodKey), (.zero, nineKeyZeroButton),
        ]
        for (slot, key) in optionalKeys where key?.isHidden == styledSlots.contains(slot) {
          key?.isHidden = !styledSlots.contains(slot)
        }
        activeStyledActionWidths = tallReturn ? tallReturnActionWidths(styledRow, digits: numberPad) : styledActionWidths(styledRow)
        NSLayoutConstraint.activate(activeStyledActionWidths)
      } else {
        switch style {
        case .phone:
          NSLayoutConstraint.activate(handwritingRow
            ? (hidesLanguage ? handwritingExpandedSpaceWidths : handwritingActionWidths)
            : englishPeriod ? (hidesLanguage ? phoneEnglishExpandedSpaceWidths : phoneEnglishActionWidths)
            : (hidesLanguage ? phoneExpandedSpaceWidths : phoneActionWidths))
        case .nineKey:
          if numberPad && numberPadSpace != .afterZero {
            // 借来的数字九宫格选了另两种空格位置：不带地球键时仍与上方五列对齐，带地球键时按 `TouchBottomRowLayout.borrowedNumberPad` 的份额。
            activeStyledActionWidths = borrowedNumberPadWidths(space: numberPadSpace, hidesLanguage: hidesLanguage)
            NSLayoutConstraint.activate(activeStyledActionWidths)
          } else if numberPad {
            // 系统要求额外地球键时仍保留其触控宽度；通常的手机数字页按网格对齐。
            NSLayoutConstraint.activate(needsInputModeSwitchKey
              ? (hidesLanguage ? numberPadExpandedZeroWidths : numberPadActionWidths)
              : (hidesLanguage ? numberPadExpandedGridWidths : numberPadGridWidths))
          } else {
            NSLayoutConstraint.activate(hidesLanguage ? nineKeyExpandedSpaceWidths : nineKeyActionWidths)
          }
        case .symbolLayer: NSLayoutConstraint.activate(layerActionWidths)
        case .standard: NSLayoutConstraint.activate(standardActionWidths)
        case .tablet: NSLayoutConstraint.activate(hidesLanguage
          ? (split ? tabletSplitExpandedSpaceWidths : tabletExpandedSpaceWidths) : tabletActionWidths)
        }
      }
    }
    symbolRowViews.forEach { $0.isHidden = !showsSymbols || kana || nineKeyFrame || symbolLayer }
    symbolLayerRowViews.forEach { $0.isHidden = !symbolLayer }
    if symbolLayer || symbolExtraRowView?.isHidden == false { updateSymbolLayerFaces() }
    updateSymbolKeyFaces()
    for (row, height) in standardRowHeights { height.isActive = !row.isHidden }
    // 数字九宫格回到字母的键印进入前那块键盘的名字（`numberPadReturnTitle`）：从其他键面借来的数字九宫格与拼音九键一族自己的数字层（「九键」）同一条规则。#+= 页保持原来的字样（`moreSymbolsReturnTitle`），大千 #+= 页的这个键回到数字九宫格、印 123。
    let borrowedNumberPad = numberPad && !nineKey
    if var configuration = layoutToggleButton?.configuration {
      configuration.title = numberPad
        ? Self.numberPadReturnTitle(scheme: inputScheme, chinese: isChineseMode, localMode: isInLocalMode,
                                    letterGrid: typesLetterGrid ? letterGrid : nil)
        : showsSymbols ? Self.moreSymbolsReturnTitle(scheme: inputScheme, chinese: isChineseMode, localMode: isInLocalMode)
        : "123"
      layoutToggleButton?.configuration = configuration
    }
    // 右列第三格的结束标点跟着语言换键面，按下写出的就是键面（`nineKeyClosingFace`）。
    let closingFace = nineKeyClosingFace
    if let closing = nineKeyClosingButton, closing.configuration?.title != closingFace {
      closing.configuration?.title = closingFace
      closing.accessibilityLabel = "符号 \(closingFace)"
    }
    layoutToggleButton?.accessibilityLabel = dachenMoreSymbolsShown ? SymbolLayerLayout.toggleLabel(more: true)
      : symbolLayer || borrowedNumberPad
      ? (strokes ? "切换到笔画" : "切换到字母键盘") : showsSymbols ? "切换到字母" : "切换到数字和符号"
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
    updateToolbarSymbolsEntry()
    updatePreferredKeyboardHeight()
  }

  /// 「按键布局」选了非标准样式、当前底行正是它作用的那一行时，按 `TouchBottomRowLayout` 排出的键；标准样式、借来的数字九宫格和其他底行为 nil，照原来的约束画，默认底行因此与以前完全相同。中/英键放在空格左边时表最后再过一遍 `TouchBottomRowLayout.languageBeforeSpace`；为 nil 的那些行由 `arrangeActionRow` 对按键视图套同一条规则，宽度约束不看次序，仍是原来那一组。26 键样式只作用于手机的中文 26 键字母层（`drawsChineseTwentySixKey`），14 键样式作用于 14 键和 18 键字母层，九键样式作用于拼音九键、九键双拼和笔画的字母层，其中「右侧大回车」（`tallReturn`）连同它们的数字层一起换成自己的外框。拼音九键、九键双拼和笔画自己的数字层（`nineKeyDigits`）不论样式都按 `TouchBottomRowLayout.nineKeyDigits` 排，符 和切换键与字母层同一格（#10061，与 Android #9113 相同）。密码框和只收数字的框按标准样式画（`activeKeyLayoutStyle`）。
  private func styledBottomRow(style: ActionRowStyle, letterRows: Bool, letterGrid: Bool, numberPad: Bool,
                               nineKeyDigits: Bool, tallReturn: Bool, language: Bool) -> [TouchBottomRowLayout.Key]? {
    let row = styleTableRow(style: style, letterRows: letterRows, letterGrid: letterGrid, numberPad: numberPad,
                            nineKeyDigits: nineKeyDigits, tallReturn: tallReturn, language: language)
    guard let row, activeKeyLayoutStyle.languageKeySide == .left else { return row }
    return TouchBottomRowLayout.languageBeforeSpace(row)
  }

  /// 样式本身（连同「符」与「123」互换和数字键盘空格位置）排出的表，还没管中/英在哪一边。
  private func styleTableRow(style: ActionRowStyle, letterRows: Bool, letterGrid: Bool, numberPad: Bool,
                             nineKeyDigits: Bool, tallReturn: Bool, language: Bool) -> [TouchBottomRowLayout.Key]? {
    let globe = needsInputModeSwitchKey
    let keyStyle = activeKeyLayoutStyle
    switch style {
    case .phone where letterRows && drawsChineseTwentySixKey && keyStyle.twentySixKey != .standard:
      let keys = TouchBottomRowLayout.twentySixKey(keyStyle.twentySixKey, globe: globe, language: language)
      return keyStyle.symbolFirst ? TouchBottomRowLayout.symbolFirst(keys) : keys
    case .phone where letterGrid && keyStyle.fourteenKey != .standard:
      return TouchBottomRowLayout.letterGrid(keyStyle.fourteenKey, globe: globe, language: language)
    case .nineKey where tallReturn:
      return TouchBottomRowLayout.nineKeyTallReturn(globe: globe, zero: !numberPad && inputScheme == .nineKeyShuangpin,
                                                    language: language, digits: numberPad)
    case .nineKey where nineKeyDigits:
      return TouchBottomRowLayout.nineKeyDigits(globe: globe, zero: inputScheme == .nineKeyShuangpin, language: language,
                                                space: keyStyle.numberPadSpace)
    case .nineKey where !numberPad && keyStyle.nineKey != .standard:
      return TouchBottomRowLayout.nineKey(keyStyle.nineKey, globe: globe, zero: inputScheme == .nineKeyShuangpin,
                                          language: language)
    default:
      return nil
    }
  }

  /// `TouchBottomRowLayout` 的一格在底行里是哪个键。「右侧大回车」的回车格在底行里是给右列伸下来的回车留出的空格（`tallReturnSlot`）。
  private func actionRowKey(_ slot: TouchBottomRowLayout.Slot, tallReturn: Bool = false) -> UIView? {
    if tallReturn && slot == .returnKey { return tallReturnSlot }
    return switch slot {
    case .symbols: nineKeySymbolsButton
    case .layer: layoutToggleButton
    case .globe: actionGlobeButton
    case .emoji: layerEmojiButton
    case .comma: quickPunctuationButton
    case .space: spaceButton
    case .period: periodKey
    case .zero: nineKeyZeroButton
    case .language: bottomLanguageButton
    case .returnKey: enterButton
    }
  }

  /// 「右侧大回车」底行的宽度约束：符 与左侧符号栏同宽，回车那一格与右列同宽；中间的 123、地球键、中/英（和九键双拼的 0、数字层的空格）与同一地球键和中/英状态下拼音九键标准底行的键绝对等宽。标准底行是 stack view 先从行宽里扣掉它的键距、剩下的按份额分，所以这里同样以整行宽度为基准：宽度 = (行宽 − 标准底行的键距总和) × 份额 / 标准底行总份额（带地球键时是 10.15 份，`nineKeyRowWeight(globe:)`），写成 行宽 × 份额占比 − 键距总和 × 份额占比。不扣键距时这些键比标准底行宽出键距总和乘份额占比，键距 6pt 时 123 宽约 3pt。空格（数字层是 0）不加约束，占九宫格剩下的宽度，于是中间几格正好与九宫格左右对齐。表里只有此刻显示的键（地球键、0、中/英 随宿主和设置进出这一行），所以这些约束都是 required：分离式键盘那个隐藏的第二个空格以 999 跟空格等宽，中间的键若也是 999，两者打平，空格和 中/英 的宽度就没有唯一解。按行和键距缓存。
  private func tallReturnActionWidths(_ row: [TouchBottomRowLayout.Key], digits: Bool) -> [NSLayoutConstraint] {
    let cacheKey = TallReturnWidthsKey(row: row, spacing: actionRow.spacing)
    if let cached = tallReturnActionWidthCache[cacheKey] { return cached }
    guard let sidebar = nineSidebarWidth?.firstItem as? UIView, let controls = nineControls else { return [] }
    let fill: TouchBottomRowLayout.Slot = digits ? .zero : .space
    let globe = row.contains { $0.slot == .globe }
    let standardWeight = TouchBottomRowLayout.nineKeyRowWeight(globe: globe)
    let standardGaps = CGFloat(TouchBottomRowLayout.nineKeyStandardKeyCount(
      globe: globe, language: row.contains { $0.slot == .language }) - 1) * actionRow.spacing
    let constraints = row.compactMap { key -> NSLayoutConstraint? in
      guard key.slot != fill, let view = actionRowKey(key.slot, tallReturn: true) else { return nil }
      let constraint: NSLayoutConstraint
      switch key.slot {
      case .symbols: constraint = view.widthAnchor.constraint(equalTo: sidebar.widthAnchor)
      case .returnKey: constraint = view.widthAnchor.constraint(equalTo: controls.widthAnchor)
      default:
        let share = key.weight / standardWeight
        constraint = view.widthAnchor.constraint(equalTo: actionRow.widthAnchor, multiplier: share, constant: -standardGaps * share)
      }
      return constraint
    }
    tallReturnActionWidthCache[cacheKey] = constraints
    return constraints
  }

  /// 一行非标准样式的宽度约束：每个键相对空格键的份额，与原来的底行一样以空格为基准；数字键盘「空格在右列」的底行没有空格，改以 0 为基准。时有时无的键略低于 required，换样式时先隐藏的键让 stack view 的零宽约束胜出。按行缓存，同一样式来回切换不再新建约束。
  private func styledActionWidths(_ row: [TouchBottomRowLayout.Key]) -> [NSLayoutConstraint] {
    if let cached = styledActionWidthCache[row] { return cached }
    guard let anchorKey = row.first(where: { $0.slot == .space }) ?? row.first(where: { $0.slot == .zero }),
          let anchor = actionRowKey(anchorKey.slot) else { return [] }
    let constraints = row.compactMap { key -> NSLayoutConstraint? in
      guard key.slot != anchorKey.slot, let view = actionRowKey(key.slot) else { return nil }
      let constraint = view.widthAnchor.constraint(equalTo: anchor.widthAnchor, multiplier: key.weight / anchorKey.weight)
      if ![.layer, .language, .returnKey].contains(key.slot) { constraint.priority = .init(999) }
      return constraint
    }
    styledActionWidthCache[row] = constraints
    return constraints
  }

  /// 借来的数字九宫格选了「空格在 0 左边」或「空格在右列」时的宽度约束，按空格位置、中/英显隐和地球键缓存。不带地球键时与上方五列对齐，与原来的数字页同一个规矩：返回与符号栏同宽、回车与右列同宽，0、空格、中/英里各占一列的键与数字键同宽，占两列的那个（`beforeZero` 隐藏中/英时的空格、`rightColumn` 的 0）不加约束、拿剩下的；带地球键时各键按 `TouchBottomRowLayout.borrowedNumberPad` 的份额相对返回键定宽。
  private func borrowedNumberPadWidths(space: TouchNumberPadSpace, hidesLanguage: Bool) -> [NSLayoutConstraint] {
    let key = BorrowedNumberPadWidthsKey(space: space, hidesLanguage: hidesLanguage, globe: needsInputModeSwitchKey)
    if let cached = borrowedNumberPadWidthCache[key] { return cached }
    guard let toggle = layoutToggleButton, let enter = enterButton, let space0 = spaceButton,
          let language = bottomLanguageButton else { return [] }
    let zero: UIView = nineKeyZeroButton
    var constraints: [NSLayoutConstraint] = []
    if !key.globe, let sidebar = nineSidebarWidth?.firstItem as? UIView, let controls = nineControls,
       let digit = nineKeyGridKeys.first?.button {
      constraints = [
        toggle.widthAnchor.constraint(equalTo: sidebar.widthAnchor),
        enter.widthAnchor.constraint(equalTo: controls.widthAnchor),
      ]
      // 各占一列的键与数字键同宽；最后一个不加约束的键拿剩下的宽度。
      var single: [UIView] = []
      switch space {
      case .beforeZero: single = hidesLanguage ? [zero] : [space0, language]
      case .rightColumn: single = hidesLanguage ? [] : [language]
      case .afterZero: single = hidesLanguage ? [space0] : [zero, language]
      }
      constraints += single.map { $0.widthAnchor.constraint(equalTo: digit.widthAnchor) }
    } else {
      let row = TouchBottomRowLayout.borrowedNumberPad(globe: key.globe, language: !hidesLanguage, space: space)
      let layerWeight = TouchBottomRowLayout.layerWeight
      constraints = row.compactMap { item -> NSLayoutConstraint? in
        guard item.slot != .layer, let view = actionRowKey(item.slot) else { return nil }
        let constraint = view.widthAnchor.constraint(equalTo: toggle.widthAnchor, multiplier: item.weight / layerWeight)
        if item.slot == .globe { constraint.priority = .init(999) }
        return constraint
      }
    }
    borrowedNumberPadWidthCache[key] = constraints
    return constraints
  }

  /// 26 键「长空格」把 符 从底行挪到工具栏（`symbolsShortcut`），与 Android 一样：进出这种键面时重排工具栏，显示方式为「隐藏」时也留着这一行，只放品牌、符号和收起三个按钮；同一块键盘切到英文 26 键时这一行还在，只放品牌和收起（`longSpaceToolbarRowKept`）。
  private func updateToolbarSymbolsEntry() {
    let shown = longSpaceActive
    let kept = longSpaceToolbarRowKept
    guard shown != toolbarSymbolsEntryShown || kept != toolbarRowKept else { return }
    toolbarSymbolsEntryShown = shown
    toolbarRowKept = kept
    updateShortcutButtons()
    shortcutBar.isHidden = stripShowsCandidates || inlineHeightBar != nil || toolbarRowHidden
  }

  /// 底行是否为设计稿的手机底行（123 | ， | space | 。 | 中 | return；手写在 中 与 return 之间多出「重写」和 ⌫，空格让窄）：手机键盘显示字母、大千注音各排或大千 #+= 页的共享符号行时是，任一设备形态上的手写板和全拼 14 键也是，与 Android 的 designEntries 给手写和 14 键共用底行一致（iPad 26 键底行的回车在第二排字母上，14 键没有那个位置）。九键外框和假名键保留自己的底行，123 / #+= 层自带一行（`ActionRowStyle.symbolLayer`），iPad 的字母也有自己的（`usesTabletBottomRow`）。
  static func usesPhoneBottomRow(formFactor: KeyboardFormFactor, nineKeyFrame: Bool, handwriting: Bool, kana: Bool,
                                 fourteenKey: Bool = false) -> Bool {
    !nineKeyFrame && !kana && (handwriting || fourteenKey || formFactor == .phone)
  }

  /// 底行是否为 iPad 的 26 键底行（123 | 中 | space | 123 | ⌄，`TabletLetterLayout`）：平板键盘显示字母行时才是，因为这一行的回车在第二排字母上。
  static func usesTabletBottomRow(formFactor: KeyboardFormFactor, letterRows: Bool) -> Bool {
    formFactor == .tablet && letterRows
  }

  /// 按当前键面形态（`keyFaceFormFactor`），在字母行里显示 iPad 键盘的 ⌫、回车和第二个 ⇧，或者手机的 ⌫ 和 44pt 的 ⇧。
  private func applyTabletLetterKeys() {
    let tablet = keyFaceFormFactor == .tablet
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
    // 14 键第三排两端的分词键和 ⌫ 跟着 26 键的 ⇧ 换宽度。
    for width in fourteenKeyEdgeWidths {
      if tablet {
        width.phone.isActive = false
        width.tablet.isActive = true
      } else {
        width.tablet.isActive = false
        width.phone.isActive = true
      }
    }
  }

  /// 屏幕上当前界面画哪种底行。
  private enum ActionRowStyle {
    /// 手机底行：26 键是 123 | 符 | space | ， | 中 | return，逗号长按输入句号；英文 26 键在空格左边多一个句点（123 | 符 | . | space | , | 中 | return）；14 键和手写保留独立逗号句号。中/英 紧挨回车左边；「中/英键放在空格左边」时排到空格和贴着空格的标点左边。
    case phone
    /// 九键外框：符 | 123 | space | 中 | return；拼音九键、九键双拼和笔画的数字层为 符 | 九键 | 0 | 空格 | 中 | return（`TouchBottomRowLayout.nineKeyDigits`），借来的数字九宫格为 返回 | 0 | 空格 | 中 | return、符 在左侧栏。
    case nineKey
    /// 123 / #+= 层：拼音或 ABC | 表情或符号 | space | return。
    case symbolLayer
    /// iPad 26 键底行：123 | 中 | space | 123 | ⌄。
    case tablet
    /// iPad 上大千各排下面的底行，弹性空格键两侧是固定宽度的键。
    case standard
  }

  /// 按布局画出的顺序排列底行的键。按键是重新插入而不是从行里移除，所以它们的宽度约束一直保持安装；每种顺序都列出全部按键，隐藏的放在最后。
  /// `symbolFirst`（`touch_twenty_six_symbol_first`）把手机 26 键底行的 符 排到 123 左边，在 123 / #+= 层（`.symbolLayer`）上把返回键排到 😀 或 符号 后面，调用方只在字母层底行确实互换时传真；`numberPadSpace`（`touch_number_pad_space`）决定借来的数字九宫格里 0 和空格的次序，空格在右列时底行的空格隐藏、排在最后。`languageLeft`（`touch_language_key_side` 为 `left`，调用方只在手机底行和九键外框、中/英显示着时传真）让没有表的那几种排法也照 `TouchBottomRowLayout.languageBeforeSpace` 把中/英排到空格左边：中缝和第二个空格算作空格，隐藏着的逗号、句号、0 也算在这一串里（它们不占宽度，越过它们不改变看得见的次序）；有表的行已经在表里排好。
  private func arrangeActionRow(_ style: ActionRowStyle, commaSecondary: Bool, numberPad: Bool,
                                styled: [TouchBottomRowLayout.Key]? = nil, tallReturn: Bool = false,
                                symbolFirst: Bool = false, numberPadSpace: TouchNumberPadSpace = .platformDefault,
                                languageLeft: Bool = false) {
    guard let toggle = layoutToggleButton, let language = bottomLanguageButton, let gap = actionSplitGap,
          let space = spaceButton, let splitSpace = splitSpaceButton, let enter = enterButton else { return }
    let symbols: UIView = nineKeySymbolsButton, globe: UIView = actionGlobeButton, delete: UIView = actionDeleteButton
    let comma: UIView = quickPunctuationButton, period: UIView = periodKey
    let emoji: UIView = layerEmojiButton, zero: UIView = nineKeyZeroButton
    let tabletLayer: UIView = tabletLayerButton, dismiss: UIView = dismissKeyButton
    // 手写底行的「重写」和 ⌫ 排在 中 与回车之间（只在手写时显示）；其他排法里它们隐藏，放在最后。
    let rewrite: UIView = handwritingRewriteButton, handwritingDelete: UIView = handwritingDeleteButton
    // 「长空格」的 123 层逗号只在符号层底行显示，其他排法里隐藏着排在最后。
    let layerComma: UIView = layerCommaButton
    // 「右侧大回车」在底行回车的位置留的空格，其他排法里隐藏着排在最后。
    let tallSlot: UIView = tallReturnSlot
    var order: [UIView]
    if let styled {
      // 非标准样式：表里的键依次排开，分离用的中缝和第二个空格紧跟空格，其余键隐藏着排在后面。
      order = []
      for key in styled {
        guard let view = actionRowKey(key.slot, tallReturn: tallReturn) else { continue }
        order.append(view)
        if key.slot == .space { order += [gap, splitSpace] }
      }
      let all: [UIView] = [symbols, toggle, globe, delete, emoji, comma, space, gap, splitSpace, period, zero, language,
                           rewrite, handwritingDelete, enter, tabletLayer, dismiss, layerComma, tallSlot]
      order += all.filter { view in !order.contains { $0 === view } }
    } else {
      switch style {
      case .phone:
        order = commaSecondary
          ? (symbolFirst
            ? [symbols, toggle, globe, period, space, gap, splitSpace, comma, language, enter, delete, emoji, zero, tabletLayer, dismiss, rewrite, handwritingDelete]
            : [toggle, globe, symbols, period, space, gap, splitSpace, comma, language, enter, delete, emoji, zero, tabletLayer, dismiss, rewrite, handwritingDelete])
          : [symbols, toggle, globe, delete, comma, space, gap, splitSpace, period, language, rewrite, handwritingDelete, enter, emoji, zero, tabletLayer, dismiss]
      case .nineKey:
        order = !numberPad
          ? [symbols, toggle, globe, space, gap, splitSpace, zero, language, enter, delete, comma, period, emoji, tabletLayer, dismiss, rewrite, handwritingDelete]
          : numberPadSpace == .beforeZero
          ? [symbols, toggle, globe, space, gap, splitSpace, zero, language, enter, delete, comma, period, emoji, tabletLayer, dismiss, rewrite, handwritingDelete]
          : [symbols, toggle, globe, zero, space, gap, splitSpace, language, enter, delete, comma, period, emoji, tabletLayer, dismiss, rewrite, handwritingDelete]
      case .symbolLayer:
        // 互换后 😀 或 符号 排在返回键前面，返回键落在字母层 123 那一格；😀、符号、逗号三者只显示一个。
        order = symbolFirst
          ? [emoji, symbols, layerComma, toggle, globe, space, gap, splitSpace, enter, language, delete, comma, period, zero, tabletLayer, dismiss, rewrite, handwritingDelete]
          : [toggle, emoji, symbols, layerComma, globe, space, gap, splitSpace, enter, language, delete, comma, period, zero, tabletLayer, dismiss, rewrite, handwritingDelete]
      case .tablet:
        order = [symbols, toggle, language, globe, space, gap, splitSpace, tabletLayer, dismiss, delete, comma, period, enter, emoji, zero, rewrite, handwritingDelete]
      case .standard:
        order = [symbols, toggle, globe, delete, comma, space, gap, splitSpace, period, language, enter, emoji, zero, tabletLayer, dismiss, rewrite, handwritingDelete]
      }
      if style != .symbolLayer { order.append(layerComma) }
      order.append(tallSlot)
      if languageLeft {
        order = TouchBottomRowLayout.languageBeforeSpace(order) { view -> TouchBottomRowLayout.Slot? in
          if view === space || view === gap || view === splitSpace { return .space }
          if view === zero { return .zero }
          if view === comma { return .comma }
          if view === period { return .period }
          return view === language ? .language : nil
        }
      }
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
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    // 邮箱后缀不在会话的候选列表里，Tab 不为它们打开候选面板。
    let composing = isChineseMode && (!visiblePreedit.isEmpty || (!visibleCandidates.isEmpty && visibleEmailSuffixes.isEmpty))
    if composing, !visibleCandidates.isEmpty, Self.tabShowsMoreCandidates(session.sharedPreferences) {
      showCandidatePanel()
      return
    }
    playInputClick()
    if composing { render(session.finishComposition()) }
    insertOwnText("\t")
    if !isChineseMode { refreshEnglishSuggestions() }
    finishTemporaryEnglish()
  }

  private func handleBackspace() {
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    if !handwriting.isHidden && handwriting.hasInk { handwriting.canvas.undo(); return }
    playInputClick(.backspace)
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
    if hasComposition && isChineseMode && inputScheme.editsBySyllable && !typesLetterGrid {
      didRepeatBackspace = true
      playInputClick(.backspace)
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
      playInputClick(.backspace)
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
    if gestureRecognizer.name == "keyboardCursorPan", let pan = gestureRecognizer as? UIPanGestureRecognizer {
      let velocity = pan.velocity(in: view)
      return cursorSwipeAllowed && !cursorMovement.isActive
        && velocity.x.isFinite && velocity.y.isFinite && abs(velocity.x) > abs(velocity.y)
    }
    if gestureRecognizer.name == "spaceVoiceHold" {
      return (spaceVoiceArmed || (!spaceVoiceEnabled && (handwriting.isHidden || !handwriting.hasInk))) && !cursorMovement.isActive
    }
    if gestureRecognizer.name == "layoutToggleSymbolsHold" { return !showsSymbols }
    if gestureRecognizer is EighteenKeySwipeGestureRecognizer { return typesEighteenKeyShuangpin && !showsSymbols }
    if gestureRecognizer.name == "nineKeyDigitHold" { return !showsSymbols }
    guard gestureRecognizer.name == "spaceCursorPan", let pan = gestureRecognizer as? UIPanGestureRecognizer else { return true }
    let velocity = pan.velocity(in: view)
    return !cursorMovement.isActive && abs(velocity.x) > abs(velocity.y)
  }

  func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive event: UIEvent) -> Bool {
    guard gestureRecognizer.name == "keyboardCursorPan", let touches = event.allTouches else { return true }
    let active = touches.filter { $0.phase != .ended && $0.phase != .cancelled }
    // 事件回调先于起点过滤，空格等功能键的第二指也必须参与单指裁决。
    if active.count > 1 {
      cursorSwipeHadMultipleTouches = true
      endKeyboardCursorSwipe(cancelGesture: false)
      gestureRecognizer.isEnabled = false
      gestureRecognizer.isEnabled = cursorSwipeEnabled
      return false
    }
    // 只有新的单指序列才能解锁；第二指抬起后，旧首指继续移动仍不能开始横滑。
    if active.count == 1, active.first?.phase == .began { cursorSwipeHadMultipleTouches = false }
    return true
  }

  func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
    guard gestureRecognizer.name == "keyboardCursorPan" else { return true }
    guard cursorSwipeEnabled, let root = keyboardRoot as? KeyAreaStackView,
          let touched = touch.view, touched.isDescendant(of: root),
          !touched.isDescendant(of: japaneseKeys),
          // iPad 单手的侧边数字键盘不挂横滑光标，那里的横滑照常是按键。
          !touched.isDescendant(of: oneHandSidePad),
          !root.gapRoutingExclusions.contains(where: { touched.isDescendant(of: $0) }) else { return false }
    var node: UIView? = touched
    while let current = node, current !== root {
      if let key = current as? KeyboardKeyButton {
        guard !key.isFunctionKey, !spaceKeys.contains(where: { $0 === key }) else { return false }
        // 18 键双拼两个字母的键自己接横滑选字母，不再移动光标。
        if typesEighteenKeyShuangpin && key.gestureRecognizers?.contains(where: { $0 is EighteenKeySwipeGestureRecognizer }) == true {
          return false
        }
        // 九键全拼字母层的 1 打开标点，数字层和双拼的 1 才输入字符。
        return showsSymbols || inputScheme == .nineKeyShuangpin
          || !nineKeyGridKeys.contains(where: { $0.button === key && $0.digit == 1 })
      }
      node = current.superview
    }
    return false
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
    handleSpaceCursorMovement(pan.state, at: pan.translation(in: view).x,
                              velocity: pan.velocity(in: view).x, space: pan.view as? UIButton)
  }

  /// 拖动和长按共用步进、文档身份检查与结束清理，慢速编辑始终按字母移动拼音光标。
  private func handleSpaceCursorMovement(_ state: UIGestureRecognizer.State, at position: CGFloat,
                                         velocity: CGFloat, space: UIButton?) {
    guard let document = KeyboardHostContext.documentIdentifier(for: textDocumentProxy) else {
      cursorMovement.cancel()
      updateSpaceKeyTitle()
      return
    }
    switch state {
    case .began:
      cursorMovement.begin(at: position, document: document)
      // While spelling, the drag edits the spelling: the caret walks through the pinyin as ← / → do in the Windows composition, and typing or backspace then acts there. A Japanese reading keeps ending the composition, because its conversion owns the space key.
      // A Korean syllable, a Zhuyin conversion and a Vietnamese word have no caret inside them either, so the drag finishes them and moves the document caret.
      // 五笔 14 键的组码字母不显示（`typesWubiGrid`），光标在里面走用户看不见，所以同样结束组字、移动文档光标。
      spaceDragEditsComposition = hasComposition && inputScheme.hasSpellingCaret && !typesWubiGrid
      if hasComposition && !spaceDragEditsComposition { render(session.finishComposition()) }
      // 分离式键盘有两个空格，只给正在拖的那个换标题；结束时 updateSpaceKeyTitle 把两个都还原。
      // 拖动时的标签单独显示、不带麦克风，与 Android 的临时空格标签一致。
      if let space, var configuration = space.configuration {
        configuration.title = spaceDragEditsComposition ? "移动拼音光标" : "移动光标"
        configuration.image = nil
        space.configuration = configuration
      }
      if hapticsEnabled { hapticStrength.impact(keyFeedback) }
    case .changed:
      let steps = cursorMovement.advance(to: position, document: document)
      if spaceDragEditsComposition && hasComposition {
        moveCompositionCaret(
          by: steps,
          bySegment: inputScheme.editsBySyllable && !typesLetterGrid && SpaceCursorMovement.movesBySegment(velocity: velocity))
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
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    if !handwriting.isHidden && handwriting.hasInk { _ = handwriting.commitFirst(); return }
    playInputClick(.space)
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
      if commitEnglishBoundary(" ") {
        finishTemporaryEnglish()
        return
      }
      insertDirectText(" ")
      refreshEnglishSuggestions()
      finishTemporaryEnglish()
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
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    defer { finishTemporaryEnglish() }
    if !handwriting.isHidden && handwriting.hasInk { _ = handwriting.commitFirst(); return }
    playInputClick(.enter)
    if inputScheme.isJapanese && hasComposition {
      let index = japaneseConversionIndex
      japaneseConversionIndex = nil
      render(index.flatMap { visibleCandidateEngineIndices.indices.contains($0) ?
        session.selectCandidate(at: UInt(visibleCandidateEngineIndices[$0])) : nil }
        ?? session.commitReading())
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
    scheduleEmailSuffixRefresh()
  }

  private var typingSource: TypingSource {
    if localModeTrigger == "R" || (isChineseMode && inputScheme.isJapanese) { return .japanese }
    if localModeTrigger != nil { return .local }
    if !isChineseMode { return .english }
    if inputScheme == .nineKeyShuangpin { return .nineKey }
    if inputScheme == .cantoneseShuangpin { return .cantonese }
    return TypingSource(rawValue: inputScheme.rawValue) ?? .unknown
  }

  /// 运行时上屏替换行内预编辑；其他直接插入先撤下组字，下一次渲染再标记剩余内容。
  private func insertOwnText(_ text: String, source: TypingSource? = nil, replacingComposition: Bool = false) {
    if replacingComposition && !inlineMarkedText.isEmpty {
      // 直接提交替换标记区；先重标记再 unmark 的跨进程调用会让随后的剩余组字覆盖已选汉字。
      inlineMarkedText = ""
      textDocumentProxy.insertText(text)
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

  /// 在线候选没有开启时跳过 `online_query` 的 C ABI 往返；设置在键盘重新出现或共享文档重载后同步。
  private func synchronizeOnlineCandidatePreference() {
    let enabled = CloudCandidatePreference.enabled || AICandidatePreference.isEnabled(session.sharedPreferences)
    onlineCandidatesEnabled = enabled
    if !enabled { onlineCandidates.cancel() }
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

  // Swallowing this left the settings screen showing zeros with nothing to explain them, which is how it reached a bug report rather than the person typing. The reason does not change between keystrokes and a banner on each one would bury the composition, so it is said once per session.
  // The shared store holds commits in this process and rewrites and fsyncs the file only once 30 seconds have passed since its last write (or on `flushTypingStatistics`), but that write, and the first read of the file, still happen inside a call, so it runs off main on a serial queue.
  private func recordTypingStatistics(_ text: String, source: TypingSource) {
    guard hasFullAccess, privacyGate.allows(.typing) else { return }
    typingStatisticsUnflushed = true
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
    let timer = Timer(timeInterval: 30, target: self, selector: #selector(flushStatistics), userInfo: nil, repeats: true)
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

  /// 按键批次和攒着的上屏一起写出：30 秒定时器、键盘收起和宿主退到后台（扩展可能不经 viewWillDisappear 就被结束）都走这里。
  @objc private func flushStatistics() {
    flushKeyPresses()
    flushTypingStatistics()
  }

  /// 共享层的 `record` 把上屏攒在本进程里，距上次写文件满 30 秒的下一次上屏才写，停下不打字时最后一批会一直留在内存里，进程被杀就丢了，设置页也看不到。所以键盘收起、换输入框、宿主退到后台和每 30 秒的定时器在这里要它写出。排在统计队列上，跟在之前交出去的 record 后面；没有攒着的上屏时共享层不碰磁盘。flush 失败只记进诊断日志，不打扰正在打字的人。
  private func flushTypingStatistics() {
    guard typingStatisticsUnflushed else { return }
    typingStatisticsUnflushed = false
    statisticsQueue.async {
      do { try TypingStatisticsStore().flush() } catch { DiagnosticLog.shared.write("statistics_flush_failed") }
    }
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

  func render(_ snapshot: MetasequoiaInputSnapshot, source originalSource: TypingSource? = nil,
              appendingLiteral: String? = nil) {
    // 控制命令已产生更新快照时，之前异步按键的候选帧不能再覆盖选词、删除或模式切换后的状态。
    deferredInputSnapshot = nil
    candidateRevision &+= 1
    // 下面的 `updateCandidateStrip` 用这次快照的候选盖掉 1 键放进来的常用标点。
    nineKeyQuickSymbols = []
    let source = originalSource ?? typingSource
    // 看这次快照是否还在本地模式，不能看 `isInLocalMode`：它此刻还是上一个快照的值，刚从菜单打开模式时是 none，会把刚设好的触发字母清掉，候选栏标题就只剩字母、打字统计也不再记作本地模式。
    if localModeTrigger != nil && !snapshot.isInLocalMode {
      localModeTrigger = nil
      showsSymbols = false
    }
    currentLocalMode = snapshot.localMode
    // 样式在偏好改动后空闲重建时才变；变了就重画受影响的键面。
    if currentCornerQuotes != snapshot.cornerQuotes || currentHollowBrackets != snapshot.hollowBrackets
      || currentEnumerationCommaBackslash != snapshot.enumerationCommaBackslash
      || currentEnumerationCommaSlash != snapshot.enumerationCommaSlash {
      currentCornerQuotes = snapshot.cornerQuotes
      currentHollowBrackets = snapshot.hollowBrackets
      currentEnumerationCommaBackslash = snapshot.enumerationCommaBackslash
      currentEnumerationCommaSlash = snapshot.enumerationCommaSlash
      updateSymbolKeyFaces()
      if !symbolLayerKeys.isEmpty { updateSymbolLayerFaces() }
    }
    currentNineKeySpellings = snapshot.nineKeySpellings
    currentNineKeySingleCharacter = snapshot.nineKeySingleCharacter
    currentNineKeyStrokes = snapshot.nineKeyStrokes
    updateLetterCaseControls()
    if snapshot.commitText != nil || appendingLiteral != nil {
      let commit = snapshot.commitText.map { source == .japanese || !snapshot.scriptConversion ? $0 : chineseOutput($0) } ?? ""
      // 原组字和数字作为同一次文档替换提交；先撤标记再插数字会读到尚未更新的选区，数字可能跑到拼音前面。
      insertOwnText(commit + (appendingLiteral ?? ""), source: source,
                    replacingComposition: snapshot.commitText != nil)
    }
    let wasComposing = renderedComposition
    let wasKoreanHanjaListOpen = koreanHanjaListOpen
    hasComposition = !snapshot.preedit.isEmpty
    if hasComposition != wasComposing { updateFourteenKeySeparator() }
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
    // 已选的那一段领在读音前面，与来源把 word_for_creating_word 拼在读音前面是同一件事。拼写以「拼音分词」作为标记文本写进编辑框；候选条只在全拼 14 键的读音行、已选的那一段和本地模式名里画它（`updatePreeditButton`）。
    // 全拼九键的 `preedit` 是数字、14 键的是组码字母，读音行显示 Engine 给的拼音读音（`ning'bai`；首选是英文词时是这个词的字母）；没有读音时才退回 `preedit`。
    let shownPreedit = !snapshot.nineKeyReading.isEmpty ? snapshot.nineKeyReading
      : inputScheme.isJapanese && !snapshot.reading.isEmpty ? snapshot.reading : snapshot.preedit
    let composing = snapshot.phrasePrefix + shownPreedit
    visiblePhrasePrefix = snapshot.phrasePrefix
    // The `editing_text` of an in-place scheme is the keys of what it composes, not a spelling with a caret to move.
    // 五笔 14 键的 `editing_text` 是各组的代表字母（QW QW 是 `qq`），不是用户打的码，读音行和拼写菜单都不显示它；它只用来给其余候选比对编码提示（`wubiCodeHint`）。
    let wubiGrid = typesWubiGrid
    visibleWubiGridKeys = wubiGrid ? snapshot.editingText : ""
    let spellingCaret = inputScheme.hasSpellingCaret && !wubiGrid
    visibleCaretSpelling = spellingCaret ? snapshot.editingTextWithCaret : nil
    editableSpelling = hasComposition && spellingCaret && !snapshot.isInLocalMode
      && !snapshot.editingText.isEmpty && snapshot.editingText.allSatisfy(\.isASCII)
      ? (snapshot.editingText, snapshot.caretPosition) : nil
    let japaneseReading = inputScheme.isJapanese && !snapshot.reading.isEmpty ? snapshot.reading : nil
    // 全拼 14 键的组字是组码字母（`buguo`），写进输入框对用户没有意义，与 Android 一样不在输入框里标记，只显示在读音行上。英文模式和本地输入模式画 26 键，组字照常标记。九键的 `preedit` 是数字串（`64426`），写进输入框的是和读音行同一个 `shownPreedit`，即 Engine 给的拼音读音（`ni'hao`）。
    showInlineComposition(hasComposition && !typesLetterGrid
      ? InlineCompositionPolicy.markedText(
        inPlace: isChineseMode && inputScheme.composesInPlace, style: .pinyin,
        drawsKeysAsGlyphs: isChineseMode && inputScheme.drawsKeysAsGlyphs, phrasePrefix: snapshot.phrasePrefix, preedit: shownPreedit,
        editingText: snapshot.editingText, japaneseReading: japaneseReading)
      : "")
    visibleInputGeneration = snapshot.generation
    let visibleIndices = CandidateMarker.visibleIndices(sources: snapshot.candidateSources)
    visibleCandidateIdentities = visibleIndices.map {
      snapshot.candidateIdentities.indices.contains($0) ? snapshot.candidateIdentities[$0] : nil
    }
    func visibleValues<T>(_ values: [T]) -> [T] {
      visibleIndices.compactMap { values.indices.contains($0) ? values[$0] : nil }
    }
    updateCandidateStrip(
                         preedit: composing,
                         candidates: visibleValues(snapshot.candidates),
                         candidateCodes: visibleValues(snapshot.candidateCodes),
                         candidateGlosses: visibleValues(snapshot.candidateGlosses),
                         candidateAnnotations: visibleValues(snapshot.candidateAnnotations),
                         candidateSources: visibleValues(snapshot.candidateSources),
                         candidateFixedPositions: visibleValues(snapshot.candidateFixedPositions),
                         candidateHighlighted: visibleValues(snapshot.candidateHighlighted),
                         candidateConfidentCorrections: visibleValues(snapshot.candidateConfidentCorrections),
                         candidatePageCount: snapshot.candidatePageCount,
                         candidateEngineIndices: visibleIndices,
                         answeredByPinyinFallback: snapshot.answeredByPinyinFallback)
    // The Hanja list opens and closes while the syllable keeps composing, and Return chooses a Hanja only while it is open.
    if !inputScheme.isJapanese && koreanHanjaListOpen != wasKoreanHanjaListOpen { updateReturnKey() }
    refreshCandidatePanelAnnotations()
    updateSpellingStrip()
    scheduleCandidateGlosses()
    if onlineCandidatesEnabled {
      onlineCandidates.refresh(allowed: hasFullAccess && hasComposition && !isInLocalMode)
    }
    if !hasComposition {
      releaseDeferredEngineCache()
      if wasComposing { synchronizeShuangpinLayoutPreference() }
    }
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

  private func chooseVisibleNineKeySpelling(at index: UInt) {
    if let generation = visibleInputGeneration {
      render(session.chooseNineKeySpelling(generation: generation, at: index))
    } else {
      render(session.chooseNineKeySpelling(at: index))
    }
  }

  private func updateCandidateStrip(preedit: String, candidates: [String],
                                    candidateCodes: [String] = [], candidateGlosses: [String] = [],
                                    candidateAnnotations: [String] = [], candidateSources: [Int] = [],
                                    candidateFixedPositions: [Int] = [], candidateHighlighted: [Bool] = [],
                                    candidateConfidentCorrections: [Bool] = [], candidatePageCount: Int = 0,
                                    candidateEngineIndices: [Int]? = nil,
                                    answeredByPinyinFallback: Bool = false) {
    // 光标前打到 `xxx@` 时邮箱后缀占候选条，排在英文建议之前，与 Android 一样；它们不是 Engine 的候选，不带编码、释义和来源。
    // 还在输入线程上算的英文建议不能再盖掉这次的内容。
    englishSuggestionStripGeneration &+= 1
    englishSuggestionPrefix = ""
    englishSuggestionDocument = nil
    let emailSuffixes = emailSuffixesForStrip(preedit: preedit)
    visibleEmailSuffixes = emailSuffixes
    let showsEmailSuffixes = !emailSuffixes.isEmpty
    let candidates = showsEmailSuffixes ? emailSuffixes : candidates
    if visibleCandidates != candidates {
      candidateGlossRequestedGeneration = nil
    }
    visiblePreedit = preedit
    visibleCandidates = candidates
    visibleCandidateCodes = showsEmailSuffixes ? [] : candidateCodes
    visibleCandidateGlosses = showsEmailSuffixes ? [] : candidateGlosses
    visibleCandidateAnnotations = showsEmailSuffixes ? [] : candidateAnnotations
    visibleCandidateSources = showsEmailSuffixes ? [] : candidateSources
    visibleCandidateFixedPositions = showsEmailSuffixes ? [] : candidateFixedPositions
    visibleCandidateHighlighted = showsEmailSuffixes ? [] : candidateHighlighted
    visibleCandidateConfidentCorrections = showsEmailSuffixes ? [] : candidateConfidentCorrections
    if showsEmailSuffixes || candidates.isEmpty {
      visibleCandidateEngineIndices = []
    } else if let candidateEngineIndices, candidateEngineIndices.count == candidates.count {
      visibleCandidateEngineIndices = candidateEngineIndices
    } else if candidateSources.isEmpty && visibleCandidateEngineIndices.count == candidates.count {
      // 光标变化只重算邮箱后缀，保留当前 Engine 候选的索引映射。
    } else {
      visibleCandidateEngineIndices = Array(candidates.indices)
    }
    visibleCandidatePageCount = showsEmailSuffixes ? 0 : candidatePageCount
    visibleCandidatesAnsweredByPinyinFallback = showsEmailSuffixes ? false : answeredByPinyinFallback
    requestCandidateTranslations()
    // Any new candidate list is a different composition or a different set of matches, so the page
    // it was showing no longer describes anything.
    // A horizontal offset belongs to the previous matches, just like the page index.
    // Cancel deceleration as well so it cannot hide the new leading candidate.
    if candidateScrollView.contentOffset != .zero {
      candidateScrollView.setContentOffset(.zero, animated: false)
    }
    renderCandidateStrip()
  }

  /// 展开网格仍使用普通当前页；独立来源标注不包含固定位置图钉或原文后缀。
  private func refreshDecorationWords() {
    let page = visibleEmailSuffixes.isEmpty ? Array(visibleCandidates.prefix(candidatePageSize)) : visibleEmailSuffixes
    let hasOverlay = candidateCanvasSkin?.banner?.candidate != nil
    let words: [ExternalCandidateSkin.Canvas.Word] = hasOverlay && visibleDiagnostic == nil ? page.enumerated().map { index, text in
      let source = visibleCandidateSources.indices.contains(index) ? visibleCandidateSources[index] : -1
      let badge = source == CandidateMarker.cloudSource ? "☁️" : source == CandidateMarker.aiSource ? "🤖" : ""
      let highlighted = japaneseConversionIndex.map { $0 == index }
        ?? (visibleCandidateHighlighted.indices.contains(index) && visibleCandidateHighlighted[index])
      return ExternalCandidateSkin.Canvas.Word(text: chineseOutput(text, source: source), badge: badge, highlighted: highlighted)
    } : []
    let hosts: [CandidateCanvasHostView?] = [compositionContainer, candidatePanel]
    for host in hosts {
      host?.decorationWords = words
      host?.decorationFontFamilies = candidateFontFamilies
    }
  }

  private func renderCandidateStrip() {
    exitLocalModeButton.isHidden = !isInLocalMode
    updateHanjaButton()
    let showsCandidates = isInLocalMode || !visiblePreedit.isEmpty || !visibleCandidates.isEmpty || visibleDiagnostic != nil
    stripShowsCandidates = showsCandidates
    compositionContainer?.backgroundColor = currentStripBackground(KeyboardTheme.current)
    // 内联高度调节条占据工具栏的位置；下面的按键照常可用，所以打字时候选仍会盖在两者之上显示。
    shortcutBar.isHidden = showsCandidates || inlineHeightBar != nil || toolbarRowHidden
    // 工具栏内容可以隐藏，但 iOS 键盘扩展不能在组字开始和结束时改变顶栏高度；否则 UIKit 会重排键盘，表现为键盘抖动，严重时还会重新选择输入法。
    inlineHeightBar?.isHidden = showsCandidates
    compositionContainer?.canvasVisible = showsCandidates && candidateCanvasActive
    readingRow.isHidden = !showsCandidates
    candidateContent?.isHidden = !showsCandidates
    updatePreeditButton()
    // 邮箱后缀不分页：二十个全排在候选条上横向滚动，与 Android 一样。
    refreshDecorationWords()
    let page = visibleEmailSuffixes.isEmpty ? Array(visibleCandidates.prefix(candidatePageSize)) : visibleEmailSuffixes
    while candidateStack.arrangedSubviews.count < page.count {
      let index = candidateStack.arrangedSubviews.count
      candidateStack.addArrangedSubview(makeCandidateButton(index: index))
      candidateButtonInputs.append(nil)
    }
    while candidateButtonInputs.count < candidateStack.arrangedSubviews.count { candidateButtonInputs.append(nil) }
    var missingPronunciations: [String] = []
    defer { if !missingPronunciations.isEmpty { requestCandidatePronunciations(missingPronunciations) } }
    for (offset, view) in candidateStack.arrangedSubviews.enumerated() {
      guard let chip = view as? KeyboardKeyButton else { continue }
      chip.isHidden = offset >= page.count
      guard offset < page.count else {
        candidateButtonInputs[offset] = nil
        continue
      }
      if !visibleEmailSuffixes.isEmpty {
        // 样式与英文建议的词块相同，只是读作邮箱后缀。
        let inputs = CandidateButtonInputs(
          candidate: page[offset], hint: "", hintSpoken: nil, reading: nil, glosses: [], pronunciation: nil, markers: [],
          number: offset + 1, converting: false, aiPlaceholder: false, source: -1, confidentCorrection: false,
          styleRevision: candidateButtonStyleRevision)
        if candidateButtonInputs[offset] != inputs {
          updateCandidateButton(chip, candidate: page[offset], hint: "", glosses: [], number: offset + 1, converting: false, source: -1)
          chip.accessibilityLabel = "邮箱后缀 \(offset + 1)：\(page[offset])"
          candidateButtonInputs[offset] = inputs
        }
        continue
      }
      // A Korean Hanja's 훈음 goes on the first line under it rather than after it, so it is not read as part of the candidate.
      let annotation = candidateAnnotation(at: offset)
      let hint = typesKorean ? "" : annotation.text
      let hintSpoken = readsChaizi ? annotation.accessibilityDescription : nil
      let reading = typesKorean ? annotation.text : nil
      let glosses = candidateGlosses(at: offset)
      let pronunciation = candidatePronunciationLine(at: offset, missing: &missingPronunciations)
      let markers = candidateMarkers(at: offset)
      let aiPlaceholder = visibleCandidateSources.indices.contains(offset)
        && (visibleCandidateSources[offset] == CandidateMarker.aiPendingSource
          || visibleCandidateSources[offset] == CandidateMarker.cloudPendingSource)
      let candidateSource = visibleCandidateSources.indices.contains(offset) ? visibleCandidateSources[offset] : -1
      let confidentCorrection = visibleCandidateConfidentCorrections.indices.contains(offset)
        && visibleCandidateConfidentCorrections[offset]
      let inputs = CandidateButtonInputs(
        candidate: page[offset], hint: hint, hintSpoken: hintSpoken, reading: reading, glosses: glosses,
        pronunciation: pronunciation,
        markers: markers, number: offset + 1, converting: japaneseConversionIndex == offset,
        aiPlaceholder: aiPlaceholder, source: candidateSource, confidentCorrection: confidentCorrection,
        styleRevision: candidateButtonStyleRevision)
      guard candidateButtonInputs[offset] != inputs else { continue }
      updateCandidateButton(
        chip, candidate: page[offset], hint: hint, hintSpoken: hintSpoken, reading: reading,
        glosses: glosses, pronunciation: pronunciation, markers: markers, number: offset + 1,
        converting: japaneseConversionIndex == offset,
        aiPlaceholder: aiPlaceholder, source: candidateSource, confidentCorrection: confidentCorrection)
      candidateButtonInputs[offset] = inputs
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
    // 拆字模式里 Engine 的注释是这个字的带调读音（焱 → yàn），找到不会读的字后正要看它，所以不论方案都画，九键进入本地模式后也一样；读音不带括号，原样显示。
    if readsChaizi { return engine.isEmpty ? ("", "") : (engine, "读音 \(engine)") }
    // 全拼、双拼以及五笔候选都可能在这里携带展示注释：前两者是辅助码或纠错提示，五笔是词条反查编码。
    guard !engine.isEmpty, !isInLocalMode,
          inputScheme == .quanpin || usesShuangpin || inputScheme == .wubi else { return ("", "") }
    // The Engine brackets its suffix for desktop windows that append it after the word; here it sits under the word like the Wubi hint, which is bare.
    let bare = engine.hasPrefix("(") && engine.hasSuffix(")") && engine.count > 2
      ? String(engine.dropFirst().dropLast()) : engine
    return (bare, "编码提示 \(bare)")
  }

  /// 候选条第 `index` 个候选下面的音标行：不显示音标时为 nil；显示时音标还没到或这个词查不到就是空串，那一行照样留着，词块高度不变。
  private func candidatePronunciationLine(at index: Int, missing: inout [String]) -> String? {
    guard pronunciationLineShown else { return nil }
    guard visibleCandidates.indices.contains(index) else { return "" }
    let word = visibleCandidates[index]
    // 英文建议这类不是会话视图的候选没有来源，也没有释义，只按词本身查。
    let hasSource = visibleCandidateSources.indices.contains(index)
    let source = hasSource ? visibleCandidateSources[index] : -1
    guard !CandidateTranslationStore.sourceGetsNoGloss(source) else { return "" }
    let reading = hasSource ? readingGloss(
      word: word, offline: visibleCandidateGlosses.indices.contains(index) ? visibleCandidateGlosses[index] : "",
      source: source, translated: drawsCandidateTranslation(at: index)) : nil
    return cachedPronunciation(pronunciationLookupText(word: word, readingGloss: reading), missing: &missing)
  }

  /// The gloss and translation lines under a strip candidate. A Korean Hanja's 훈음 is not one of them: it is drawn above them by the strip and never offered for insertion.
  private func candidateGlosses(at index: Int) -> [String] {
    guard CandidateGlossPreference.enabled, visibleCandidates.indices.contains(index),
          CandidateTranslationStore.allowsGloss(sources: visibleCandidateSources, at: index) else { return [] }
    let word = visibleCandidates[index]
    var languages = [CandidateTranslationPreference.primary]
    if let secondary = CandidateTranslationPreference.secondary { languages.append(secondary) }
    let translated = drawsCandidateTranslation(at: index)
    // 「仅当前高亮」时其他候选没有候选词翻译，那一行照旧留着（占位或离线释义），候选条高度和词块排版不变。
    return languages
      .filter {
        Self.canFillGloss($0, fullAccess: hasFullAccess, onlineRoute: translationRoute != .none,
                          offline: offlineGlossLanguages)
      }
      .map {
        gloss(word: word, language: $0,
              offline: visibleCandidateGlosses.indices.contains(index) ? visibleCandidateGlosses[index] : "",
              translated: translated)
          ?? Self.pendingGlossPlaceholder
      }
  }

  /// 候选条第 `index` 个候选画不画宿主缓存里的候选词翻译：「仅当前高亮」生效时只有视图里高亮的那一个画。英文离线释义来自视图的 `translation`，范围已由会话按同一规则给好。
  private func drawsCandidateTranslation(at index: Int) -> Bool {
    CandidateTranslationPreference.drawsTranslation(
      at: index, highlighted: visibleCandidateHighlighted, scoped: CandidateTranslationPreference.scopedToHighlight)
  }

  static let pendingGlossPlaceholder = " "

  /// 展开面板和它的长按菜单里一个候选的释义行。`highlighted` 是这个候选是不是会话当前高亮的那一个：「仅当前高亮」生效时只有它带候选词翻译。
  private func glosses(word: String, offline: String, source: Int, highlighted: Bool) -> [String] {
    guard !CandidateTranslationStore.sourceGetsNoGloss(source) else { return [] }
    let translated = !CandidateTranslationPreference.scopedToHighlight || highlighted
    var lines: [String] = []
    if let value = gloss(word: word, language: CandidateTranslationPreference.primary, offline: offline,
                         translated: translated) { lines.append(value) }
    if let secondary = CandidateTranslationPreference.secondary,
       let value = gloss(word: word, language: secondary, offline: "", translated: translated) { lines.append(value) }
    return lines
  }

  private func gloss(word: String, language: CandidateTranslationLanguage, at index: Int) -> String? {
    guard CandidateTranslationStore.allowsGloss(sources: visibleCandidateSources, at: index) else { return nil }
    return gloss(word: word, language: language,
                 offline: visibleCandidateGlosses.indices.contains(index) ? visibleCandidateGlosses[index] : "",
                 translated: drawsCandidateTranslation(at: index))
  }

  /// `translated` 为假时不取宿主缓存里的候选词翻译（在线、账号、AI 翻译），只画离线释义：「仅当前高亮」时不是高亮的候选就是这样。
  private func gloss(word: String, language: CandidateTranslationLanguage, offline: String, translated: Bool) -> String? {
    if !CandidateTranslationPreference.needsNetwork(language), glossesCandidates,
       !offline.isEmpty, offline != word { return offline }
    let target = !glossesCandidates ? nil : candidateTargetGlosses[language.code]?[word]
    let online = translated && CandidateTranslationPreference.onlineEnabled
      ? translations.gloss(word: word, code: language.code) : nil
    return Self.preferredGloss(offline: target, online: online, route: translationRoute)
  }

  /// A non-English gloss, as on macOS: a service of the user's own outranks the offline dictionary, which outranks the 水杉 account.
  static func preferredGloss(offline: String?, online: String?, route: TranslationRoute) -> String? {
    switch route {
    case .account, .none: return offline ?? online
    case .niutrans, .tencent, .custom, .ai: return online ?? offline
    }
  }

  private func candidatePanelAnnotation(code: String, gloss: String, word: String, engine: String,
                                        source: Int, highlighted: Bool, typed: String,
                                        missingPronunciations: inout [String]) -> KeyboardCandidateAnnotation {
    let (hint, hintDescription) = codeHint(code: code, engine: engine, typed: typed)
    let lines = glosses(word: word, offline: gloss, source: source, highlighted: highlighted)
    // 展开网格的格子按内容定高，音标到了才多一行，不预留。
    var pronunciation = ""
    if pronunciationLineShown, !CandidateTranslationStore.sourceGetsNoGloss(source) {
      let translated = !CandidateTranslationPreference.scopedToHighlight || highlighted
      let reading = readingGloss(word: word, offline: gloss, source: source, translated: translated)
      pronunciation = cachedPronunciation(pronunciationLookupText(word: word, readingGloss: reading),
                                          missing: &missingPronunciations)
    }
    let text = ([hint] + lines + [pronunciation]).filter { !$0.isEmpty }.joined(separator: "\n")
    guard !text.isEmpty else { return .none }
    let description = ([hintDescription, lines.isEmpty ? "" : "英文释义：\(lines.joined(separator: "，"))",
                        pronunciation.isEmpty ? "" : CandidatePronunciationModel.accessibilityLabel(pronunciation)])
      .filter { !$0.isEmpty }.joined(separator: "，")
    return KeyboardCandidateAnnotation(text: text, accessibilityDescription: description)
  }

  private func synchronizeTranslationRoute() -> Bool {
    var route = TranslationProviderPreference.route(in: session.sharedPreferences)
    if case let .ai(provider, endpoint, model, _) = route,
       let token = KeyboardAIService.translationToken(endpoint: endpoint) {
      route = .ai(provider: provider, endpoint: endpoint, model: model, token: token)
    }
    translationRouteGeneration &+= 1
    let generation = translationRouteGeneration
    if route == .account {
      // Forget the previous login before the asynchronous keychain read. The keyboard may
      // render between this call and its completion, so no old account gloss may survive here.
      translationAccountScope = nil
      let changed = applyTranslationRoute(route, scope: "account|pending=\(generation)")
      synchronizeAccountTranslationIdentity(generation: generation)
      return changed
    }

    translationAccountIdentityTask?.cancel()
    translationAccountIdentityTask = nil
    translationAccountScope = nil
    return applyTranslationRoute(route, scope: route.cacheScope)
  }

  private func applyTranslationRoute(_ route: TranslationRoute, scope: String) -> Bool {
    guard route != translationRoute || scope != translations.scope else { return false }
    translationRoute = route
    translations.use(route == .account ? BackendCandidateTranslationService() : ProviderCandidateTranslationService(route: route),
                     scope: scope)
    DiagnosticLog.shared.write("translation_route provider=\(route.provider?.rawValue ?? "none")")
    // Rows reserved for network-only languages follow whether any service is chosen.
    applyCandidateGlossLayout()
    return true
  }

  private func synchronizeAccountTranslationIdentity(generation: Int) {
    translationAccountIdentityTask?.cancel()
    translationAccountIdentityTask = Task { [weak self] in
      let identity = try? await BackendAccountSession.shared.login()
      guard !Task.isCancelled, let self,
            self.translationRouteGeneration == generation,
            self.translationRoute == .account else { return }
      let scope = identity.map { TranslationRoute.accountCacheScope(sessionID: $0.sessionID) }
        ?? "account|anonymous"
      self.translationAccountScope = scope
      guard self.applyTranslationRoute(.account, scope: scope) else { return }
      // A scope change clears the old cache. Refresh the visible strip immediately so an old
      // account gloss cannot remain painted while the new request is in flight.
      self.renderCandidateStrip()
      self.requestCandidateTranslations()
    }
  }

  private func requestCandidateTranslations() {
    guard CandidateGlossPreference.enabled, CandidateTranslationPreference.onlineEnabled,
          translationRoute != .none, hasFullAccess, glossesCandidates, !isInLocalMode,
          translationRoute != .account || translationAccountScope != nil,
          !visibleCandidates.isEmpty, visibleEmailSuffixes.isEmpty else { translations.cancel(); return }
    var codes = [CandidateTranslationPreference.primary.code]
    if let secondary = CandidateTranslationPreference.secondary { codes.append(secondary.code) }
    // 「仅当前高亮」：在线、账号、AI 翻译只为视图里高亮的那一个候选请求；高亮换了候选就取消旧请求，缓存留着。离线释义（英文和已装的其他语言）不经这里，照旧整页查。
    let scoped = CandidateTranslationPreference.scopedToHighlight
    let page = CandidateTranslationStore.requestPage(
      words: Array(visibleCandidates.prefix(candidatePageSize)), sources: visibleCandidateSources,
      highlighted: visibleCandidateHighlighted, scoped: scoped)
    translations.refresh(words: page.words, sources: page.sources, codes: codes, dropsStale: scoped)
  }

  private func wubiCodeHint(code: String, typed: String) -> String {
    guard inputScheme == .wubi, !isInLocalMode,
          WubiCodeHintPreference.isEnabled(in: session.sharedPreferences) else { return "" }
    // 五笔 14 键的读音只是首选编码的前几位；其余候选的编码前几位逐位落在已按的键上时，按它自己打过的那几位提示剩下的编码，与 26 键一样每个候选都有提示。
    let typed = typesWubiGrid ? FourteenKeyLayout.typedPrefix(of: code, keys: visibleWubiGridKeys) ?? typed : typed
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
      // AI 回答到达时也会换一代，但最多只有 AI 行变了（第一条插在第一页最后一格，多出来的接在末尾；回答的词都已在列表里时一行都没变）：面板留着、原地换上新的一代，不跳回开头。原来这里一换代就收起普通的候选网格，用户刚展开完整列表，AI 的结果一到它就没了（#6888）。
      if hasComposition, snapshot.onlyTheAISeatChanged(since: current) {
        candidatePanelSnapshot = snapshot
        panel.reload(candidates: candidatePanelTexts(snapshot), annotations: candidatePanelAnnotations(snapshot),
                     markers: candidatePanelMarkers(snapshot), aiPlaceholders: candidatePanelAIPlaceholders(snapshot),
                     confidentCorrections: candidatePanelConfidentCorrections(snapshot), keepingScrollPosition: true)
        updateNineKeyPanelColumns()
        return
      }
      // 九键面板里选拼音、退格和筛选都会换一代候选，面板留着、按新的一代重建，组字结束才收起。面板里刚选了候选时普通网格也一样（`selectPanelCandidate`）；其他换代照旧收起普通网格。
      guard nineKeyPanelColumns != nil || candidatePanelFollowsSelection, hasComposition else {
        closeKeyboardPicker()
        return
      }
      candidatePanelSnapshot = snapshot
      panel.reload(candidates: candidatePanelTexts(snapshot), annotations: candidatePanelAnnotations(snapshot),
                   markers: candidatePanelMarkers(snapshot), aiPlaceholders: candidatePanelAIPlaceholders(snapshot),
                   confidentCorrections: candidatePanelConfidentCorrections(snapshot))
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
          !isInLocalMode, !visibleCandidates.isEmpty, visibleEmailSuffixes.isEmpty,
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
    // The answer is applied asynchronously and guarded by generation, so the glosses for the compositions a fast typist passes through are fetched and then thrown away. Nobody ever saw them.
    candidateGlossTimer?.invalidate()
    candidateGlossTimer = nil
    // The render that applies an answer lands here again with the same generation; the strip's page was already asked for, so there is nothing to wait for.
    if candidatePanel == nil, let visible = visibleInputGeneration, candidateGlossRequestedGeneration == visible { return }
    let timer = Timer(timeInterval: 0.12, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated {
        self?.candidateGlossTimer = nil
        self?.requestCandidateGlosses()
      }
    }
    RunLoop.main.add(timer, forMode: .common)
    candidateGlossTimer = timer
  }

  /// `allCandidates` is the whole list the candidate grid just read, so opening the grid does not read it a second time.
  private func requestCandidateGlosses(allCandidates prefetched: [String: Any]? = nil) {
    guard CandidateGlossPreference.enabled, glossesCandidates,
          !isInLocalMode, !visibleCandidates.isEmpty,
          let resources = session.candidateGlossResources(), !resources.isEmpty else { return }
    do {
      let generation: UInt64
      let candidates: [[String: Any]]
      let fullList: Bool
      // While the grid is closed only the strip's page is on screen, so only that page is asked for, straight from the rendered view. Reading the whole list costs in proportion to it: `yi` answers with hundreds, and that one call measured 3.5ms on the main thread - more than the whole rest of a keystroke - before the lookup, two more encodes of every row and the full view `applyTranslations` returns. The grid asks for the whole list when it opens (`showCandidatePanel`) and on every pause while it is open. A strip that is not the session's view (no sources, e.g. English suggestions or nine-key symbols) still reads the whole list as before.
      if prefetched == nil, candidatePanel == nil, let visible = visibleInputGeneration,
         visibleCandidateSources.count == visibleCandidates.count, visibleCandidateSources.allSatisfy({ $0 >= 0 }) {
        if candidateGlossRequestedGeneration == visible { return }
        generation = visible
        candidates = zip(visibleCandidates, visibleCandidateSources).map { text, source -> [String: Any] in
          ["text": text, "source": NSNumber(value: source)]
        }
        fullList = false
      } else {
        let allCandidates = try prefetched ?? session.allCandidates()
        guard let value = allCandidates["generation"] as? NSNumber else { return }
        guard let listGeneration = CandidateGlossModel.integerValue(value, maximum: UInt64.max) else { return }
        if candidateGlossRequestedGeneration == listGeneration && candidateGlossRequestedFullList { return }
        guard let rows = allCandidates["candidates"] as? [[String: Any]] else { return }
        generation = listGeneration
        candidates = rows
        fullList = true
      }
      let request = try CandidateGlossModel.request(generation: generation, candidates: candidates)
      candidateGlossRequestedGeneration = generation
      candidateGlossRequestedFullList = fullList
      let targetEpoch = candidateGlossEpoch
      let targetResources = resources
      let queue = candidateGlossQueue
      let applyOnMain: (UInt64, Data) -> Void = { [weak self] responseGeneration, translations in
        guard translations != Data("[]".utf8) else { return }
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
              let entries = try? JSONSerialization.jsonObject(with: decoded.translations) as? [[String: Any]]
        else { return }
        // 每条还带着 `gloss: true`（交给会话时用），这里只取文字和释义。
        var glosses: [String: String] = [:]
        for entry in entries {
          if let text = entry["text"] as? String, let translation = entry["translation"] as? String {
            glosses[text] = translation
          }
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
    // 词块按住时立即变淡，松手立即恢复。
    button.animatesPressFeedback = false
    button.pressFeedback = .opacity(0.6)
    button.setContentCompressionResistancePriority(.required, for: .horizontal)
    let minimumWidth = button.widthAnchor.constraint(greaterThanOrEqualToConstant: Self.candidateChipMinimumSize.width)
    // 隐藏的栈内词块由 UIStackView 强制设为零宽，词块自己的宽度下限必须让步。
    minimumWidth.priority = .init(999)
    // 高度下限随「候选栏大小」缩放（#10137）：它和候选内容贴住面板底边的约束同为 999，下限不缩放时，矮于 34pt 的候选栏里两者冲突，Auto Layout 可能撑高候选内容、压到按键上。宽度是触控下限，不缩放。
    let minimumHeight = button.heightAnchor.constraint(
      greaterThanOrEqualToConstant: Self.candidateChipMinimumSize.height * candidateBarScale)
    minimumHeight.priority = .init(999)
    candidateChipHeightConstraints.append(minimumHeight)
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
    if visibleEmailSuffixes.indices.contains(index) {
      useEmailSuffix(at: index)
      return
    }
    if nineKeyQuickSymbols.indices.contains(index) {
      let symbol = nineKeyQuickSymbols[index]
      nineKeyQuickSymbols = []
      // 没在组字时 Engine 没有候选，写标点的路线也不一定经过 `render`，所以先把候选条清空；组字中写标点会上屏首选并重画候选条。
      if !hasComposition { updateCandidateStrip(preedit: "", candidates: []) }
      handleSymbol(symbol)
      return
    }
    if inputScheme == .handwriting, !handwritingResults.isEmpty {
      if handwriting.use(at: index) { handwritingResults = [] }
      return
    }
    if !isChineseMode {
      useEnglishSuggestion(at: index)
      return
    }
    guard visibleCandidateEngineIndices.indices.contains(index) else { return }
    if visibleCandidateIdentities.indices.contains(index), let identity = visibleCandidateIdentities[index] {
      render(session.selectCandidate(generation: identity.generation, globalIndex: identity.index))
    } else {
      render(session.selectCandidate(at: UInt(visibleCandidateEngineIndices[index])))
    }
  }

  private func candidateColumnWidth() -> CGFloat {
    KeyboardKeyButton.glossColumnWidth(
      visible: candidateScrollView.bounds.width, spacing: candidateStack.spacing,
      insets: Self.candidateChipInsets)
  }

  /// 复用构造释义行时已经算好的内容宽度，避免再次转换富文本并测量首行。
  ///
  /// 皮肤的 `item_min_width` 不用在这里：它是桌面候选窗的格宽（`base = "microsoft"` 出厂 3.5em），套到手机和 iPad 的候选条上，候选字号 24 时每个词块至少 84pt，「的」「㎥」这样一个字宽的候选也撑成一大格，一屏只剩三个多候选。词块只按内容取宽，下限是 `candidateChipMinimumSize` 的触控宽度，与 Android、鸿蒙的触屏候选条一致。
  private func pinCandidateWidth(of button: KeyboardKeyButton, contentWidth: CGFloat?) {
    let existing = button.constraints.first { $0.identifier == "candidateChipWidth" }
    guard let contentWidth else {
      if existing?.isActive == true { existing?.isActive = false }
      return
    }
    let insets = button.configuration?.contentInsets ?? .zero
    let width = ceil(contentWidth + insets.leading + insets.trailing)
    if let existing {
      guard !existing.isActive || abs(existing.constant - width) > 0.5 else { return }
      existing.constant = width
      existing.isActive = true
    } else {
      let constraint = button.widthAnchor.constraint(equalToConstant: width)
      constraint.identifier = "candidateChipWidth"
      constraint.priority = .init(999)
      constraint.isActive = true
    }
  }

  private func updateCandidateButton(
    _ button: KeyboardKeyButton, candidate: String, hint: String, hintSpoken: String? = nil, reading: String? = nil, glosses: [String],
    pronunciation: String? = nil,
    markers: [CandidateMarker] = [], number: Int, converting: Bool, aiPlaceholder: Bool = false, source: Int = -1,
    confidentCorrection: Bool = false
  ) {
    let display = chineseOutput(candidate, source: source)
    guard var configuration = button.configuration else { return }
    let skin = KeyboardTheme.current
    let annotationColor = (number == 1 || converting ? candidatePalette?.selectedTranslation : nil) ?? candidatePalette?.secondary ?? skin.secondary
    if let palette = candidatePalette {
      // canvas 首选用选中色，其余透明；既有桌面配色保留「首选高亮」取色器的 hover 语义。
      configuration.background.customView = number == 1 && palette.showSelectedBar
        ? CandidateSelectionBarView(color: palette.selectedBar ?? palette.accent, layout: palette.layout,
                                    fontSize: Self.candidateChipFontSize * candidateFontScale) : nil
      configuration.background.backgroundColor = converting ? palette.selected.withAlphaComponent(0.35)
        : number == 1 ? (candidateCanvasSkin?.panelLoaded != true ? palette.hover : palette.selected)
        : candidateCanvasSkin?.panelLoaded != true ? palette.surface : .clear
      configuration.background.cornerRadius = candidatePalette?.layout?.itemRadius ?? candidateCanvasSkin?.itemCornerRadius ?? 9
      configuration.background.strokeWidth = palette.layout?.borderWidth ?? 1
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
    // 英文音标在释义下面单独一行（与 macOS、Android 相同）；还没到或查不到时留一个占位，行数不变。
    let lines = (reading.map { [$0.isEmpty ? Self.pendingGlossPlaceholder : $0] } ?? []) + glosses
      + (pronunciation.map { [$0.isEmpty ? Self.pendingGlossPlaceholder : $0] } ?? [])
    // On the keyboard's own colours the first candidate, the one space commits, is the selected one: the accent at weight 600 while the rest stay in the key text colour at 400 (dc.html `mobCands`). The desktop palette marks it with its fill instead.
    let selected = candidatePalette == nil && number == 1
    let weight: UIFont.Weight = selected ? .semibold : .regular
    // A pinned word takes the accent colour, as it does in the Windows candidate window; chips are reused, so every other word is set back.
    let pinned = markers.contains(where: \.pinned)
    // 坐了首选的双拼纠错（`confident_correction`）同样用强调色画字；键盘自己的配色里首选本来就是强调色，所以它的纠错注释（纠错前的拼写）和释义行也改用强调色，不画次要色，和没坐首选的纠错区分开。桌面调色板下首个词块画在选中底色上，调色板给了那里的颜色就照用（字用选中文字色，注释用选中翻译色、其次选中文字色），都没给才用强调色：微信外观的选中底色就是强调色，强调色的字会看不见。展开网格是同一条规则（`KeyboardCandidatePanelView.makeChip`）。
    let accent = candidatePalette?.accent ?? skin.accent
    let onSelectedFill = candidatePalette != nil && number == 1
    let captionColor = confidentCorrection
      ? (onSelectedFill ? candidatePalette?.selectedTranslation ?? candidatePalette?.selectedText : nil) ?? accent : annotationColor
    configuration.baseForegroundColor = pinned || selected
      ? accent
      : confidentCorrection ? (onSelectedFill ? candidatePalette?.selectedText : nil) ?? accent
      : (number == 1 ? candidatePalette?.selectedText : nil) ?? candidatePalette?.text ?? skin.keyForeground
    // 在线占位行画成次要色，和真候选区分开（#6888）。
    if aiPlaceholder { configuration.baseForegroundColor = annotationColor }
    // The 훈음 is drawn whole, like an inline hint was, so the chip widens to it rather than cutting it to the gloss column; the glosses under it fit that width.
    var contentWidth: CGFloat?
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
          .foregroundColor: captionColor,
        ]))
      }
      if !lines.isEmpty {
        let caption = UIFont.systemFont(ofSize: Self.candidateGlossFontSize)
        let readingWidth = reading.map { ceil(($0 as NSString).size(withAttributes: [.font: caption]).width) } ?? 0
        let glossWidth = lines.map { ceil(($0 as NSString).size(withAttributes: [.font: caption]).width) }.max() ?? 0
        let content = KeyboardKeyButton.chipContentWidth(
          titleLine: max(NSAttributedString(title).size().width, readingWidth), glossLine: glossWidth,
          glossLines: lines.count, column: candidateColumnWidth())
        contentWidth = content
        for gloss in lines {
          let fitted = KeyboardKeyButton.fittedGloss(gloss, font: caption, width: content)
          title += AttributedString("\n" + fitted.text, attributes: AttributeContainer([
            .font: fitted.font, .paragraphStyle: paragraph,
            .foregroundColor: captionColor,
          ]))
        }
      }
      configuration.attributedTitle = title
    }
    configuration.contentInsets = candidatePalette?.layout?.chipInsets(fontSize: candidateFont.pointSize, fallback: Self.candidateChipInsets) ?? Self.candidateChipInsets
    button.configuration = configuration
    button.titleLineCount = 1 + lines.count
    pinCandidateWidth(of: button, contentWidth: contentWidth)
    // `hintSpoken` 是注释另有读法时的朗读文字，比如拆字的读音不是还要输入的键。
    button.accessibilityLabel = annotation.isEmpty
      ? "候选词 \(number)：\(display)"
      : "候选词 \(number)：\(display)，\(hintSpoken ?? "还需输入 \(annotation)")"
    // A Korean Hanja's 훈음 is its meaning and reading, not keys still to type.
    if let reading, !reading.isEmpty { button.accessibilityLabel? += "，训音 \(reading)" }
    for marker in markers { button.accessibilityLabel? += "，\(marker.spoken)" }
    let spoken = glosses.filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
    if !spoken.isEmpty {
      button.accessibilityLabel? += "，释义 " + spoken.joined(separator: "，")
    }
    if let pronunciation, !pronunciation.isEmpty { button.accessibilityLabel? += "，\(CandidatePronunciationModel.accessibilityLabel(pronunciation))" }
    if confidentCorrection { button.accessibilityLabel? += KeyboardCandidateAnnotation.confidentCorrectionSpoken }
    // A Hanja's long press offers nothing (candidateMenuElements(at:)), so its glosses are read but not offered for insertion.
    button.accessibilityHint = spoken.isEmpty || reading != nil ? nil : "轻点输入，长按可输入释义"
    if aiPlaceholder {
      button.accessibilityLabel = CandidateMarker.pendingSpoken(candidate, markers: markers)
      button.accessibilityHint = nil
    }
  }

  /// The markers as symbol attachments after the word, at the annotation's size and colour so they read as a suffix rather than as part of the candidate.
  static func markerRun(_ markers: [CandidateMarker], color: UIColor, scale: CGFloat) -> AttributedString {
    let font = CandidateFontPreference.font(.caption1, scale: scale)
    var run = AttributedString()
    for marker in markers where !marker.symbol.isEmpty {
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
          guard let self else { return }
          session.flushPendingCharacters()
          guard isCurrent() else { return }
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
    // 邮箱后缀不是词条，长按没有固定、删除或以词定字可做。
    guard isChineseMode, inputScheme.writesChinese, !isInLocalMode, visibleEmailSuffixes.isEmpty,
          visibleCandidates.indices.contains(index) else { return [] }
    let candidate = visibleCandidates[index]
    let revision = candidateRevision
    let glosses = glossMenuElements(at: index)
    let fixedPosition = visibleCandidateFixedPositions.indices.contains(index) ? visibleCandidateFixedPositions[index] : 0
    let identity = visibleCandidateIdentities.indices.contains(index) ? visibleCandidateIdentities[index] : nil
    let management = candidateMenuElements(candidate: candidate, fixedPosition: fixedPosition) { [weak self] operation in
      guard let self, candidateRevision == revision,
            visibleCandidates.indices.contains(index), visibleCandidates[index] == candidate else { return nil }
      guard visibleCandidateEngineIndices.indices.contains(index) else { return nil }
      if let identity {
        return session.editCandidate(generation: identity.generation, globalIndex: identity.index, action: operation)
      }
      return session.editCandidate(at: UInt(visibleCandidateEngineIndices[index]), expectedWord: candidate, action: operation)
    }
    let source = visibleCandidateSources.indices.contains(index) ? visibleCandidateSources[index] : -1
    let edges = wordCharacterMenuElements(candidate: candidate, source: source) { [weak self] last in
      guard let self, candidateRevision == revision,
            visibleCandidates.indices.contains(index), visibleCandidates[index] == candidate else { return nil }
      guard visibleCandidateEngineIndices.indices.contains(index) else { return nil }
      if let identity {
        return session.selectCandidateEdge(generation: identity.generation, globalIndex: identity.index, last: last)
      }
      return session.selectCandidateEdge(at: UInt(visibleCandidateEngineIndices[index]), last: last)
    }
    return edges + glosses + management + personalWordbookElements(candidate: candidate, glosses: candidateGlosses(at: index))
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
    generation: UInt64, globalIndex: UInt64, candidate: String, offlineGloss: String, source: Int, highlighted: Bool,
    fixedPosition: Int
  ) -> [UIMenuElement] {
    guard isChineseMode, inputScheme.writesChinese, !isInLocalMode else { return [] }
    let glosses = glossMenuElements(glosses(word: candidate, offline: offlineGloss, source: source,
                                            highlighted: highlighted)) { [weak self] in
      guard let self else { return false }
      guard let snapshot = try? session.allCandidates(),
            let current = try? CandidatePanelSnapshot.decode(snapshot),
            current.generation == generation,
            current.entries.contains(where: { $0.index == globalIndex && $0.text == candidate }) else {
        return false
      }
      return true
    }
    let personal = personalWordbookElements(
      candidate: candidate,
      glosses: self.glosses(word: candidate, offline: offlineGloss, source: source, highlighted: highlighted))
    guard session.isOnCurrentPage(generation: generation, globalIndex: globalIndex) else { return glosses + personal }
    let management = candidateMenuElements(candidate: candidate, fixedPosition: fixedPosition) { [weak self] operation in
      self?.session.editCandidate(generation: generation, globalIndex: globalIndex, action: operation)
    }
    let edges = wordCharacterMenuElements(candidate: candidate, source: source) { [weak self] last in
      guard let self else { return nil }
      closeKeyboardPicker()
      return session.selectCandidateEdge(generation: generation, globalIndex: globalIndex, last: last)
    }
    return edges + glosses + management + personal
  }

  /// 长按菜单最后一组的「加入生词本」，和 Android、鸿蒙的长按菜单、桌面的候选右键菜单同一个位置：把候选和它显示着的释义（几种语言各一行）收进背单词的生词本。没有释义时没有这一项。不经过 Engine，组字不变；写文件放在后台，提示回到主线程显示在诊断行并播报。
  private func personalWordbookElements(candidate: String, glosses: [String]) -> [UIMenuElement] {
    guard let gloss = PersonalWordbookStore.menuGloss(glosses) else { return [] }
    let action = UIAction(title: PersonalWordbookStore.menuTitle, image: UIImage(systemName: "text.book.closed")) { [weak self] _ in
      guard let self else { return }
      playInputClick()
      // 没有完全访问时键盘写不进 App Group，背单词也就读不到。
      guard hasFullAccess else {
        showPersonalWordbookNotice("开启「允许完全访问」后才能加入生词本")
        return
      }
      DispatchQueue.global(qos: .userInitiated).async {
        let notice = PersonalWordbookStore.add(candidate: candidate, gloss: gloss)
        DispatchQueue.main.async { [weak self] in self?.showPersonalWordbookNotice(notice) }
      }
    }
    return [UIMenu(title: "", options: .displayInline, children: [action])]
  }

  private func showPersonalWordbookNotice(_ notice: String) {
    showDiagnostic(notice)
    renderCandidateStrip()
    UIAccessibility.post(notification: .announcement, argument: notice)
  }

  /// 以词定字: commit just the first or the last Han character of a longer candidate. Desktop hosts bind it to [ ] or - =; a keyboard extension receives no hardware keys, even on an iPad with one attached, so here it lives in the candidate's long-press menu and follows only the on/off half of the shared `word_character` preference.
  private func wordCharacterMenuElements(
    candidate: String, source: Int, select: @escaping (_ last: Bool) -> MetasequoiaInputSnapshot?
  ) -> [UIMenuElement] {
    let stored = session.sharedPreferences?["word_character"] as? [String: Any] ?? [:]
    let han = candidate.filter { $0.unicodeScalars.first?.properties.isUnifiedIdeograph == true }
    guard stored["enabled"] as? Bool ?? true, han.count > 1, let first = han.first, let last = han.last else { return [] }
    func action(_ title: String, _ character: Character, last: Bool) -> UIAction {
      UIAction(title: "\(title)「\(chineseOutput(String(character), source: source))」", image: UIImage(systemName: last ? "text.insert" : "text.append")) { [weak self] _ in
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
        if operation == .remove, result.isHandled { recordDictionaryChange() }
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
    makeKey(title: title, accessibilityLabel: accessibilityLabel, emphasized: emphasized, function: function,
            primaryAction: UIAction { _ in action() })
  }

  private func makeKey(
    title: String,
    accessibilityLabel: String,
    emphasized: Bool = false,
    function: Bool = false,
    primaryAction: UIAction
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
    let button = KeyboardKeyButton(configuration: configuration, primaryAction: primaryAction)
    button.isFunctionKey = function
    button.accessibilityLabel = accessibilityLabel
    decorateKey(button)
    return button
  }

  /// 26 键的字母键和微软双拼的 `;`：与 `makeKey` 相同，但按键记下每次触摸的按下点（`recordsTouchDown`），动作拿到触发它的按键（`UIAction.sender`），好取走那个按下点。`sendActions` 这类没有触摸的激活也传按键，只是它没有按下点。
  private func makeTouchKey(
    title: String, accessibilityLabel: String, action: @escaping (KeyboardKeyButton?) -> Void
  ) -> UIButton {
    let button = makeKey(title: title, accessibilityLabel: accessibilityLabel,
                         primaryAction: UIAction { action($0.sender as? KeyboardKeyButton) })
    (button as? KeyboardKeyButton)?.recordsTouchDown = true
    return button
  }

  private func decorateKey(_ button: UIButton) {
    button.addTarget(self, action: #selector(prepareKeyFeedback), for: .touchDown)
    if button.window == nil { keySkinNeedsWindowPass = true }
    let skin = KeyboardTheme.current
    guard var configuration = button.configuration else { return }
    if let design = skin.design {
      // 已经有按键表面就接着用，只在设计或底色变了时重画；以前每次换装、每次切换 ⇧ 都给按键新建一个表面，丢掉旧的位图重新画。
      let surface = configuration.background.customView as? SkinKeySurfaceView ?? SkinKeySurfaceView()
      if surface.design != design { surface.design = design }
      let fill = configuration.background.backgroundColor ?? skin.keyBackground
      if surface.fillColor != fill { surface.fillColor = fill }
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
    // iPad 单手的块宽和侧边数字键盘的宽度按键盘宽度算：第一次有了宽度、旋转和台前调度改窗口大小时在这里重排（排法没变时什么也不做）。
    applyOneHanded()
    updateLetterRowInsets()
    if let globe = actionGlobeButton, globe.isHidden != !needsInputModeSwitchKey {
      updateKeyboardLayout()
    }
    updateSplitKeyboardIfNeeded()
    // 转到横屏时数字行收起，转回竖屏时回来。
    if actionRow != nil, keyLayoutStyle.numberRow, appliedLayoutInputs?.numberRowFits != numberRowFits {
      updateKeyboardLayoutIfNeeded()
    }
    updatePreferredKeyboardHeight()
  }

  private var showsPhoneLetterRows: Bool {
    formFactor == .phone && letterRowViews.first?.isHidden == false
  }

  /// 手机竖屏 26 键在存储行距之外增加 4pt 留白；横屏空间更紧，沿用存储行距。
  private var keyRowSpacing: CGFloat {
    let letterPadding: CGFloat = showsPhoneLetterRows && !isLandscape
      ? KeyboardFormFactor.phoneLetterRowPadding : 0
    return CGFloat(KeyboardLayoutPreference.rowSpacing) + letterPadding
  }

  /// 26 键收紧键帽后，把省下的高度放到键区上下，保持九键和英文键盘的外框一致。
  private func applyPhoneLetterVerticalPadding() {
    let inset = phoneLetterVerticalInset
    let margins = UIEdgeInsets(top: inset, left: 0, bottom: inset, right: 0)
    if keyColumn.layoutMargins != margins || keyColumn.isLayoutMarginsRelativeArrangement != (inset > 0) {
      keyColumn.isLayoutMarginsRelativeArrangement = inset > 0
      keyColumn.layoutMargins = margins
    }
  }

  /// 用户调整之前的键盘高度：由内边距、当前尺寸的顶栏、顶栏下的间距和按存储行距排列的各排按键累加而成，所以顶栏变高（释义行、更大的候选字号）或行距变宽会加高键盘，而不是从按键里扣。
  private var preferredKeyboardHeight: CGFloat {
    // 手写共用顶栏，因此也共用竖屏的通用高度。
    formFactor.keyboardHeight(
      topRow: shownTopRowHeight, rowSpacing: keyRowSpacing,
      landscape: isLandscape, handwriting: !handwriting.isHidden, numberRow: showsKeyboardNumberRow,
      letterKeys: showsPhoneLetterRows, phoneNumberRow: showsPhoneNumberRow)
  }

  /// 手机上画着「按键布局」的顶部数字行，或者数字和符号页上替它占位的补充行：键盘多一排高。
  private var showsPhoneNumberRow: Bool {
    formFactor == .phone && (numberRowView?.isHidden == false || symbolExtraRowView?.isHidden == false)
  }

  /// 是否显示第五排按键：iPad 数字行、数字符号页补充行或大千各排。
  private var showsKeyboardNumberRow: Bool {
    numberRowView?.isHidden == false || symbolExtraRowView?.isHidden == false
      || zhuyinRowViews.first?.isHidden == false
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
    root.setCustomSpacing(formFactor.topRowGap, after: topRow)
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
  /// `keepsKeyTouches` 为真时键区只是看不见，按着的手指（按住空格说话）照常收到松开和移动；新的触摸落在面板上。
  private func installPanel(_ panel: UIView, keepsKeyTouches: Bool = false) {
    guard let root = keyboardRoot, let topRow = compositionContainer else { return }
    // 面板替换回复面板的方式与替换按键相同。
    if replyKeyboardShown { closeReplyKeyboard() }
    keyAreaPanel = panel
    panel.accessibilityViewIsModal = false
    panel.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(panel)
    NSLayoutConstraint.activate([
      panel.topAnchor.constraint(equalTo: topRow.bottomAnchor),
      panel.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -phoneLetterVerticalInset),
      panel.leadingAnchor.constraint(equalTo: root.leadingAnchor),
      panel.trailingAnchor.constraint(equalTo: root.trailingAnchor),
    ])
    setKeyRowsCovered(true, keepsTouches: keepsKeyTouches)
    updateToolbarActiveState()
    UIAccessibility.post(notification: .layoutChanged, argument: panel)
  }

  /// 26 键收紧键帽后，键区面板也要停在实际按键的底边，不能盖住补偿留白。留白按此刻的排数算（`KeyboardFormFactor.phoneLetterVerticalPadding(rows:)`），顶部数字行打开时多一排。
  ///
  /// 九键一族（拼音九键、九键双拼、笔画，`TouchKeyLayoutOffering.nineKeySchemes`）切到英文或本地模式时不留白：字母行和底行一样由按键列均分，默认行距下键帽从 44pt 长到 49pt，排距仍是紧凑的 11pt，字母区上下边界与九宫格重合，切换时按键区不缩一截。其他方案的 26 键保持 44pt 键帽和上下留白。
  private var phoneLetterVerticalInset: CGFloat {
    guard showsPhoneLetterRows && !isLandscape,
          !TouchKeyLayoutOffering.nineKeySchemes.contains(inputScheme) else { return 0 }
    return KeyboardFormFactor.phoneLetterVerticalPadding(
      rows: formFactor.keyRows(numberRow: false, phoneNumberRow: showsPhoneNumberRow))
  }

  /// 隐藏或恢复顶栏下面的所有行，以及它们旁边的单手模式侧栏和 iPad 单手的侧边数字键盘。这些行保留布局，键盘高度不变；它们只是不再绘制、不再接收触摸。
  private func setKeyRowsCovered(_ covered: Bool, keepsTouches: Bool = false) {
    // 「右侧大回车」的回车和数字层右列的空格叠在按键列上、不在任何一行里，跟着这些行一起盖住；侧边数字键盘和侧栏一样在按键列旁边，也一起盖住。
    for row in keyColumn.arrangedSubviews + [oneHandGutter, oneHandSidePad] + [tallReturnKey, numberPadColumnSpace].compactMap({ $0 }) {
      row.alpha = covered ? 0 : 1
      row.isUserInteractionEnabled = !covered || keepsTouches
      row.accessibilityElementsHidden = covered
    }
  }

  // The drags report on every gesture frame; the shared document is written once the value has
  // settled, so a single adjustment does not take a file lock a hundred times.
  private func commitTouchKeyboardGeometry() {
    let saved = session.persistTouchKeyboardGeometry(
      keySpacing: KeyboardLayoutPreference.keySpacing,
      rowSpacing: KeyboardLayoutPreference.rowSpacing,
      heightAdjustment: KeyboardLayoutPreference.heightAdjustment)
    guard !saved else { return }
    // 共享文档没写成（多半是和设置 App 同时写、比较交换输了）。原先不声不响：App Group 和这个键盘用着新值，文档里还是旧值，下次键盘出现时被文档盖回去，看起来就是「调的高度自己变回去了」。现在记一笔日志，并按文档把 App Group 和键盘对齐，用户当场看到的就是实际留下的值。
    DiagnosticLog.shared.write("touch_geometry_not_persisted")
    guard let document = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    KeyboardLayoutPreference.mirrorGeometry(document)
    sharedKeyboardHeightAdjustment = CGFloat(KeyboardLayoutPreference.heightAdjustment)
    applyLayoutPreferences()
    // 文档里的键距可能与键盘刚才用的不同。applyLayoutPreferences 只改各行的 spacing，九键「右侧大回车」中列的宽度约束在常数里扣的是建约束时的键距，要重排底行才会换成新键距的那一组；几何是 layoutInputs 的一部分，键距变了这里就会重排，没变什么也不做。
    updateKeyboardLayoutIfNeeded()
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
    let picker = KeyboardMorePickerView(tools: makeTools(), formFactor: formFactor, landscape: isLandscape)
    pickerLandscape = isLandscape
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
      rows: formFactor.keyRows(numberRow: showsKeyboardNumberRow, phoneNumberRow: showsPhoneNumberRow),
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
    closeCandidatePanel()
    render(session.currentSnapshot())
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
      onABC: { [weak self] in self?.closeKeyboardPicker() },
      onSticker: { [weak self] sticker in self?.copySticker(sticker) },
      pluginKaomoji: pluginsDirectory.map { KeyboardEmojiCatalog.pluginSymbolGroups(resources: resources, plugins: $0) } ?? [])
    applyEmojiPickerAppearance(picker)
    emojiPicker = picker
    installPanel(picker)
  }

  /// 插件目录：会话的状态目录（App Group 里与设置 App 共用的那一个）下的 `plugins`，符号集插件从这里读。会话没准备好时为 nil。
  private var pluginsDirectory: String? {
    session.stateDirectory.map { ($0 as NSString).appendingPathComponent("plugins") }
  }

  /// 键盘扩展没有把图片插进输入框的接口，表情包转成 PNG 放进系统剪贴板，由用户长按输入框粘贴。没有完全访问时写剪贴板会被系统静默丢弃，所以先提示去打开。
  private func copySticker(_ sticker: KeyboardStickerCatalog.Sticker) {
    playInputClick()
    guard hasFullAccess else {
      showDiagnostic(KeyboardStickerCatalog.needsFullAccessMessage)
      renderCandidateStrip()
      return
    }
    DispatchQueue.global(qos: .userInitiated).async {
      let png = KeyboardStickerCatalog.pngData(sticker)
      DispatchQueue.main.async { [weak self] in
        guard let self else { return }
        if let png {
          UIPasteboard.general.setData(png, forPasteboardType: KeyboardStickerCatalog.pasteboardType)
          showDiagnostic(KeyboardStickerCatalog.copiedMessage(sticker))
        } else {
          showDiagnostic(KeyboardStickerCatalog.copyFailedMessage)
        }
        renderCandidateStrip()
      }
    }
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
    let plugins = pluginsDirectory
    let catalog = session.candidateGlossResources().flatMap {
      $0.isEmpty ? nil : KeyboardSymbolPanelView.Catalog.engine(resources: $0, plugins: plugins)
    }
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
    pickerLandscape = isLandscape
    let picker = KeyboardSchemePickerView(
      selected: inputScheme, isChineseMode: isChineseMode, showsHeader: false, formFactor: formFactor, landscape: isLandscape,
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
        else if temporaryEnglishFromScheme {
          temporaryEnglishFromScheme = false
          if !inputContext.isInLatinField { ImeModeMemoryPreference.record(chinese: false) }
          updateLetterCaseControls()
        }
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
      selected: KeyboardTheme.current.id, document: session.sharedPreferences, dark: drawsDark, showsHeader: false,
      formFactor: formFactor,
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
    KeyboardTheme.reload(session.sharedPreferences, dark: drawsDark)
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
      // 数字键盘「空格在右列」时底行空格藏着，焦点回到右列那个空格。
      let shownSpace = numberPadColumnSpace.flatMap { $0.isHidden ? nil : $0 } ?? spaceButton
      UIAccessibility.post(notification: .layoutChanged, argument: shownSpace)
    }
    // 听写还没结束时面板被关掉（点面板、上滑取消、键盘收起、换出别的面板）就取消它。
    if let dictation = voiceDictation {
      voiceDictation = nil
      voiceDictationContext = nil
      voiceHoldOriginY = nil
      dictation.cancel()
    }
    if keyAreaPanel != nil {
      keyAreaPanel = nil
      setKeyRowsCovered(false)
    }
    updateToolbarActiveState()
  }

  /// The top row's fill for what it shows now. With nothing to show it is the toolbar, part of the keyboard, so it takes the keyboard's own fill: the candidate palette's surface there put keyboard-coloured icons on, say, a white skin card over a dark keyboard, where they all but vanished. While a canvas panel is still decoding or failed to, the same holds.
  private func currentStripBackground(_ skin: KeyboardTheme) -> UIColor {
    guard stripShowsCandidates else { return Self.stripBackground(skin, palette: nil) }
    return candidateCanvasSkin?.panelLoaded == true ? .clear : Self.stripBackground(skin, palette: candidatePalette)
  }

  /// The top row sits straight on the keyboard in the design (dc.html `mobCands`, the 48px row); the desktop palette brings its own surface, and a keyboard design keeps a veil of its key fill so the candidates stay legible over a photo or pattern.
  static func stripBackground(_ skin: KeyboardTheme, palette: CandidatePalette?) -> UIColor {
    if let palette { return palette.surface }
    if skin.design != nil { return skin.keyBackground.withAlphaComponent(0.6) }
    // 把背景交给系统底板的皮肤也不留完全透明：候选栏自己带一层几乎看不出的底色，从它上面起手的触摸才有落点（#10173）。
    return skin.drawsNativeBackground ? skin.nativeBackgroundFill : .clear
  }

  /// 给键盘换上 `KeyboardTheme.current`。`onlyIfChanged` 时（每次出现由 `viewWillAppear` 调用），皮肤和 trait 都与上一次换装相同、也没有按键在窗口外建过，就跳过重画背景、重走整棵按键树重设每个按键的配置这些重活；按键状态相关的更新（回车、方案键、工具栏、候选条）照常做。视图不在窗口里时只看皮肤：trait 还没定，在窗口外走的一遍也只会留下标记让 `viewIsAppearing` 再走一遍，这两项交给它判断。
  private func applyKeyboardSkin(onlyIfChanged: Bool = false) {
    let skin = KeyboardTheme.current
    let reskinsKeys = !onlyIfChanged || appliedKeySkin != skin
      || (view.window != nil && (keySkinNeedsWindowPass || appliedKeySkinTraits != traitCollection))
    if reskinsKeys {
      handwriting.canvas.setNeedsDisplay()
      replyModel.objectWillChange.send()
    }
    candidatePalette = currentCandidatePalette()
    refreshCandidatePalette()
    candidateButtonStyleRevision &+= 1
    if reskinsKeys { recolorKeys(skin) }
    styleReturnKey()
    updateLanguageModeButton()
    updateSchemeButton()
    updateShortcutButtons()
    renderCandidateStrip()
    updateSpellingStrip()
    updateSpaceKeyTitle()
    exitLocalModeButton.configuration?.baseForegroundColor = candidatePalette?.accent ?? skin.accent
    preeditButton.configuration?.baseForegroundColor = candidatePalette?.preeditText ?? candidatePalette?.secondary ?? skin.secondary
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
    applyCandidateAppearance()
  }

  /// 换装里的重活：键盘背景、整棵按键树和回车键的配置。每个按键重设一遍配置，设计皮肤还要重画每个按键表面，键盘图片或花纹的背景整块重画。
  private func recolorKeys(_ skin: KeyboardTheme) {
    // 把背景交给系统底板时不是完全透明，而是铺一层几乎看不出的 `nativeBackgroundFill`，按键之间的空白也带内容（#10173）。只铺这一层不够：#7682 里背后不透明的根视图没让透明按钮收到触摸，所以面板、网格、格子和候选栏各自也铺一层，见 `nativeBackgroundFill`。
    view.backgroundColor = skin.drawsNativeBackground ? skin.nativeBackgroundFill : skin.background
    skinBackdrop.skin = skin
    // 只在这里改，不改 KeyboardSkinBackgroundView：App 里的皮肤预览背后没有系统底板。底板关了交互，它这一层对系统命中是否有用不确定，铺上只是多一层保险，也是键盘自己的底色。
    if skin.drawsNativeBackground { skinBackdrop.backgroundColor = skin.nativeBackgroundFill }
    // 候选在这个滚动视图里左右滑；原生皮肤下它自己也带一层内容，从候选之间的空白起手的滑动才有收触摸的视图。
    candidateScrollView.backgroundColor = skin.drawsNativeBackground ? skin.nativeBackgroundFill : .clear
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
          ? currentStripBackground(skin)
          : skin.keyBackground.withAlphaComponent(0.6)
      }
      if let label = node as? UILabel, label.accessibilityIdentifier == "keyNumberHint" { label.textColor = skin.secondary }
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
    appliedKeySkin = skin
    appliedKeySkinTraits = traitCollection
    // 在窗口外走完的这一遍，阴影色仍按默认外观解析，下一次出现还要再走。
    keySkinNeedsWindowPass = view.window == nil
    // 在设备上确认出现时重走按键树的次数：皮肤没变时再出现应当一行都没有。
    DiagnosticLog.shared.write("keys_reskinned in_window=\(keySkinNeedsWindowPass ? 0 : 1)")
  }

  /// 键盘画在深色下：「键盘明暗」设成深色，或者跟随时宿主是深色。
  private var drawsDark: Bool { traitCollection.userInterfaceStyle == .dark }

  /// Draw the keyboard and its panels in the light or dark form the shared themes ask for (see KeyboardAppearancePreference). A changed style reaches `traitCollectionDidChange`, which redraws the skin.
  private func applyKeyboardAppearance() {
    let preferences = session.sharedPreferences
    // 明暗只由「键盘明暗」决定，主题不再把键盘拉到自己的明暗：属于另一种明暗的主题在这里按跟随系统画（`KeyboardTheme.reload(_:dark:)`），所以 `traitCollection` 始终是键盘真正的明暗。
    let keyboard = KeyboardAppearancePreference.style(KeyboardAppearancePreference.keyboardKey, in: preferences)
    if overrideUserInterfaceStyle != keyboard { overrideUserInterfaceStyle = keyboard }
    // 第一次出现时把键盘的明暗告诉 `KeyboardTheme`，之后的变化由 `traitCollectionDidChange` 跟上。明暗没变时不在这里重新解析，免得抢在 `synchronizeSharedTouchPreferences` 前面，让它以为皮肤没变而漏掉重画。
    if KeyboardTheme.mode != drawsDark { KeyboardTheme.reload(preferences, dark: drawsDark) }
    handwriting.overrideUserInterfaceStyle = KeyboardAppearancePreference.style(KeyboardAppearancePreference.handwritingKey, in: preferences)
    if let emojiPicker { applyEmojiPickerAppearance(emojiPicker) }
  }

  /// 默认从 App Group 读取；测试可在创建视图前注入独立的合成皮肤目录。
  var candidateSkinsRoot: URL? = ExternalCandidateSkin.defaultRoot

  private func currentCandidatePalette() -> CandidatePalette? {
    CandidatePalette.active(in: session.sharedPreferences, systemDark: drawsDark,
                            skinsRoot: candidateSkinsRoot, canvasPanelDrawn: candidateCanvasSkin?.panelLoaded == true)
  }

  /// 候选主题独立决定明暗；资源只在主题、模式或清单变化时重载，异步旧结果不能覆盖新选择。
  private func refreshCandidatePalette() {
    let preferences = session.sharedPreferences
    let dark = CandidatePalette.isDark(candidateTheme: preferences?["candidate_theme"] as? String,
                                       appMode: preferences?["theme"] as? String, systemDark: drawsDark)
    let resolved = CandidatePalette.followsDesktop ? CandidatePalette.resolveTheme(preferences, systemDark: drawsDark, skinsRoot: candidateSkinsRoot) : nil
    let id = resolved?.candidateSkin
    let root = candidateSkinsRoot
    var custom = GlobalThemePreference.customTheme(in: preferences)
    custom.removeValue(forKey: "keyboard")
    var request: [String: Any] = ["enabled": CandidatePalette.followsDesktop, "dark": dark,
      "theme": GlobalThemePreference.theme(in: preferences), "custom": custom]
    if let root { request["skins_root"] = root.path }
    if let id, let root {
      let manifest = root.appendingPathComponent(id).appendingPathComponent("skin.toml")
      let attributes = try? FileManager.default.attributesOfItem(atPath: manifest.path)
      request["manifest_time"] = (attributes?[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0
      request["manifest_size"] = attributes?[.size] as? NSNumber ?? 0
    }
    let key = try? JSONSerialization.data(withJSONObject: request, options: .sortedKeys)
    if key != candidateCanvasRequestKey {
      candidateCanvasEpoch &+= 1
      let epoch = candidateCanvasEpoch
      candidateCanvasRequestKey = key
      candidateCanvasSkin = nil
      if let id, let root {
        Self.candidateCanvasQueue.async { [weak self] in
          let assets = autoreleasepool { ExternalCandidateSkin.canvas(id, dark: dark, root: root) }
          DispatchQueue.main.async { [weak self] in
            guard let self, candidateCanvasEpoch == epoch, CandidatePalette.followsDesktop else { return }
            candidateCanvasSkin = assets
            candidatePalette = currentCandidatePalette()
            applyCandidateAppearance()
          }
        }
      }
    }
    let palette = currentCandidatePalette()
    guard palette != candidatePalette || compositionContainer?.canvas != candidateCanvasSkin else { return }
    candidatePalette = palette
    applyCandidateAppearance()
  }

  private func applyCandidateAppearance() {
    candidateButtonStyleRevision &+= 1
    candidateStack.spacing = candidatePalette?.layout?.horizontalGap(fontSize: Self.candidateChipFontSize * candidateFontScale) ?? 6
    let skin = KeyboardTheme.current
    compositionContainer?.canvas = candidateCanvasSkin
    updateCandidateCanvasMetrics()
    compositionContainer?.backgroundColor = currentStripBackground(skin)
    candidatePanel?.applyAppearance(palette: candidatePalette, canvas: candidateCanvasSkin)
    candidatePanel?.canvasVisible = candidateCanvasActive
    exitLocalModeButton.configuration?.baseForegroundColor = candidatePalette?.accent ?? skin.accent
    preeditButton.configuration?.baseForegroundColor = candidatePalette?.preeditText ?? candidatePalette?.secondary ?? skin.secondary
    expandCandidatesButton.configuration?.baseForegroundColor = candidatePalette?.text ?? skin.keyForeground
    renderCandidateStrip()
  }

  private func updateCandidateCanvasMetrics() {
    let readingHeight = Self.readingRowHeight(preeditScale: preeditFontScale, barScale: candidateBarScale)
    // 皮肤只占原行高里可让出的 10pt；放大字号与多行释义的高度全部留给文字。下限是词块的最小高度：低于它时词块和首选高亮会伸出面板。两者都按「候选栏大小」缩放。
    let textHeight = max(Self.candidateChipMinimumSize.height * candidateBarScale,
      Self.candidateRowHeight(candidateScale: candidateFontScale, glossed: glossLineCount > 0, barScale: candidateBarScale)
      + Self.glossHeight(lines: glossLineCount - 1, barScale: candidateBarScale) - 10 * candidateBarScale)
    let gap = separateReadingRow ? min(candidateCanvasSkin?.lineGap ?? 0, max(0, shownTopRowHeight - readingHeight - textHeight)) : 0
    compositionContainer?.minimumContentHeight = textHeight + (separateReadingRow ? readingHeight + gap : 0)
    candidatePreeditGapConstraint?.constant = gap
    compositionContainer?.setNeedsLayout()
  }

  private func refreshTraits(_ previousTraitCollection: UITraitCollection) {
    if isViewLoaded, actionRow != nil,
       previousTraitCollection.userInterfaceStyle != traitCollection.userInterfaceStyle {
      // 明暗变了，实际生效的主题可能跟着变：所选主题属于另一种明暗时换成跟随系统，切回来时换回所选主题。
      KeyboardTheme.reload(session.sharedPreferences, dark: drawsDark)
      applyKeyboardSkin()
      // 键盘的明暗变了，表情面板是否与它一致也要重新判断。
      if let emojiPicker { applyEmojiPickerAppearance(emojiPicker) }
    }
    // iPad 键盘停靠状态变化会改变宽度类别，需要重新计算布局。
    if isViewLoaded, actionRow != nil,
       previousTraitCollection.horizontalSizeClass != traitCollection.horizontalSizeClass {
      updateKeyboardLayoutIfNeeded()
      // 设备形态也决定同步的候选字号上限。
      applyCandidateGlossLayout()
    }
  }

  /// iPad 旋转时 size class 不变，trait 观察器不会被叫到；转完再核对一次分离式键盘（viewDidLayoutSubviews 通常已经处理过）。
  override func viewWillTransition(to size: CGSize, with coordinator: UIViewControllerTransitionCoordinator) {
    super.viewWillTransition(to: size, with: coordinator)
    coordinator.animate(alongsideTransition: nil) { [weak self] _ in
      guard let self else { return }
      updateSplitKeyboardIfNeeded()
      closePickerBuiltForOtherOrientation()
    }
  }

  /// 输入方式和功能面板按打开时的横竖屏定首选行数（`KeyboardPagedGridView.pickerRows`）。网格按真实高度缩放、转屏后也不会溢出，但横屏打开的两行面板转回竖屏仍只排两行，所以转屏后首选行数不再相同时关掉面板，再打开就按新方向排。
  private func closePickerBuiltForOtherOrientation() {
    guard morePicker != nil || schemePicker != nil else { return }
    let rows = { KeyboardPagedGridView.pickerRows(formFactor: self.formFactor, landscape: $0) }
    if rows(pickerLandscape) != rows(isLandscape) { closeKeyboardPicker() }
  }

  /// 按当前的按键音开关、完全访问和共享偏好（`plugins.key_sound` 的包和音量）决定要加载哪个音效包。同一个包不会重新加载，所以键盘每次出现、偏好每次重读之后都调用。
  private func synchronizeKeySoundPack() {
    let preferences = session.sharedPreferences
    keySoundGain = KeySoundPackPolicy.gain(volume: KeySoundPackPolicy.volume(in: preferences))
    wantedKeySoundPack = KeySoundPackPolicy.wantedPack(
      soundEnabled: KeyboardFeedbackPreference.soundEnabled, fullAccess: hasFullAccess, preferences: preferences)
    keySounds.want(wantedKeySoundPack)
  }

  @objc private func prepareKeyFeedback() {
    lastKeyTouchTime = ProcessInfo.processInfo.systemUptime
    if hapticsEnabled { keyFeedback.prepare() }
  }

  /// 按键音按 `KeySoundPackPolicy.playback` 出声：有完全访问、选中的包已经加载好时播包里这一类按键的样本，密码框里一律播普通键的；否则退回系统按键声。
  private func playInputClick(_ keyClass: KeySoundClass = .standard) {
    // 普通键在密码框里外都播同一个样本，只有空格、回车和退格才需要问一下输入框。
    let playback = KeySoundPackPolicy.playback(
      soundEnabled: KeyboardFeedbackPreference.soundEnabled, wanted: wantedKeySoundPack, loaded: keySounds.loaded,
      keyClass: keyClass, secure: keyClass != .standard && isCredentialField)
    switch playback {
    case .silent: break
    case .system: UIDevice.current.playInputClick()
    case .sample(let file): if !keySounds.play(file, gain: keySoundGain) { UIDevice.current.playInputClick() }
    }
    if hapticsEnabled {
      hapticStrength.impact(keyFeedback)
      keyFeedback.prepare()
    }
  }
}
