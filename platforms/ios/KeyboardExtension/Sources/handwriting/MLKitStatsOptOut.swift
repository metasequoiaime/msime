import Foundation

/// Turns off Google ML Kit's usage statistics for this process. An input method must not send undisclosed telemetry, and ML Kit treats a missing preference as consent: `-[MLKAnalyticsLogger isStatsCollectionEnabled]` (MLKitCommon 14.0.0) returns YES when `NSUserDefaults.standardUserDefaults` has no value, and `logEvent:withName:` records model checks, downloads and recognitions for upload to Google. The Clearcut transport that would upload them is not linked today, so nothing leaves the device either way; writing the opt-out keeps it that way if a pod update links it. Call this before any ML Kit API runs.
///
/// The key is `com.google.firebase.ml.<module>.<context>.isStatsCollectionEnabled`, where the context is always `__MLKIT_DEFAULT` without a Firebase app, and the modules are the logger types instantiated by the linked binaries: `digitalInk` (`+[MLKDigitalInkRecognizer initialize]`), `mddDownloader` (`-[MLKMDDLogger init]`), `commonDownloader` (`MLKModelDownloadLogger`) and `vision` (`MLKCustomModelManager`). See `.agents/notes/implemented/bug-fix/2026-10-25-ios-mlkit-stats-opt-out.md`.
enum MLKitStatsOptOut {
  static let keys = ["digitalInk", "mddDownloader", "commonDownloader", "vision"]
    .map { "com.google.firebase.ml.\($0).__MLKIT_DEFAULT.isStatsCollectionEnabled" }

  static func apply() {
    for key in keys { UserDefaults.standard.set(false, forKey: key) }
  }
}
