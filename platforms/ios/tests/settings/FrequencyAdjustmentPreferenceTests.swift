import Foundation

@main
enum FrequencyAdjustmentPreferenceTests {
  static func main() {
    precondition(FrequencyAdjustmentPreference.resolvedMode(nil) == .promote)
    precondition(FrequencyAdjustmentPreference.resolvedMode("pin") == .pin)
    precondition(FrequencyAdjustmentPreference.resolvedMode("halve") == .halve)
    precondition(FrequencyAdjustmentPreference.resolvedMode("linear") == .linear)
    precondition(FrequencyAdjustmentPreference.resolvedMode("promote") == .promote)
    precondition(FrequencyAdjustmentPreference.resolvedMode("disabled") == .disabled)
    precondition(FrequencyAdjustmentPreference.resolvedMode("unknown") == .promote)
    precondition(FrequencyAdjustmentPreference.resolvedCount(nil) == 1)
    precondition(FrequencyAdjustmentPreference.resolvedCount(3) == 3)
    precondition(FrequencyAdjustmentPreference.resolvedCount(1) == 1)
    precondition(FrequencyAdjustmentPreference.resolvedCount(6) == 6)
    precondition(FrequencyAdjustmentPreference.resolvedCount(0) == 1)
    precondition(FrequencyAdjustmentPreference.resolvedCount(10) == 10)
    precondition(FrequencyAdjustmentPreference.resolvedCount(11) == 1)
    precondition(FrequencyAdjustmentPreference.resolvedCount(NSNumber(value: 4)) == 4)
    precondition(FrequencyAdjustmentPreference.resolvedCount(NSNumber(value: 2.5)) == 1)
    precondition(FrequencyAdjustmentPreference.resolvedCount(NSNumber(value: true)) == 1)
    precondition(FrequencyAdjustmentMode.pin.title == "一次置顶")
    precondition(FrequencyAdjustmentMode.promote.title == "一次置前")
    precondition(FrequencyAdjustmentMode.disabled.title == "不调频")
  }
}
