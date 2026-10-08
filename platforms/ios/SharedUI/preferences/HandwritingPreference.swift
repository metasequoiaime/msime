import Foundation

/// 手写输入的书写与笔迹：识别等待时间、笔迹颜色和笔迹粗细，与 Android `HandwritingPage` 的三项相同。
///
/// 这几项与按键反馈开关一样只存在 App Group 的 defaults 里：设置 App 写入，键盘的手写面板每次打开时读取。它们不在共享偏好文档里，iOS 的设置同步也不携带它们，所以不随账号同步；Android 的 `platform.android.handwriting_*` 则经 `android_local` 同步。
enum HandwritingPreference {
  static let delayKey = "handwriting.delayMs"
  static let strokeColorKey = "handwriting.strokeColor"
  static let strokeWidthKey = "handwriting.strokeWidth"

  static let delayRange = 200...1500
  static let delayStep = 100
  /// 识别等待时间的默认值，与 Android `platform.android.handwriting_delay_ms` 的 500 ms 相同；设计稿写的是 600，以 Android 为准。
  static let defaultDelay = 500
  static let strokeWidthRange = 1...8
  static let defaultStrokeWidth = 3

  /// 笔迹颜色的可选项，取值沿用 Android 存储的 id。
  enum StrokeColor: String, CaseIterable {
    case followSkin = "follow_skin"
    case black
    case white
    case blue

    var title: String {
      switch self {
      case .followSkin: "跟随皮肤"
      case .black: "黑色"
      case .white: "白色"
      case .blue: "蓝色"
      }
    }
  }

  static var defaults: UserDefaults {
    UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard
  }

  /// 最后一笔之后面板等多久开始识别，单位毫秒：200…1500 之间 100 的整数倍。
  static var delayMilliseconds: Int {
    get { clampedDelay(defaults.object(forKey: delayKey) as? Int ?? defaultDelay) }
    set { defaults.set(clampedDelay(newValue), forKey: delayKey) }
  }

  static var strokeColor: StrokeColor {
    get { StrokeColor(rawValue: defaults.string(forKey: strokeColorKey) ?? "") ?? .followSkin }
    set { defaults.set(newValue.rawValue, forKey: strokeColorKey) }
  }

  /// 笔迹粗细，单位 pt，1…8。
  static var strokeWidth: Int {
    get { clampedWidth(defaults.object(forKey: strokeWidthKey) as? Int ?? defaultStrokeWidth) }
    set { defaults.set(clampedWidth(newValue), forKey: strokeWidthKey) }
  }

  /// 把三项都恢复为默认值，供「重置所有设置」使用。
  static func reset() {
    for key in [delayKey, strokeColorKey, strokeWidthKey] { defaults.removeObject(forKey: key) }
  }

  /// 取整到最近的步长并限制在范围内，这样旧版本或手工写入的值不会以奇怪的延迟传到计时器上。
  static func clampedDelay(_ value: Int) -> Int {
    let bounded = min(max(value, delayRange.lowerBound), delayRange.upperBound)
    return Int((Double(bounded) / Double(delayStep)).rounded()) * delayStep
  }

  static func clampedWidth(_ value: Int) -> Int {
    min(max(value, strokeWidthRange.lowerBound), strokeWidthRange.upperBound)
  }
}
