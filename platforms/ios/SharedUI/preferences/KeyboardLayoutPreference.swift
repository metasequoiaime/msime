import Foundation

enum KeyboardLayoutPreference {
  static var defaults: UserDefaults { UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard }
  static let keySpacingKey = "keyboard.spacing.keys"
  static let rowSpacingKey = "keyboard.spacing.rows"
  static let voiceShortcutKey = "keyboard.shortcut.voice"
  static let heightAdjustmentKey = "keyboard.height.adjustment"
  static let tabletFullKeysKey = "keyboard.tablet.fullKeys"
  static var keySpacing: Double {
    get { spacing(key: keySpacingKey, fallback: 6, range: 3...6) }
    set { defaults.set(min(6, max(3, newValue)), forKey: keySpacingKey) }
  }
  static var rowSpacing: Double {
    get { spacing(key: rowSpacingKey, fallback: 7, range: 4...10) }
    set { defaults.set(min(10, max(4, newValue)), forKey: rowSpacingKey) }
  }
  /// Keyboard height adjustment relative to the platform's current default, in points.
  ///
  /// The extension adds this value after accounting for orientation and candidate rows, so the
  /// same setting remains useful when the candidate surface changes height.
  static var heightAdjustment: Double {
    get { spacing(key: heightAdjustmentKey, fallback: 0, range: -12...48) }
    set { defaults.set(min(48, max(-12, newValue)), forKey: heightAdjustmentKey) }
  }
  /// Whether Tab opens the full candidate panel while composing on the full-size iPad keyboard: the shared `navigation.tab` (Windows `paging_tab`, on by default). Read by the keyboard and edited by the App's iPad section.
  static func tabShowsMoreCandidates(_ preferences: [String: Any]?) -> Bool {
    (preferences?["navigation"] as? [String: Any])?["tab"] as? Bool ?? true
  }

  /// Writes `navigation.tab` into the shared document, keeping the other paging keys in the same object.
  static func saveTabShowsMoreCandidates(_ enabled: Bool, stateRoot: URL? = nil) -> Bool {
    MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { document in
      var navigation = document["navigation"] as? [String: Any] ?? [:]
      navigation["tab"] = enabled
      document["navigation"] = navigation
    }
  }

  static func resetToDefaults() {
    for stored in [keySpacingKey, rowSpacingKey, heightAdjustmentKey, voiceShortcutKey] {
      defaults.removeObject(forKey: stored)
    }
  }
  /// Save the geometry where the keyboard reads it. The keyboard copies the shared document's `touch_*` fields over the App Group every time it appears, so a value written only to the App Group lasted until then. The App Group is written after the document, and only when the document took the change.
  @discardableResult
  static func saveGeometry(keySpacing: Double, rowSpacing: Double, heightAdjustment: Double,
                           voiceShortcut: Bool, stateRoot: URL? = nil) -> Bool {
    guard let mapping = MetasequoiaInputSessionBridge.geometryMapping(
      keySpacing: keySpacing, rowSpacing: rowSpacing, heightAdjustment: heightAdjustment, voiceEnabled: voiceShortcut),
      MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, mapping) else { return false }
    self.keySpacing = keySpacing
    self.rowSpacing = rowSpacing
    self.heightAdjustment = heightAdjustment
    voiceShortcutEnabled = voiceShortcut
    return true
  }

  /// `resetToDefaults`, for the shared document as well.
  @discardableResult
  static func resetGeometry(stateRoot: URL? = nil) -> Bool {
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { preferences in
      for key in ["touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment",
                  "touch_voice_shortcut"] {
        preferences.removeValue(forKey: key)
      }
    }
    guard written else { return false }
    resetToDefaults()
    return true
  }
  static var voiceShortcutEnabled: Bool {
    get { defaults.bool(forKey: voiceShortcutKey) }
    set { defaults.set(newValue, forKey: voiceShortcutKey) }
  }
  /// 「数字行与 Tab 键」: the full-size iPad keyboard carries a digit row above the letters and a Tab key before Q, as a desktop keyboard does. On by default; phones and the compact iPad keyboards never show them.
  static var tabletFullKeys: Bool {
    get { defaults.object(forKey: tabletFullKeysKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: tabletFullKeysKey) }
  }
  static var geometry: KeyboardGeometry { KeyboardGeometry(keySpacing: keySpacing, rowSpacing: rowSpacing) }
  private static func spacing(key: String, fallback: Double, range: ClosedRange<Double>) -> Double {
    guard let value = defaults.object(forKey: key) as? NSNumber, value.doubleValue.isFinite else { return fallback }
    return min(range.upperBound, max(range.lowerBound, value.doubleValue))
  }
}

/// 「全角输入」: the shared `character_width`, the width a keyboard session starts in.
///
/// The document spells it in lower case, `fullwidth` and `halfwidth` (client-core's serde rename); the runtime view spells it `Fullwidth`, so the keyboard never reads its width back from the view. As on the other hosts, the keyboard's own 全角 card switches the running keyboard only, and a reloaded document replaces that switch only when `character_width` itself changed, so saving any other setting never clears a switch the user just pressed.
enum CharacterWidthPreference {
  static let key = "character_width"
  static let fullwidth = "fullwidth"
  static let halfwidth = "halfwidth"

  static func value(in preferences: [String: Any]?) -> String? { preferences?[key] as? String }

  /// The width a keyboard starts in: the document's.
  static func startsFullwidth(in preferences: [String: Any]?) -> Bool {
    value(in: preferences) == fullwidth
  }

  /// Whether a reloaded document's width replaces the keyboard's current switch.
  static func overridesToggle(previous: String?, next: String?) -> Bool { next != nil && previous != next }
}

/// Converts the text the keyboard commits itself, past the runtime: symbols, spaces, quick punctuation and English letters. Whatever the runtime commits it has already converted once told the width (`setCharacterWidth`); handwriting, local tools and service results keep their original identity.
enum FullWidthInputPolicy {
  static func output(_ text: String, enabled: Bool) -> String {
    guard enabled, !text.isEmpty else { return text }
    var result = ""
    result.unicodeScalars.reserveCapacity(text.unicodeScalars.count)
    for scalar in text.unicodeScalars {
      let value = scalar.value
      if value == 0x20 {
        result.unicodeScalars.append("\u{3000}")
      } else if (0x21...0x7E).contains(value), let converted = UnicodeScalar(value + 0xFEE0) {
        result.unicodeScalars.append(converted)
      } else {
        result.unicodeScalars.append(scalar)
      }
    }
    return result
  }
}

struct KeyboardGeometry: Equatable {
  let keySpacing: Double
  let rowSpacing: Double
  var sidebarRatio: Double { 0.14 }
  var letterInsetRatio: Double { 0 }
  var centeredLetters: Bool { false }
  var showsBottomLanguage: Bool { true }
  var showsFullKeyboardSymbols: Bool { false }

  static func clamped(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
    min(upper, max(lower, value))
  }

  static func formattedHeightAdjustment(_ value: Double) -> String {
    value > 0 ? "+\(Int(value))" : "\(Int(value))"
  }
}
