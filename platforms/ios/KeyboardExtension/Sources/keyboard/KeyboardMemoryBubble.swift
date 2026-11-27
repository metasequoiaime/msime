import UIKit

/// 双击品牌键弹出的内存读数气泡：一行写「占用 · 余量 · 峰值」，3 秒后自动消失，点气泡外任何地方也关掉。
///
/// 键盘里没有 Xcode 的内存仪表，未签名的测试包又拿不到 App Group，诊断日志写下的 `mem=`、`avail=` 到不了 App 那一侧，
/// 所以在键盘上直接显示一份。数字都来自 `DiagnosticLog.MemorySample`：`footprint` 是系统判断扩展是否超出上限时用的
/// 物理占用，`available` 是离上限还有多少（系统没给时为 0，见 `MemorySample` 的说明），`peak` 是启动以来的峰值。
///
/// 气泡盖住整块键盘用来接住「点外面关掉」的那一下：那一击不会落到下面的按键上，这是弹出层的常规做法。
/// 读数属于调试信息，气泡固定深色，不跟皮肤配色，浅色深色键盘上都看得清。
final class KeyboardMemoryBubble: UIView {
  /// 自动消失的等待时间。测试传入更短的值，免得真等 3 秒。
  static let autoDismissDelay: TimeInterval = 3
  private static let fadeDuration: TimeInterval = 0.15
  private static let pillCornerRadius: CGFloat = 12
  private static let pillInsets = UIEdgeInsets(top: 8, left: 14, bottom: 8, right: 14)
  /// 气泡离键盘左右边的距离；窄屏上文字折成两行也不贴边。
  private static let horizontalMargin: CGFloat = 12

  /// 接住气泡外那一下的透明遮罩。
  let catcher = UIControl()
  private let pill = UIView()
  private let label = UILabel()
  private var dismissTimer: Timer?
  private var finished = false

  /// 气泡关掉（自动、点外面或被键盘收走）之后调用一次；键盘用它清掉自己的引用。
  var onDismiss: (() -> Void)?
  /// 气泡上沿在容器里的位置；`nil` 时放在键盘上方三分之一处。键盘按品牌键的位置算好设进来。
  var pillTop: CGFloat? {
    didSet { if pillTop != oldValue { setNeedsLayout() } }
  }

  /// 气泡上的完整文字，读屏也念它。
  let text: String

  init(sample: DiagnosticLog.MemorySample, autoDismissAfter delay: TimeInterval = KeyboardMemoryBubble.autoDismissDelay) {
    text = Self.text(for: sample)
    super.init(frame: .zero)
    accessibilityIdentifier = "memoryBubble"
    backgroundColor = .clear

    catcher.frame = bounds
    catcher.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    catcher.accessibilityIdentifier = "memoryBubbleCatcher"
    catcher.addTarget(self, action: #selector(dismiss), for: .touchDown)
    addSubview(catcher)

    pill.backgroundColor = UIColor.black.withAlphaComponent(0.82)
    pill.layer.cornerRadius = Self.pillCornerRadius
    pill.layer.cornerCurve = .continuous
    pill.isUserInteractionEnabled = false
    addSubview(pill)

    label.text = text
    label.font = .monospacedDigitSystemFont(ofSize: 13, weight: .medium)
    label.textColor = .white
    label.textAlignment = .center
    label.numberOfLines = 2
    label.isAccessibilityElement = true
    label.accessibilityLabel = text
    pill.addSubview(label)

    let timer = Timer(timeInterval: delay, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated { self?.dismiss() }
    }
    RunLoop.main.add(timer, forMode: .common)
    dismissTimer = timer
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 「占用 41.2 MiB · 余量 12.0 MiB · 峰值 48.0 MiB」。
  ///
  /// `available` 为 0 时写「余量未知」：系统没有给出上限（模拟器、或按 `os/proc.h` 的说明调用方不是 App）与
  /// 已经超限都是 0，不能写成「余量 0」把人吓一跳。峰值取系统和当前占用里大的那个，系统没给峰值时不会出现比占用还小。
  static func text(for sample: DiagnosticLog.MemorySample) -> String {
    let peak = max(sample.peak, sample.footprint)
    return "占用 \(mebibytes(sample.footprint)) · "
      + "余量 \(sample.available == 0 ? "未知" : mebibytes(sample.available)) · "
      + "峰值 \(mebibytes(peak))"
  }

  /// 字节数换成 MiB，保留一位小数（`41.2 MiB`）。
  static func mebibytes(_ bytes: UInt64) -> String {
    String(format: "%.1f MiB", Double(bytes) / (1024 * 1024))
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    catcher.frame = bounds
    guard bounds.width > 0, bounds.height > 0 else { return }
    let textWidth = bounds.width - 2 * Self.horizontalMargin - Self.pillInsets.left - Self.pillInsets.right
    let textSize = label.sizeThatFits(CGSize(width: max(textWidth, 1), height: .greatestFiniteMagnitude))
    let pillWidth = min(textSize.width, textWidth) + Self.pillInsets.left + Self.pillInsets.right
    let pillHeight = textSize.height + Self.pillInsets.top + Self.pillInsets.bottom
    let top = min(max(pillTop ?? bounds.height / 3, 8), bounds.height - pillHeight - 8)
    pill.frame = CGRect(x: ((bounds.width - pillWidth) / 2).rounded(), y: top,
                        width: pillWidth, height: pillHeight)
    label.frame = CGRect(x: Self.pillInsets.left, y: Self.pillInsets.top,
                         width: pillWidth - Self.pillInsets.left - Self.pillInsets.right, height: textSize.height)
  }

  /// 关掉气泡：先淡出再摘掉，重复调用只生效一次。
  @objc func dismiss() {
    guard !finished else { return }
    finished = true
    dismissTimer?.invalidate()
    dismissTimer = nil
    UIView.animate(withDuration: Self.fadeDuration, animations: { self.alpha = 0 }) { _ in
      self.removeFromSuperview()
      self.onDismiss?()
    }
  }
}
