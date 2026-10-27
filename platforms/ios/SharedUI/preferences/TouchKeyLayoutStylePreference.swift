import CoreGraphics
import Foundation

/// 「按键布局」里 26 键的底行样式，共享文档里是 `touch_twenty_six_key_style`（client-core `TouchTwentySixKeyStyle`），作用于手机中文 26 键字母层；英文 26 键和 iPad 全尺寸键盘不受它影响。各样式的排法见 `TouchBottomRowLayout`。
enum TouchTwentySixKeyStyle: String, CaseIterable, Hashable {
  /// 123 | 符 | 空格 | ，（长按出。）| 中/英 | 回车，即原来的底行。
  case standard
  /// 123 | 符 | ， | 空格 | 。 | 中/英 | 回车。
  case commaPeriod = "comma_period"
  /// 123 | ， | 长空格 | 中/英 | 回车，符号入口移到工具栏，逗号下滑或长按输入句号。
  case longSpace = "long_space"
  /// 123 | 😀 | 空格 | ， | 中/英 | 回车，符号面板经长按 123 打开。
  case emoji

  var title: String {
    switch self {
    case .standard: "标准"
    case .commaPeriod: "逗号句号在空格两侧"
    case .longSpace: "长空格"
    case .emoji: "表情键"
    }
  }
}

/// 14 键与 18 键共用的底行样式，共享文档里是 `touch_fourteen_key_style`（client-core `TouchFourteenKeyStyle`）。
enum TouchFourteenKeyStyle: String, CaseIterable, Hashable {
  /// 123 | ， | 空格 | 。 | 中/英 | 回车，即原来的底行。
  case standard
  /// 123 | ， | 长空格 | 中/英 | 回车，句号的份额给空格。
  case longSpace = "long_space"
  /// 123 | 😀 | 空格 | ， | 中/英 | 回车。
  case emoji

  var title: String {
    switch self {
    case .standard: "标准"
    case .longSpace: "长空格"
    case .emoji: "表情键"
    }
  }
}

/// 九键字母层的底行样式，拼音九键、九键双拼和笔画共用，共享文档里是 `touch_nine_key_style`（client-core `TouchNineKeyStyle`）。左侧符号栏、右侧功能列和数字层不变。
enum TouchNineKeyStyle: String, CaseIterable, Hashable {
  /// 符 | 123 | 空格 | 中/英 | 回车，即原来的底行。
  case standard
  /// 符 | 123 | ， | 空格 | 。 | 中/英 | 回车。
  case commaPeriod = "comma_period"
  /// 符 | 123 | 😀 | 空格 | 中/英 | 回车。
  case emoji

  var title: String {
    switch self {
    case .standard: "标准"
    case .commaPeriod: "底栏带标点"
    case .emoji: "表情键"
    }
  }
}

/// 触屏键盘的按键布局：三类键面各自的底行样式和顶部数字行（`touch_number_row`），三端共用同一组共享偏好，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md。
///
/// 共享文档是权威来源，键盘从 App Group 镜像读：键盘和设置页读到文档时刷新镜像（`mirror`），设置页保存时文档接受了才写镜像，与 `TouchShuangpinLayoutPreference` 和「组字时保留 Shift」的做法相同。iOS 以前没有对应的本机设置，没有旧值要迁移。
struct TouchKeyLayoutStyle: Equatable, Hashable {
  var twentySixKey: TouchTwentySixKeyStyle = .standard
  var fourteenKey: TouchFourteenKeyStyle = .standard
  var nineKey: TouchNineKeyStyle = .standard
  /// 在 26 键、14 键和 18 键字母层上方画一排 1 到 0；手机横屏时不画，存着的值不变。
  var numberRow = false

  static let twentySixKeyDocumentKey = "touch_twenty_six_key_style"
  static let fourteenKeyDocumentKey = "touch_fourteen_key_style"
  static let nineKeyDocumentKey = "touch_nine_key_style"
  static let numberRowDocumentKey = "touch_number_row"

  private static let twentySixKeyMirrorKey = "keyboard.style.twentySixKey"
  private static let fourteenKeyMirrorKey = "keyboard.style.fourteenKey"
  private static let nineKeyMirrorKey = "keyboard.style.nineKey"
  private static let numberRowMirrorKey = "keyboard.numberRow"

  init(twentySixKey: TouchTwentySixKeyStyle = .standard, fourteenKey: TouchFourteenKeyStyle = .standard,
       nineKey: TouchNineKeyStyle = .standard, numberRow: Bool = false) {
    self.twentySixKey = twentySixKey
    self.fourteenKey = fourteenKey
    self.nineKey = nineKey
    self.numberRow = numberRow
  }

  /// 文档里的四项。缺键、不是字符串或不认识的写法（较新版本加的样式）一律按标准读，与 client-core 的宽松读取一致；数字行不是布尔值时按关。
  init(in document: [String: Any]?) {
    twentySixKey = (document?[Self.twentySixKeyDocumentKey] as? String).flatMap(TouchTwentySixKeyStyle.init(rawValue:)) ?? .standard
    fourteenKey = (document?[Self.fourteenKeyDocumentKey] as? String).flatMap(TouchFourteenKeyStyle.init(rawValue:)) ?? .standard
    nineKey = (document?[Self.nineKeyDocumentKey] as? String).flatMap(TouchNineKeyStyle.init(rawValue:)) ?? .standard
    numberRow = document?[Self.numberRowDocumentKey] as? Bool ?? false
  }

  private static var defaults: UserDefaults { InputSchemePreference.appGroupDefaults }

  /// App Group 里的镜像，键盘画键时读它，不读文档。
  static var mirrored: TouchKeyLayoutStyle {
    get {
      TouchKeyLayoutStyle(
        twentySixKey: defaults.string(forKey: twentySixKeyMirrorKey).flatMap(TouchTwentySixKeyStyle.init(rawValue:)) ?? .standard,
        fourteenKey: defaults.string(forKey: fourteenKeyMirrorKey).flatMap(TouchFourteenKeyStyle.init(rawValue:)) ?? .standard,
        nineKey: defaults.string(forKey: nineKeyMirrorKey).flatMap(TouchNineKeyStyle.init(rawValue:)) ?? .standard,
        numberRow: defaults.object(forKey: numberRowMirrorKey) as? Bool ?? false)
    }
    set {
      defaults.set(newValue.twentySixKey.rawValue, forKey: twentySixKeyMirrorKey)
      defaults.set(newValue.fourteenKey.rawValue, forKey: fourteenKeyMirrorKey)
      defaults.set(newValue.nineKey.rawValue, forKey: nineKeyMirrorKey)
      defaults.set(newValue.numberRow, forKey: numberRowMirrorKey)
    }
  }

  /// 把文档里的四项抄进 App Group 镜像；读不到文档时保留镜像。
  static func mirror(_ document: [String: Any]?) {
    guard document != nil else { return }
    let style = TouchKeyLayoutStyle(in: document)
    if style != mirrored { mirrored = style }
  }

  /// 每次只写用户刚改的那一项，其余保持文档里的样子；文档接受了才更新镜像。标准样式和关着的数字行由 client-core 从文档里省掉。
  @discardableResult
  static func save(twentySixKey style: TouchTwentySixKeyStyle, stateRoot: URL? = nil) -> Bool {
    save(key: twentySixKeyDocumentKey, value: style.rawValue, stateRoot: stateRoot) { $0.twentySixKey = style }
  }

  @discardableResult
  static func save(fourteenKey style: TouchFourteenKeyStyle, stateRoot: URL? = nil) -> Bool {
    save(key: fourteenKeyDocumentKey, value: style.rawValue, stateRoot: stateRoot) { $0.fourteenKey = style }
  }

  @discardableResult
  static func save(nineKey style: TouchNineKeyStyle, stateRoot: URL? = nil) -> Bool {
    save(key: nineKeyDocumentKey, value: style.rawValue, stateRoot: stateRoot) { $0.nineKey = style }
  }

  @discardableResult
  static func save(numberRow enabled: Bool, stateRoot: URL? = nil) -> Bool {
    save(key: numberRowDocumentKey, value: enabled, stateRoot: stateRoot) { $0.numberRow = enabled }
  }

  private static func save(key: String, value: Any, stateRoot: URL?, apply: (inout TouchKeyLayoutStyle) -> Void) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { $0[key] = value }) else { return false }
    var style = mirrored
    apply(&style)
    mirrored = style
    return true
  }
}

/// 手机触屏底行按「按键布局」样式的排法：每个显示的键、从左到右的顺序和它在本行里的宽度份额。三端相同，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md。
///
/// 标准样式就是原来的底行（`KeyboardViewController` 的 `phoneActionWidths`、`nineKeyActionWidths` 等），表里照抄一份，供统计热力图、设置页预览和测试使用；其余样式由键盘控制器按这张表排列并加宽度约束。样式加的键都从空格里让出份额，整行总份额不变。地球键只在宿主要求时出现，占 1 份，从行宽里另外分；隐藏中/英键时它的 1.05 份交给空格。
enum TouchBottomRowLayout {
  enum Slot: String, Hashable {
    case symbols, layer, globe, emoji, comma, space, period, zero, language, returnKey
  }

  struct Key: Hashable {
    let slot: Slot
    let weight: CGFloat

    init(_ slot: Slot, _ weight: CGFloat) {
      self.slot = slot
      self.weight = weight
    }
  }

  static let layerWeight: CGFloat = 1.25
  static let symbolsWeight: CGFloat = 1.05
  static let languageWeight: CGFloat = 1.05
  static let globeWeight: CGFloat = 1
  static let punctuationWeight: CGFloat = 1
  /// 26 键、14 键和 18 键底行的空格与回车。
  static let spaceWeight: CGFloat = 4
  static let returnWeight: CGFloat = 1.9
  /// 九键底行的空格与回车；九键双拼字母层的 0 与 符 同宽。
  static let nineKeySpaceWeight: CGFloat = 4.2
  static let nineKeyReturnWeight: CGFloat = 1.6
  static let zeroWeight: CGFloat = 1.05

  /// 中文 26 键字母层。标准：123 | 符 | 空格 | ， | 中/英 | 回车；逗号句号在空格两侧：。从空格让出 1 份；长空格：符 的 1.05 份给空格；表情键：😀 占 符 的位置和份额。
  static func twentySixKey(_ style: TouchTwentySixKeyStyle, globe: Bool, language: Bool = true) -> [Key] {
    var keys = [Key(.layer, layerWeight)]
    if globe { keys.append(Key(.globe, globeWeight)) }
    switch style {
    case .standard:
      keys += [Key(.symbols, symbolsWeight), Key(.space, spaceWeight), Key(.comma, punctuationWeight)]
    case .commaPeriod:
      keys += [Key(.symbols, symbolsWeight), Key(.comma, punctuationWeight),
               Key(.space, spaceWeight - punctuationWeight), Key(.period, punctuationWeight)]
    case .longSpace:
      keys += [Key(.comma, punctuationWeight), Key(.space, spaceWeight + symbolsWeight)]
    case .emoji:
      keys += [Key(.emoji, symbolsWeight), Key(.space, spaceWeight), Key(.comma, punctuationWeight)]
    }
    return finish(keys, language: language, returnWeight: returnWeight)
  }

  /// 14 键与 18 键字母层。标准：123 | ， | 空格 | 。 | 中/英 | 回车；长空格：。的份额给空格；表情键：123 | 😀 | 空格 | ， | 中/英 | 回车，😀 用原来 ， 的份额，， 用原来 。 的份额。
  static func letterGrid(_ style: TouchFourteenKeyStyle, globe: Bool, language: Bool = true) -> [Key] {
    var keys = [Key(.layer, layerWeight)]
    if globe { keys.append(Key(.globe, globeWeight)) }
    switch style {
    case .standard:
      keys += [Key(.comma, punctuationWeight), Key(.space, spaceWeight), Key(.period, punctuationWeight)]
    case .longSpace:
      keys += [Key(.comma, punctuationWeight), Key(.space, spaceWeight + punctuationWeight)]
    case .emoji:
      keys += [Key(.emoji, punctuationWeight), Key(.space, spaceWeight), Key(.comma, punctuationWeight)]
    }
    return finish(keys, language: language, returnWeight: returnWeight)
  }

  /// 九键字母层（拼音九键、九键双拼、笔画）。标准：符 | 123 | 空格 | 中/英 | 回车，九键双拼在空格后多一个 0；底栏带标点：， 和 。 各从空格让出 1 份；表情键：😀 从空格让出 1.05 份。数字层不用这张表。
  static func nineKey(_ style: TouchNineKeyStyle, globe: Bool, zero: Bool = false, language: Bool = true) -> [Key] {
    var keys = [Key(.symbols, symbolsWeight), Key(.layer, layerWeight)]
    if globe { keys.append(Key(.globe, globeWeight)) }
    switch style {
    case .standard:
      keys.append(Key(.space, nineKeySpaceWeight))
    case .commaPeriod:
      keys += [Key(.comma, punctuationWeight), Key(.space, nineKeySpaceWeight - 2 * punctuationWeight),
               Key(.period, punctuationWeight)]
    case .emoji:
      keys += [Key(.emoji, symbolsWeight), Key(.space, nineKeySpaceWeight - symbolsWeight)]
    }
    if zero { keys.append(Key(.zero, zeroWeight)) }
    return finish(keys, language: language, returnWeight: nineKeyReturnWeight)
  }

  /// 补上中/英和回车；隐藏中/英时把它的份额并进空格。
  private static func finish(_ keys: [Key], language: Bool, returnWeight: CGFloat) -> [Key] {
    var keys = keys
    if language {
      keys.append(Key(.language, languageWeight))
    } else if let index = keys.firstIndex(where: { $0.slot == .space }) {
      keys[index] = Key(.space, keys[index].weight + languageWeight)
    }
    keys.append(Key(.returnKey, returnWeight))
    return keys
  }
}
