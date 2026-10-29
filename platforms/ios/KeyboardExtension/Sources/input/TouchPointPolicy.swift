import CoreGraphics
import Foundation

/// 触屏 26 键双拼纠错（#10018）里与 UIKit 无关的那部分：哪一键带着按下点交给 `msime_client_touch_character`，注册给 `msime_client_set_key_geometry` 的键位几何 JSON 怎么拼，以及什么时候要重新注册。
///
/// 坐标都在键区（`KeyAreaStackView`）坐标系里，与滑行输入同一个；键的矩形取布局位置（`KeyAreaStackView.layoutFrame`），不受按下缩放影响，键距归属（`KeyGapRouting`）也不挪动按下点。宿主只上报按下点和几何，距离和罚分都由引擎算。落点只在会话内存里给双拼纠错计价，不落盘、不进统计。与 Android、HarmonyOS 的 `TouchPointPolicy` 是同一套规则。
enum TouchPointPolicy {
  /// 键区里此刻的字母键几何：a..z 二十六个字母键的布局矩形，按字母顺序；`semicolon` 是第二排显示中的 `;` 键（微软双拼、粤语双拼），其余方案为 nil；粤语双拼不走触点纠错（`sendsTouch` 只认 26 键双拼），不会把这份几何注册给引擎。iOS 的分离键盘只在中间加一道中缝，不重复任何字母键，所以没有 `duplicates`。
  struct Layout: Equatable {
    var letters: [CGRect]
    var semicolon: CGRect?
  }

  /// `msime_client_set_key_geometry` 接受的 JSON 最大字节数。
  static let requestByteLimit = 16_384

  /// 带按下点的键：小写字母和微软双拼的 `;`。
  private static let touchKeys = Set("abcdefghijklmnopqrstuvwxyz;")

  /// 这一键是否带着按下点走 `msime_client_touch_character`。要同时满足：
  /// - 字母键显示的是 26 键双拼（`shuangpinLetters`：中文模式、双拼方案、不是 14/18 键网格、不在本地模式）；
  /// - 这一键是小写字母或 `;`（辅助码的大写、`'` 和其他键照常走 `msime_client_character`）；
  /// - 按键记下了这一次触摸的按下点（`sendActions` 和程序化的激活没有）；
  /// - 没开 VoiceOver：读屏的双击落在键的激活点，不是手指的落点。
  static func sendsTouch(character: String, shuangpinLetters: Bool, hasTouchDown: Bool, voiceOver: Bool) -> Bool {
    guard shuangpinLetters, hasTouchDown, !voiceOver, character.count == 1, let key = character.first else { return false }
    return touchKeys.contains(key)
  }

  /// `msime_client_set_key_geometry` 的请求：`{"id":u32,"keys":[[cx,cy,w,h] × 26，按 a..z],"semicolon":[cx,cy,w,h] 或 null,"duplicates":[]}`，坐标保留一位小数。`id` 为 0（那是清除）、不是 26 个字母键、某个字母键是空矩形或有非有限值时为 nil；`;` 键是空矩形或有非有限值时写 null，这时按 `;` 不带落点，字母照常。
  static func request(id: UInt32, layout: Layout) -> Data? {
    guard id != 0, layout.letters.count == 26, layout.letters.allSatisfy(isUsable) else { return nil }
    let keys = layout.letters.map(rect).joined(separator: ",")
    let semicolon = layout.semicolon.flatMap { isUsable($0) ? rect($0) : nil } ?? "null"
    let json = "{\"id\":\(id),\"keys\":[\(keys)],\"semicolon\":\(semicolon),\"duplicates\":[]}"
    let data = Data(json.utf8)
    return data.count <= requestByteLimit ? data : nil
  }

  /// 已经注册给会话的几何和它的代次。只在内容变了、或会话里已不是这份几何（会话重建后句柄换了）时才重新注册，不是每键都发：每键多一次跨 C ABI 的调用会落在按键延迟上。
  struct Registration: Equatable {
    private(set) var layout: Layout?
    private(set) var id: UInt32

    /// `id` 是上一个用过的代次，新代次从它的下一个数起；键盘从 0 起。
    init(id: UInt32 = 0) {
      self.id = id
    }

    /// 会话里此刻注册着代次 `sessionID`（没有为 nil）时，按 `layout` 打字之前是否要先注册它。
    func needsRegistering(_ layout: Layout, sessionID: UInt32?) -> Bool {
      layout != self.layout || sessionID == nil || sessionID != id
    }

    /// 给 `layout` 换一个新代次并记下它：递增，跳过 0（0 表示清除）。
    mutating func advance(to layout: Layout) -> UInt32 {
      id = id == .max ? 1 : id + 1
      self.layout = layout
      return id
    }

    /// 注册失败：忘掉记下的几何，下一键再试。
    mutating func reset() {
      layout = nil
    }
  }

  private static func isUsable(_ frame: CGRect) -> Bool {
    !frame.isNull && frame.width > 0 && frame.height > 0
      && frame.minX.isFinite && frame.minY.isFinite && frame.width.isFinite && frame.height.isFinite
  }

  private static func rect(_ frame: CGRect) -> String {
    "[\(number(frame.midX)),\(number(frame.midY)),\(number(frame.width)),\(number(frame.height))]"
  }

  private static func number(_ value: CGFloat) -> String {
    String(format: "%.1f", Double(value))
  }
}
