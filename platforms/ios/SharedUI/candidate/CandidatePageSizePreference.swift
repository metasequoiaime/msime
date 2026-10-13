import Foundation

/// How many candidates the strip numbers before the rest move to the expanded panel, the desktop's `candidate_page_size`.
///
/// The shared document's value (6 by default) is laid out for a desktop candidate window and cannot tell a chosen 6 from the default one, so iOS keeps its own in the App Group, starting at 9 so the strip's numbers cover every digit on the symbol layer and the iPad number row. The bridge lays it over the document as it does cloud candidates, and the strip reads the value the session was given, so the digits and the numbers never disagree. Never written to the shared document: a desktop keeps its own page size.
enum CandidatePageSizePreference {
  static let key = "candidate.pageSize"
  static let defaultSize = 9
  static let range = 1...9

  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }

  static var size: Int {
    get { clamped(defaults.object(forKey: key) as? Int) }
    set { defaults.set(clamped(newValue), forKey: key) }
  }

  /// A stored or received value inside 1–9; anything else is the default.
  static func clamped(_ value: Int?) -> Int {
    guard let value, range.contains(value) else { return defaultSize }
    return value
  }
}
