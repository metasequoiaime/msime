import Foundation

/// Controls the offline annotation shown below iOS candidates.
///
/// The keyboard and its containing app share this value through the App Group. It is deliberately
/// Enabled by default now that glosses occupy their own row, matching the macOS candidate surface.
enum CandidateGlossPreference {
  static let key = "candidate.englishGloss"

  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }

  static var enabled: Bool {
    get { defaults.object(forKey: key) as? Bool ?? true }
    set { defaults.set(newValue, forKey: key) }
  }
}
