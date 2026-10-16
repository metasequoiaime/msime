import UIKit
import XCTest

/// 键盘在内存上的两件事（#6685）：诊断日志各行带上的内存读数，以及内存警告不能让候选条和引擎的候选列表对不上、又要在组字结束后补清引擎缓存。
@MainActor
final class KeyboardMemoryTests: XCTestCase {
  private var savedScheme: ChineseInputScheme?

  override func setUp() {
    super.setUp()
    savedScheme = InputSchemePreference.scheme
  }

  override func tearDown() {
    if let savedScheme { InputSchemePreference.scheme = savedScheme }
    super.tearDown()
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  private func press(_ label: String, in controller: KeyboardViewController) throws {
    let key = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityLabel == label } as? UIButton, "no key labelled \(label)")
    key.sendActions(for: .primaryActionTriggered)
  }

  private func preedit(_ controller: KeyboardViewController) throws -> String? {
    let button = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "preeditButton" } as? UIButton)
    // 拼写写进输入框，候选栏不画它；读屏值仍是完整的组字。
    return button.accessibilityValue
  }

  /// 打开共享的诊断日志跑 `body`，返回这期间写下的各行；结束后恢复原来的开关并删掉日志。
  private func withSharedLog(_ body: () throws -> Void) throws -> [String] {
    guard let log = DiagnosticLog.url(in: MetasequoiaInputSessionBridge.sharedStateDirectory) else {
      throw XCTSkip("the test host has no App Group state directory")
    }
    let stored = MetasequoiaInputSessionBridge.loadSharedPreferences()?["diagnostic_log"]
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences {
      var diagnostic = $0["diagnostic_log"] as? [String: Any] ?? [:]
      diagnostic["server"] = true
      $0["diagnostic_log"] = diagnostic
    })
    try? FileManager.default.removeItem(at: log)
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0["diagnostic_log"] = stored }
      DiagnosticLog.shared.configure(directory: nil, enabled: false)
      try? FileManager.default.removeItem(at: log)
    }
    try body()
    return try String(contentsOf: log, encoding: .utf8).split(separator: "\n").map(String.init)
  }

  /// 日志里 `event` 最后一行的 `key=value` 字段。
  private func fields(_ event: String, in lines: [String]) throws -> [String: String] {
    let line = try XCTUnwrap(lines.last { $0.contains("] \(event) ") }, "no \(event) line in \(lines)")
    var values: [String: String] = [:]
    for pair in line.split(separator: " ") {
      let parts = pair.split(separator: "=", maxSplits: 1)
      if parts.count == 2 { values[String(parts[0])] = String(parts[1]) }
    }
    return values
  }

  /// 内存读数的两个字段都在：`mem` 是整数，`avail` 是整数或表示系统没给出余量的 `-`。
  private func assertMemoryFields(_ values: [String: String], _ event: String, file: StaticString = #filePath, line: UInt = #line) {
    XCTAssertNotNil(values["mem"].flatMap { UInt64($0) }, "\(event) has no mem", file: file, line: line)
    let avail = values["avail"]
    XCTAssertTrue(avail == "-" || avail.flatMap { UInt64($0) } != nil, "\(event) has avail=\(avail ?? "nil")", file: file, line: line)
  }

  private func composingKeyboard() -> KeyboardViewController {
    enableAllInputSchemes()
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  /// 候选条上可见的词块，按序号排列。
  private func shownCandidates(_ controller: KeyboardViewController) -> [String] {
    descendants(controller.view)
      .compactMap { view -> (Int, String)? in
        guard let button = view as? UIButton, !button.isHidden,
              let identifier = button.accessibilityIdentifier, identifier.hasPrefix("candidate-"),
              let index = Int(identifier.dropFirst("candidate-".count)),
              let title = button.configuration?.title, !title.isEmpty else { return nil }
        return (index, title)
      }
      .sorted { $0.0 < $1.0 }
      .map(\.1)
  }

  // MARK: - 日志字段

  func testMemoryFieldsAreWholeMebibytes() {
    let sample = DiagnosticLog.MemorySample(footprint: 41 * 1024 * 1024 + 1023, available: 7 * 1024 * 1024)
    XCTAssertEqual(sample.fields, "mem=41 avail=7")
    // 系统没给出余量（模拟器、不算「App」的进程或已超限）写成 `-`；余量不足 1 MiB 仍是数字 0。
    XCTAssertEqual(DiagnosticLog.MemorySample(footprint: 0, available: 0).fields, "mem=0 avail=-")
    XCTAssertEqual(DiagnosticLog.MemorySample(footprint: 0, available: 1023).fields, "mem=0 avail=0")
    XCTAssertEqual(DiagnosticLog.MemorySample.mebibytes(1024 * 1024 - 1), 0)
  }

  func testCurrentSampleReadsTheFootprint() {
    // 模拟器上 `os_proc_available_memory` 是 0；键盘扩展在真机上能不能读到余量还没有实测，这里只查占用。
    XCTAssertGreaterThan(DiagnosticLog.MemorySample.current().footprint, 0)
  }

  /// 最长的那一行在数字最大时也不会被 192 字节截掉，并且仍是可打印 ASCII。
  func testTheLongestMemoryLineFitsTheRecord() {
    let huge = DiagnosticLog.MemorySample(footprint: .max, available: .max)
    let line = "keyboard_loaded full_access=1 idiom=phone base=\(DiagnosticLog.MemorySample.mebibytes(.max)) session=\(DiagnosticLog.MemorySample.mebibytes(.max)) \(huge.fields)"
    XCTAssertEqual(DiagnosticLog.sanitize(line), line)
    XCTAssertTrue(line.hasPrefix("keyboard_loaded full_access=1 idiom=phone "))
  }

  // MARK: - 内存警告与组字

  /// 点候选条上的第 `number` 个词块（从 1 数起）。
  private func tapCandidate(_ number: Int, in controller: KeyboardViewController) throws {
    let chip = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "candidate-\(number)" && !$0.isHidden } as? UIButton,
      "no candidate-\(number) on the strip")
    chip.sendActions(for: .primaryActionTriggered)
  }

  /// 组字中收到内存警告：引擎缓存不动，组字和候选条原样保留；选词上屏结束这次组字（推迟的清缓存在这时补上）后接着打字照常。这一条在组字中也清缓存的旧写法上同样通过，它守的是组字不被打断。
  func testAMemoryWarningMidCompositionKeepsTheComposition() throws {
    let controller = composingKeyboard()
    for letter in ["N", "I", "H", "A", "O"] { try press("字母 \(letter)", in: controller) }
    controller.view.layoutIfNeeded()
    let composing = try preedit(controller)
    let candidates = shownCandidates(controller)
    XCTAssertFalse(candidates.isEmpty)

    controller.didReceiveMemoryWarning()
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try preedit(controller), composing)
    XCTAssertEqual(shownCandidates(controller), candidates)

    try tapCandidate(1, in: controller)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(shownCandidates(controller), [], "selecting the whole word should end the composition")
    for letter in ["N", "I"] { try press("字母 \(letter)", in: controller) }
    controller.view.layoutIfNeeded()
    XCTAssertNotEqual(try preedit(controller), composing)
    XCTAssertFalse(shownCandidates(controller).isEmpty)
    controller.viewWillDisappear(false)
  }

  /// 没有组字时内存警告照常清引擎缓存，键盘随后正常组字。
  func testAMemoryWarningWhileIdleLeavesTheKeyboardUsable() throws {
    let controller = composingKeyboard()
    let idle = try preedit(controller)

    controller.didReceiveMemoryWarning()
    for letter in ["N", "I"] { try press("字母 \(letter)", in: controller) }
    controller.view.layoutIfNeeded()
    XCTAssertNotEqual(try preedit(controller), idle)
    XCTAssertFalse(shownCandidates(controller).isEmpty)
    controller.viewWillDisappear(false)
  }

  /// 日志写明每次警告引擎缓存是当场清了还是留到组字结束：空闲时 `engine_cache=reset`；组字中 `engine_cache=deferred`，上屏后补清并写 `engine_cache_reset`。前后几行都带内存读数。
  func testTheMemoryWarningLinesTellWhenTheEngineCacheWasReleased() throws {
    let lines = try withSharedLog {
      let controller = composingKeyboard()
      controller.didReceiveMemoryWarning()
      for letter in ["N", "I", "H", "A", "O"] { try press("字母 \(letter)", in: controller) }
      controller.view.layoutIfNeeded()
      XCTAssertFalse(shownCandidates(controller).isEmpty)
      controller.didReceiveMemoryWarning()
      try tapCandidate(1, in: controller)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(shownCandidates(controller), [], "selecting the whole word should end the composition")
      controller.viewWillDisappear(false)
    }
    let released = lines.filter { $0.contains("] memory_released ") }
    XCTAssertEqual(released.count, 2, "\(lines)")
    XCTAssertTrue(released.first?.contains(" engine_cache=reset ") == true, "\(released)")
    XCTAssertTrue(released.last?.contains(" engine_cache=deferred ") == true, "\(released)")
    let resets = lines.filter { $0.contains("] engine_cache_reset ") }
    XCTAssertEqual(resets.count, 1, "the deferred reset runs once, when the composition ends: \(lines)")
    if let reset = resets.first, let deferred = released.last {
      XCTAssertGreaterThan(lines.firstIndex(of: reset) ?? -1, lines.firstIndex(of: deferred) ?? .max)
    }
    for event in ["memory_warning", "memory_released", "engine_cache_reset"] {
      let values = try fields(event, in: lines)
      assertMemoryFields(values, event)
    }
  }

  // MARK: - 启动占用

  /// 按键盘自己写进诊断日志的读数，量出启动各阶段的 `phys_footprint`：引擎会话（`base` → `session`）、其余存储属性（→ `keyboard_loaded` 的 `mem`）、建界面（→ `keyboard_built`）、按屏幕宽度画出第一帧（→ `keyboard_shown`）。
  ///
  /// 模拟器的数字不是真机的数字：模拟器不设上限、`avail` 写成 `-`，内存页的计法也与设备不同，只用来看各阶段的比例和防止哪一段突然变大。单独跑这一条（`-only-testing`）得到的是冷进程的数；整套里跑时前面的用例已经映射过词库，引擎那一段会偏小。
  func testKeyboardLaunchFootprintIsLoggedPerStage() throws {
    let width = UIScreen.main.bounds.width
    let lines = try withSharedLog {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: width, height: 420))
      window.rootViewController = controller
      window.isHidden = false
      defer { window.isHidden = true }
      controller.viewWillAppear(false)
      window.layoutIfNeeded()
      // 提交一次 Core Animation 事务，让各视图真的画出位图，与系统显示键盘时一样。
      CATransaction.flush()
      controller.viewDidAppear(false)
      controller.viewWillDisappear(false)
    }
    let loaded = try fields("keyboard_loaded", in: lines)
    let built = try fields("keyboard_built", in: lines)
    let shown = try fields("keyboard_shown", in: lines)
    let focus = try fields("focus_in", in: lines)
    for (event, values) in [("keyboard_loaded", loaded), ("keyboard_built", built), ("keyboard_shown", shown), ("focus_in", focus)] {
      assertMemoryFields(values, event)
    }
    let base = try XCTUnwrap(loaded["base"].flatMap { UInt64($0) })
    let session = try XCTUnwrap(loaded["session"].flatMap { UInt64($0) })
    let afterLoad = try XCTUnwrap(loaded["mem"].flatMap { UInt64($0) })
    let afterBuild = try XCTUnwrap(built["mem"].flatMap { UInt64($0) })
    let afterShow = try XCTUnwrap(shown["mem"].flatMap { UInt64($0) })
    print("keyboard launch footprint (MiB, idiom=\(UIDevice.current.userInterfaceIdiom == .pad ? "pad" : "phone"), width=\(width)): base=\(base) session=\(session) loaded=\(afterLoad) built=\(afterBuild) shown=\(afterShow); engine=\(Int(session) - Int(base)) stored=\(Int(afterLoad) - Int(session)) ui=\(Int(afterBuild) - Int(afterLoad)) render=\(Int(afterShow) - Int(afterBuild))")
    XCTAssertGreaterThan(base, 0)
    XCTAssertLessThanOrEqual(base, session)
    // 2026-10-10 在 iOS 27.0 的 13 英寸 iPad Pro 模拟器上冷进程实测从 `base` 到 `keyboard_shown` 约 20 MiB；上限取它的三倍，只拦突然翻倍的那种回归。
    XCTAssertLessThan(afterShow - min(afterShow, base), 60,
      "the keyboard's launch grew past the regression budget; see the per-stage numbers above")
  }
}
