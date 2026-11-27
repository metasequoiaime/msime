import CoreGraphics
import Foundation

/// 按住 ⌫ 横滑删除：按住不放往左滑，滑多远删多少；不松手再往右滑，刚删掉的按原样一个个恢复；松手才算定下来。
///
/// 删的单位有两种（`Unit`）：组字时一格删一个音节（对引擎做一次按音节删，与长按 ⌫ 连删相同），删掉的那段码记在 `removedCodes`，恢复时按原顺序重打；没有组字时是光标前的文字，按字形（`Character`）一个个删，恢复是把删掉的字原样插回。这样两种情况下滑一格大致都是一个字。这个类型只管「滑到这里该删到第几格」和删掉了什么，真正去删和插回由键盘控制器做。
struct BackspaceSwipeDeletion: Equatable {
  /// 每删一个单位要往左滑的距离（pt），由设置里的「滑动删除速度」决定（`KeyboardLayoutPreference.backspaceSwipeStep`），默认第 7 档 10pt。
  let step: Double

  enum Unit: Equatable {
    /// 组字的组码：`original` 是开始滑时的整段码，从末尾一个音节一个音节地删。
    case composition(original: String)
    /// 光标前的文字：`context` 是开始滑时光标前读得到的那段文字。
    case text(context: String)
  }

  let unit: Unit
  /// 这次滑动最多能删几格：光标前读得到的字数；组字时音节长短不一、事先不知道有几个，先按组码字数估上限，组字删空时由 `stop(at:)` 收紧。
  private(set) var available: Int
  /// 组字时每一格删掉的那段码，按删的先后排；恢复时从后往前重打。
  private(set) var removedCodes: [String] = []
  /// 删组码时有一格对不上（删完后的码不是删之前的前缀），这次滑动就不再恢复，免得重打出错的码。
  private(set) var restorable = true
  /// 目前已经删掉的单位数。
  private(set) var applied = 0

  init(unit: Unit, step: Double = KeyboardLayoutPreference.backspaceSwipeStep(speed: KeyboardLayoutPreference.backspaceSwipeSpeedDefault)) {
    self.unit = unit
    self.step = max(1, step)
    switch unit {
    case .composition(let original): available = original.count
    case .text(let context): available = context.count
    }
  }

  /// 水平位移 `translation`（往左为负）对应应当删掉几个单位：往左每满一个 `step` 删一个，最多 `available`；滑回起点或往右都是 0。
  func target(translation: Double) -> Int {
    guard translation < 0, translation.isFinite else { return 0 }
    return min(available, Int((-translation / step).rounded(.down)))
  }

  /// 滑到 `translation` 时要做的事：正数是还要再删几个，负数是要恢复几个，0 是不动。调用后 `applied` 记成新的目标。
  mutating func move(to translation: Double) -> Int {
    let goal = target(translation: translation)
    defer { applied = goal }
    return goal - applied
  }

  /// 记下组字时一格删掉的码：`before` 是删之前的整段码，`after` 是删之后的。`after` 是 `before` 的前缀时，删掉的就是多出来的那段；否则这次滑动不再恢复。
  mutating func recordRemoved(before: String, after: String) {
    guard restorable, after.count < before.count, before.hasPrefix(after) else {
      restorable = false
      return
    }
    removedCodes.append(String(before.dropFirst(after.count)))
  }

  /// 取出最近删掉的一段码，恢复时重打它。
  mutating func popRemovedCode() -> String? { removedCodes.popLast() }

  /// 组字在删到第 `count` 格时已经删空：之后再往左滑也不再删，往右滑从这里恢复。
  mutating func stop(at count: Int) {
    applied = count
    available = count
  }

  /// 删文字时第 `index` 个（从 0 起，离光标最近的是 0）删掉的字，恢复时原样插回。
  func deletedText(at index: Int) -> String? {
    guard case .text(let context) = unit else { return nil }
    let characters = Array(context)
    guard index >= 0, index < characters.count else { return nil }
    return String(characters[characters.count - 1 - index])
  }

  /// 组字能不能按组码滑删：码不空、光标在末尾（删的总是末尾的码），而且每个码都是重打得回去的 ASCII 字母、数字或分词符 `'`。双拼的 `;`、假名、注音这类码不在其内，这时不接横滑，⌫ 照常按下删一个、按住连删。
  static func canRetype(composition: String, caret: Int) -> Bool {
    !composition.isEmpty && caret == composition.count
      && composition.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "'") }
  }
}

/// 删除键上滑清空：按住 ⌫ 往上滑过 `armDistance` 后待命，这时松手清掉正在输入的组码和候选；没有组字时清空光标前的文字。待命后滑回 `cancelDistance` 以内取消，再松手什么也不做。
///
/// 横滑删除仍由 `BackspaceSwipeDeletion` 负责。两者共用删除键上的同一个 pan，按起手方向分开（`KeyboardViewController.handleBackspaceSwipe`）：竖直向上是清空，向左是逐格删除。这个类型只管上滑到了哪个阶段，真正去清空由键盘控制器做。
struct BackspaceClearSwipe: Equatable {
  /// 手指离按下点往上这么远才待命（pt）。与 Android `BackspaceSwipePolicy.ARM_DP` 相同：点按删除时手指轻微上移不会清空。
  static let armDistance: CGFloat = 40
  /// 待命后滑回到这个距离以内就取消待命，避免滑回来时还停在待命状态误清。
  static let cancelDistance: CGFloat = 16

  /// 手指此刻是不是已经滑到待命位置。
  private(set) var armed = false

  /// 手指往上移动 `upward`（正数）之后的待命状态：滑过 `armDistance` 待命，滑回 `cancelDistance` 以内取消，中间保持原状。距离不是有限数时保持原状。
  @discardableResult
  mutating func move(upward: CGFloat) -> Bool {
    guard upward.isFinite else { return armed }
    if upward >= Self.armDistance {
      armed = true
    } else if upward < Self.cancelDistance {
      armed = false
    }
    return armed
  }
}

/// 清空光标前的文字。`UITextDocumentProxy` 没有按范围删除的接口，只能一个一个 `deleteBackward`；每轮先读光标前的一段，再按读到的字数逐个删，读到空或满轮数就停。
enum BackspaceClear {
  /// 每轮最多读、删的字数，与 Android `BackspaceSwipePolicy.CLEAR_CHUNK` 相同。
  static let chunk = 4096
  /// 最多删几轮（约二十六万字），编辑器不配合时也会停下。
  static let maxRounds = 64

  /// 反复读光标前的文字并逐字删掉，返回删掉的字数。读、删两个操作由调用方给出，便于回归测试。
  @discardableResult
  static func clearBeforeCursor(read: () -> String?, deleteOne: () -> Void) -> Int {
    var deleted = 0
    for _ in 0..<maxRounds {
      guard let context = read(), !context.isEmpty else { break }
      let count = min(context.count, chunk)
      guard count > 0 else { break }
      for _ in 0..<count {
        deleteOne()
        deleted += 1
      }
    }
    return deleted
  }
}
