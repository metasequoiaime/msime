import UIKit
import XCTest

/// What one keystroke costs inside the extension, key by key.
///
/// The shared runtime answers in well under a millisecond, so anything a person feels as the
/// keyboard falling behind is spent above it — in the view work each key triggers. This measures
/// the controller's own path rather than a mean over a word: dropped frames come from the tail,
/// and a mean hides it behind the many cheap keys around it.
@MainActor
final class KeystrokeLatencyTests: XCTestCase {
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  private func controller(_ scheme: ChineseInputScheme, width: CGFloat = 390) -> KeyboardViewController {
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  /// The bridge on its own: one C ABI round trip and the parse of what comes back, with no view
  /// work at all. What is left after this is what the keyboard itself costs.
  func testWhatTheBridgeAloneCosts() {
    let bridge = MetasequoiaInputSessionBridge()
    var samples: [Double] = []
    for _ in 0..<50 {
      _ = bridge.cancel()
      for letter in "nihao" {
        let started = Date()
        _ = bridge.handleCharacter(String(letter))
        samples.append(Date().timeIntervalSince(started) * 1000)
      }
    }
    report("bridge-character", samples)
  }

  /// A query with a great many candidates, which is the common case people type into.
  ///
  /// `yi` answers with hundreds. Anything on the keystroke path that walks the whole answer rather
  /// than the nine on screen costs in proportion to it, and those are exactly the syllables a fast
  /// typist hits most.
  func testWhatACrowdedQueryCosts() throws {
    let previous = InputSchemePreference.scheme
    // Glosses ship on. Other cases in this suite turn them off and the App Group keeps that, so
    // the default has to be restored here or this measures a configuration nobody runs.
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.scheme = previous
      CandidateGlossPreference.enabled = previousGloss
    }
    CandidateGlossPreference.enabled = true
    let controller = self.controller(.quanpin)

    var samples: [Double] = []
    for _ in 0..<40 {
      for label in ["字母 Y", "字母 I"] {
        let button = try key(label, in: controller)
        let started = Date()
        button.sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
        samples.append(Date().timeIntervalSince(started) * 1000)
      }
      let space = try key("空格", in: controller)
      space.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
    }
    report("crowded-query", samples)

    // And the call the gloss scheduler makes on every keystroke, on its own.
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.cancel()
    _ = bridge.handleCharacter("y")
    _ = bridge.handleCharacter("i")
    var all: [Double] = []
    for _ in 0..<80 {
      let started = Date()
      _ = try? bridge.allCandidates()
      all.append(Date().timeIntervalSince(started) * 1000)
    }
    report("allCandidates(yi)", all)
    print("GLOSS resources=\(bridge.candidateGlossResources() ?? "nil") enabled=\(CandidateGlossPreference.enabled)")
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  private func key(_ label: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityLabel == label } as? UIButton,
      "no key labelled \(label)")
  }

  private func report(_ label: String, _ samples: [Double]) {
    let sorted = samples.sorted()
    let at = { (q: Double) in sorted[Int((Double(sorted.count - 1) * q).rounded())] }
    let mean = samples.reduce(0, +) / Double(samples.count)
    print(String(
      format: "LATENCY %@ n=%d mean=%.2fms p50=%.2fms p95=%.2fms max=%.2fms",
      label, samples.count, mean, at(0.5), at(0.95), at(1.0)))
  }

  func testWhatOneKeystrokeCosts() throws {
    let previous = InputSchemePreference.scheme
    // Glosses ship on, and other cases in this suite turn them off into the App Group where the
    // setting survives the run. Measuring with them off measures a configuration nobody has - it
    // is how the gloss path's cost stayed invisible here for as long as it did.
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.scheme = previous
      CandidateGlossPreference.enabled = previousGloss
    }
    CandidateGlossPreference.enabled = true

    // 九键按「nihao」的数字 64426 打，量的是共享引擎逐键刷新九键拼写之后的整条按键路径。
    for scheme in [ChineseInputScheme.quanpin, .nineKey] {
      let controller = self.controller(scheme)
      let labels = scheme == .nineKey
        ? ["6 MNO", "4 GHI", "4 GHI", "2 ABC", "6 MNO"]
        : ["字母 N", "字母 I", "字母 H", "字母 A", "字母 O"]
      var letters: [Double] = []
      var spaces: [Double] = []
      for _ in 0..<50 {
        for label in labels {
          let button = try key(label, in: controller)
          let started = Date()
          button.sendActions(for: .primaryActionTriggered)
          controller.view.layoutIfNeeded()
          letters.append(Date().timeIntervalSince(started) * 1000)
        }
        let space = try key("空格", in: controller)
        let started = Date()
        space.sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
        spaces.append(Date().timeIntervalSince(started) * 1000)
      }
      report("\(scheme.rawValue)-letter", letters)
      report("\(scheme.rawValue)-space", spaces)
    }
  }
}
