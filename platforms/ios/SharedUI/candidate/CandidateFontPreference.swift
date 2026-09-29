import UIKit

/// Candidate and composition text size, and the candidates' font family chain, read from the shared preference document.
///
/// The document stores points as the desktop candidate window uses them, 18 for candidates and 15 for the composition. iOS draws with Dynamic Type instead, so those defaults stand for the `.body` and `.subheadline` styles the keyboard already used, and any other value scales them by the same ratio; the user's system text size keeps applying on top. A phone strip has far less width than an iPad one, so a size synced from a desktop is clamped to what that surface can show.
enum CandidateFontPreference {
  static let candidateKey = "candidate_font_size"
  static let preeditKey = "candidate_preedit_font_size"
  static let defaultCandidateSize = 18
  static let defaultPreeditSize = 15
  static let familyKey = "candidate_font_family"
  static let englishFamilyKey = "candidate_english_font"
  static let fallbackFamiliesKey = "candidate_fallback_fonts"
  /// client-core's default family. iOS does not ship it, so a document nobody changed draws in the system font as before.
  static let defaultFamily = "Noto Sans SC"

  static func candidateRange(tablet: Bool) -> ClosedRange<Int> { tablet ? 12...32 : 12...24 }
  static func preeditRange(tablet: Bool) -> ClosedRange<Int> { tablet ? 12...24 : 12...20 }

  static func candidateSize(in preferences: [String: Any]?, tablet: Bool) -> Int {
    size(preferences?[candidateKey], default: defaultCandidateSize, range: candidateRange(tablet: tablet))
  }

  static func preeditSize(in preferences: [String: Any]?, tablet: Bool) -> Int {
    size(preferences?[preeditKey], default: defaultPreeditSize, range: preeditRange(tablet: tablet))
  }

  /// How much larger than `.body` the candidates are drawn.
  static func candidateScale(in preferences: [String: Any]?, tablet: Bool) -> CGFloat {
    CGFloat(candidateSize(in: preferences, tablet: tablet)) / CGFloat(defaultCandidateSize)
  }

  /// How much larger than `.subheadline` the composition is drawn.
  static func preeditScale(in preferences: [String: Any]?, tablet: Bool) -> CGFloat {
    CGFloat(preeditSize(in: preferences, tablet: tablet)) / CGFloat(defaultPreeditSize)
  }

  /// The text style scaled at the default text size, then handed to `UIFontMetrics` so the system text size still scales it and a label that adjusts for content size keeps doing so.
  static func font(_ style: UIFont.TextStyle, scale: CGFloat) -> UIFont {
    guard scale != 1 else { return .preferredFont(forTextStyle: style) }
    let base = UIFont.preferredFont(
      forTextStyle: style, compatibleWith: UITraitCollection(preferredContentSizeCategory: .large))
    return UIFontMetrics(forTextStyle: style).scaledFont(for: base.withSize((base.pointSize * scale).rounded()))
  }

  /// The families the candidates are drawn in, in order: the Latin face, the family, then its fallbacks, as the desktop candidate window chains them. Only families this device has are kept, since a document synced from Windows names faces an iPhone has never heard of; an empty chain means the system font.
  static func families(in preferences: [String: Any]?, installed: (String) -> Bool = isInstalled) -> [String] {
    let named = [preferences?[englishFamilyKey] as? String, preferences?[familyKey] as? String ?? defaultFamily]
      + ((preferences?[fallbackFamiliesKey] as? [Any]) ?? []).map { $0 as? String }
    var seen = Set<String>()
    return named.compactMap { $0 }.filter { !$0.isEmpty && installed($0) && seen.insert($0).inserted }
  }

  /// client-core's default supplementary chain, for a document that has never stored one.
  static let defaultFallbackFamilies = ["Noto Sans SC", "Microsoft YaHei"]
  /// The shared validator refuses more, matching the 32 ordered families of the Windows appearance page.
  static let maximumFallbackFamilies = 32

  /// The supplementary families as stored, in order, including ones this device lacks, so the settings page edits what the document says.
  static func fallbackFamilies(in preferences: [String: Any]?) -> [String] {
    guard let stored = preferences?[fallbackFamiliesKey] as? [Any] else { return defaultFallbackFamilies }
    return stored.compactMap { $0 as? String }
  }

  /// Adds a family at the end of the chain, once, within the shared limit.
  static func appending(_ family: String, to families: [String]) -> [String] {
    guard !family.isEmpty, !families.contains(family), families.count < maximumFallbackFamilies else { return families }
    return families + [family]
  }

  static func isInstalled(_ family: String) -> Bool { !UIFont.fontNames(forFamilyName: family).isEmpty }

  /// `font(_:scale:)` drawn in `families`: the first face leads and the rest cascade, so a Latin face takes the letters and a CJK face the Han characters, and whatever none of them has comes from the system font.
  static func font(_ style: UIFont.TextStyle, scale: CGFloat, families: [String]) -> UIFont {
    guard let first = families.first else { return font(style, scale: scale) }
    let base = UIFont.preferredFont(
      forTextStyle: style, compatibleWith: UITraitCollection(preferredContentSizeCategory: .large))
    let descriptor = UIFontDescriptor(fontAttributes: [
      .family: first,
      .cascadeList: families.dropFirst().map { UIFontDescriptor(fontAttributes: [.family: $0]) },
    ])
    return UIFontMetrics(forTextStyle: style).scaledFont(
      for: UIFont(descriptor: descriptor, size: (base.pointSize * scale).rounded()))
  }

  private static func size(_ value: Any?, default fallback: Int, range: ClosedRange<Int>) -> Int {
    guard let number = value as? NSNumber else { return fallback }
    return SharedNumber.clamped(number.intValue, to: range)
  }
}
