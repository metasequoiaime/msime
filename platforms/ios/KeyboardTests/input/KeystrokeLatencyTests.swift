import UIKit
import XCTest

/// 分别测量桥接与按键回调加布局，保留逐键尾延迟。
/// 测试宿主没有连接真实文档代理；系统键盘的跨进程输入另外验收。
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

  /// 九键长句的桥接与控制器对照；独立会话隔离词频和临时学习。
  func testNineKeyLongCompositionCosts() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: root)
    XCTAssertNil(bridge.switchToNineKey().diagnosticText)
    _ = bridge.setLearningEnabled(false)
    _ = bridge.setPrivateSession(true)
    let words = ["nihao", "zhongguo", "shurufa", "jintiantianqihenhao",
                 "womenyongshurufalianxudaziceshi", "zhonghuarenmingongheguo"]
    let digitsByLetter = Dictionary(uniqueKeysWithValues:
      zip(["abc", "def", "ghi", "jkl", "mno", "pqrs", "tuv", "wxyz"], "23456789")
        .flatMap { letters, digit in letters.map { ($0, digit) } })
    var bridgeSamples: [Double] = []
    for round in 0..<3 {
      for (caseIndex, word) in words.enumerated() {
        _ = bridge.cancel()
        var samples: [Double] = []
        for digit in word.compactMap({ digitsByLetter[$0] }) {
          let start = CACurrentMediaTime()
          let snapshot = bridge.handleCharacter(String(digit))
          samples.append((CACurrentMediaTime() - start) * 1000)
          XCTAssertNil(snapshot.diagnosticText)
        }
        report("nineKey-bridge-round\(round)-case\(caseIndex)", samples)
        bridgeSamples += samples
      }
    }
    report("nineKey-long-bridge", bridgeSamples)

    let previous = InputSchemePreference.scheme
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.scheme = previous
      CandidateGlossPreference.enabled = previousGloss
    }
    CandidateGlossPreference.enabled = true
    let controller = self.controller(.nineKey)
    defer { controller.viewDidDisappear(false) }
    var controllerSamples: [Double] = []
    let buttons = Dictionary(uniqueKeysWithValues: try "23456789".map { digit in
      (digit, try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityIdentifier == "nineKey\(digit)"
      } as? UIButton))
    })
    let space = try key("空格", in: controller)
    let enter = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == "returnKey"
    })
    for round in 0..<3 {
      for (caseIndex, word) in words.enumerated() {
        var samples: [Double] = []
        for digit in word.compactMap({ digitsByLetter[$0] }) {
          let start = CACurrentMediaTime()
          buttons[digit]?.sendActions(for: .primaryActionTriggered)
          controller.view.layoutIfNeeded()
          samples.append((CACurrentMediaTime() - start) * 1000)
        }
        report("nineKey-controller-round\(round)-case\(caseIndex)", samples)
        controllerSamples += samples
        // 连续选词直至组字消耗完，下一句必须从空组字开始。
        XCTAssertEqual(enter.accessibilityLabel, "确认", "必须确实进入九键组字")
        for _ in 0..<64 where enter.accessibilityLabel == "确认" {
          space.sendActions(for: .primaryActionTriggered)
          controller.view.layoutIfNeeded()
        }
        XCTAssertNotEqual(enter.accessibilityLabel, "确认", "下一句开始前必须消耗完组字")
      }
    }
    report("nineKey-long-controller", controllerSamples)
  }

  /// 异步路径分别记录逐键入队和整段查询加候选绘制，不将整段耗时当作单键延迟。
  func testAsyncNineKeyBurstCosts() async throws {
    let previous = InputSchemePreference.scheme
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.scheme = previous
      CandidateGlossPreference.enabled = previousGloss
    }
    CandidateGlossPreference.enabled = true
    let controller = self.controller(.nineKey)
    controller.asynchronousNineKeyInputEnabled = true
    defer { controller.viewWillDisappear(false) }
    let buttons = Dictionary(uniqueKeysWithValues: try "23456789".map { digit in
      (digit, try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityIdentifier == "nineKey\(digit)"
      } as? UIButton))
    })
    let space = try key("空格", in: controller)
    let enter = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == "returnKey"
    })
    let sequences = ["64426", "94664486", "7487832", "5468426842674436426"]
    for interval in [0.0, 0.04, 0.07] {
      var enqueueSamples: [Double] = []
      var settledSamples: [Double] = []
      var finalKeySamples: [Double] = []
      var lateSamples: [Double] = []
      for _ in 0..<3 {
        for digits in sequences {
          let started = CACurrentMediaTime()
          var finalKeyStarted = started
          for (index, digit) in digits.enumerated() {
            let scheduled = started + Double(index) * interval
            let remaining = scheduled - CACurrentMediaTime()
            if remaining > 0 {
              try await Task.sleep(nanoseconds: UInt64(remaining * 1_000_000_000))
            }
            let enqueueStarted = CACurrentMediaTime()
            lateSamples.append(max(0, enqueueStarted - scheduled) * 1000)
            finalKeyStarted = enqueueStarted
            buttons[digit]?.sendActions(for: .primaryActionTriggered)
            enqueueSamples.append((CACurrentMediaTime() - enqueueStarted) * 1000)
          }
          // 等最后一个数字完成查询与绘制；首键把回车改成「确认」不代表整段完成。
          let deadline = CACurrentMediaTime() + 5
          while controller.hasPendingNineKeyInput && CACurrentMediaTime() < deadline {
            try await Task.sleep(nanoseconds: 1_000_000)
          }
          guard !controller.hasPendingNineKeyInput else {
            XCTFail("异步九键未完成最后一键的候选绘制")
            return
          }
          controller.view.layoutIfNeeded()
          let settled = CACurrentMediaTime()
          settledSamples.append((settled - started) * 1000)
          finalKeySamples.append((settled - finalKeyStarted) * 1000)
          XCTAssertEqual(enter.accessibilityLabel, "确认")
          for _ in 0..<64 where enter.accessibilityLabel == "确认" {
            space.sendActions(for: .primaryActionTriggered)
            controller.view.layoutIfNeeded()
          }
          XCTAssertNotEqual(enter.accessibilityLabel, "确认")
          // 让旧回调再走一轮，确认提交后不会重新恢复中间帧。
          await Task.yield()
          XCTAssertNotEqual(enter.accessibilityLabel, "确认")
        }
      }
      let cadence = Int(interval * 1000)
      report("nineKey-async-\(cadence)ms-enqueue", enqueueSamples)
      report("nineKey-async-\(cadence)ms-whole-sequence", settledSamples)
      report("nineKey-async-\(cadence)ms-final-key", finalKeySamples)
      report("nineKey-async-\(cadence)ms-schedule-lateness", lateSamples)
    }
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
