/// 中文数字九宫格的临时随机键序（#10254），与 Android `RandomDigitOrderPolicy` 一一对应：键位 0–8 是网格的 1–9，键位 9 是底行的 0。
///
/// 开关只属于键盘这一次运行，不保存：工具栏的「随」点一下打开、再点一下关闭。打开后第一次在随机资格成立时取键面才打乱，离开数字页不取新随机数，回来沿用上次排列；每输入一个数字重新打乱，随机数恰好给出原排列时也强制换一次序。关掉后恢复 1234567890。
struct RandomDigitOrderPolicy {
  /// 标准键序：1–9 在网格，0 在底行。
  static let standard = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0]

  private let nextInt: (Int) -> Int
  private var digits = Self.standard
  private(set) var enabled = false
  private var prepared = false

  /// - Parameter nextInt: 返回 `0..<bound` 里的一个整数；测试传固定序列。
  init(nextInt: @escaping (Int) -> Int = { Int.random(in: 0..<$0) }) {
    self.nextInt = nextInt
  }

  mutating func setEnabled(_ value: Bool) {
    guard enabled != value else { return }
    enabled = value
    prepared = false
  }

  /// 键位 `slot` 此刻画哪个数字。没开或没有随机资格（不在中文数字页）时是标准键序，不取随机数。
  mutating func digit(slot: Int, eligible: Bool) -> Int {
    precondition(Self.standard.indices.contains(slot), "Invalid digit slot \(slot)")
    guard enabled && eligible else { return (slot + 1) % 10 }
    if !prepared { shuffle() }
    return digits[slot]
  }

  /// 十个键位按顺序的数字，同 `digit` 的规则。
  mutating func faces(eligible: Bool) -> [Int] {
    Self.standard.indices.map { digit(slot: $0, eligible: eligible) }
  }

  /// 一个数字上屏之后：开着且有资格时重新打乱。
  mutating func afterDigit(eligible: Bool) {
    if enabled && eligible { shuffle() }
  }

  private mutating func shuffle() {
    let previous = digits
    for index in stride(from: digits.count - 1, to: 0, by: -1) {
      digits.swapAt(index, nextInt(index + 1))
    }
    // 即使随机数恰好给出原排列，这次点击也要产生新的键序。
    if digits == previous { digits.swapAt(0, 1) }
    prepared = true
  }
}
