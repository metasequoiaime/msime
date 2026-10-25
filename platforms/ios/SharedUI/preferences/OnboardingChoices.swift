import Foundation

/// 引导选择的待保存队列由主应用持有；共享偏好写入成功前保留，不能只写键盘会覆盖的镜像。
final class OnboardingChoices {
  static let shared = OnboardingChoices()
  private let defaults: UserDefaults
  private let schemeKey = "onboarding.pendingScheme"
  private let glossKey = "onboarding.pendingGloss"

  init(defaults: UserDefaults = .standard) { self.defaults = defaults }

  var scheme: ChineseInputScheme? {
    defaults.string(forKey: schemeKey).flatMap(ChineseInputScheme.init(rawValue:))
  }
  var gloss: Bool? {
    defaults.object(forKey: glossKey) == nil ? nil : defaults.bool(forKey: glossKey)
  }
  var hasPending: Bool { scheme != nil || gloss != nil }

  func remember(_ scheme: ChineseInputScheme) { defaults.set(scheme.rawValue, forKey: schemeKey) }
  func remember(gloss: Bool) { defaults.set(gloss, forKey: glossKey) }

  /// 每次只清掉已经写成且没有被新选择替换的那一项；失败不会让另一项的成功丢失。
  @discardableResult
  func sync(saveScheme: (ChineseInputScheme) -> Bool = { InputSchemePreference.select($0) },
            saveGloss: (Bool) -> Bool = { enabled in
              InputHabitPreference.update { $0.glossEnabled = enabled } != nil
            }) -> Bool {
    if let selected = scheme, saveScheme(selected), scheme == selected {
      defaults.removeObject(forKey: schemeKey)
    }
    if let selected = gloss, saveGloss(selected), gloss == selected {
      defaults.removeObject(forKey: glossKey)
    }
    return !hasPending
  }
}
