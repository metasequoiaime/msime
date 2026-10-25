import XCTest

final class OnboardingChoicesTests: XCTestCase {
  private func defaults() -> UserDefaults {
    let name = "msime.synthetic.onboarding.\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: name)!
    addTeardownBlock { defaults.removePersistentDomain(forName: name) }
    return defaults
  }

  func testFailedSaveSurvivesReopeningAndOnlySuccessfulFieldsAreCleared() {
    let storage = defaults()
    var choices = OnboardingChoices(defaults: storage)
    choices.remember(.nineKey)
    choices.remember(gloss: false)
    XCTAssertFalse(choices.sync(saveScheme: { _ in false }, saveGloss: { _ in true }))
    choices = OnboardingChoices(defaults: storage)
    XCTAssertEqual(choices.scheme, .nineKey)
    XCTAssertNil(choices.gloss)
    XCTAssertTrue(choices.sync(saveScheme: { XCTAssertEqual($0, .nineKey); return true }, saveGloss: { _ in
      XCTFail("已保存的字段不应再次写入")
      return false
    }))
    XCTAssertFalse(choices.hasPending)
  }

  func testNewChoiceDuringSaveRemainsPending() {
    let choices = OnboardingChoices(defaults: defaults())
    choices.remember(.nineKey)
    XCTAssertFalse(choices.sync(saveScheme: { _ in choices.remember(.quanpin); return true }, saveGloss: { _ in true }))
    XCTAssertEqual(choices.scheme, .quanpin)
  }

  func testFalseGlossIsARealPendingChoice() {
    let choices = OnboardingChoices(defaults: defaults())
    choices.remember(gloss: false)
    XCTAssertFalse(choices.sync(saveScheme: { _ in true }, saveGloss: { XCTAssertFalse($0); return false }))
    XCTAssertEqual(choices.gloss, false)
  }
}
