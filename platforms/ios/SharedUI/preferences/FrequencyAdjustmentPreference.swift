import Foundation
import CoreFoundation

/// The shared document's `frequency.mode`, in the Windows order; `disabled` keeps the dictionary's own order while learning still records new words.
enum FrequencyAdjustmentMode: String, CaseIterable {
  case disabled, pin, halve, linear, promote

  var title: String {
    switch self {
    case .disabled: "不调频"
    case .pin: "一次置顶"
    case .halve: "折半调频"
    case .linear: "线性调频"
    case .promote: "一次置前"
    }
  }
}

enum FrequencyAdjustmentPreference {
  static let modeKey = "frequencyAdjustmentMode"
  static let triggerCountKey = "frequencyAdjustmentTriggerCount"
  static let linearStepKey = "frequencyAdjustmentLinearStep"
  /// The shared document's range for `trigger_count` and `linear_step`, the same 1-10 Windows validates.
  static let countRange = 1...10

  static func resolvedMode(_ stored: String?) -> FrequencyAdjustmentMode {
    FrequencyAdjustmentMode(rawValue: stored ?? "") ?? .promote
  }

  static func resolvedCount(_ stored: Any?) -> Int {
    let value: Int?
    if let number = stored as? Int {
      value = number
    } else if let number = stored as? NSNumber {
      value = integer(number)
    } else {
      value = nil
    }
    guard let value, countRange.contains(value) else { return 1 }
    return value
  }

  private static func integer(_ value: NSNumber) -> Int? {
    guard CFGetTypeID(value) != CFBooleanGetTypeID(),
          let integer = Int(value.stringValue),
          NSNumber(value: integer).compare(value) == .orderedSame else { return nil }
    return integer
  }

  static var mode: FrequencyAdjustmentMode {
    get { resolvedMode(defaults.string(forKey: modeKey)) }
    set { defaults.set(newValue.rawValue, forKey: modeKey) }
  }

  static var triggerCount: Int {
    get { resolvedCount(defaults.object(forKey: triggerCountKey)) }
    set { defaults.set(resolvedCount(newValue), forKey: triggerCountKey) }
  }

  static var linearStep: Int {
    get { resolvedCount(defaults.object(forKey: linearStepKey)) }
    set { defaults.set(resolvedCount(newValue), forKey: linearStepKey) }
  }

  private static var defaults: UserDefaults {
    UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard
  }
}
