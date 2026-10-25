import XCTest

@MainActor
final class MLKitStatsOptOutTests: XCTestCase {
  // The digitalInk key is the one the keyboard extension looked up on a device on every recognition (iPhone XR, iOS 18.7.10); the format and the other module names come from MLKitCommon 14.0.0.
  private let expectedKeys = [
    "com.google.firebase.ml.digitalInk.__MLKIT_DEFAULT.isStatsCollectionEnabled",
    "com.google.firebase.ml.mddDownloader.__MLKIT_DEFAULT.isStatsCollectionEnabled",
    "com.google.firebase.ml.commonDownloader.__MLKIT_DEFAULT.isStatsCollectionEnabled",
    "com.google.firebase.ml.vision.__MLKIT_DEFAULT.isStatsCollectionEnabled",
  ]

  override func tearDown() {
    for key in expectedKeys { UserDefaults.standard.removeObject(forKey: key) }
    super.tearDown()
  }

  func testApplyWritesFalseForEveryStatsKeyInTheProcessDefaults() {
    XCTAssertEqual(MLKitStatsOptOut.keys, expectedKeys)
    for key in expectedKeys { UserDefaults.standard.removeObject(forKey: key) }
    // That HandwritingRecognizer calls this before its first ML Kit call is checked by ProjectConfigurationTests.py: the recognizer is device-only and no test target links ML Kit.
    MLKitStatsOptOut.apply()
    for key in expectedKeys {
      XCTAssertEqual(UserDefaults.standard.object(forKey: key) as? Bool, false, key)
    }
  }
}
