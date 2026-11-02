import ObjectiveC
import UIKit
import XCTest

/// 正式渲染器通过代理写入原生 UITextView；标记、选区和 Unicode 行为由 UIKit 执行。
@MainActor
final class QuickPhraseCursorKeyboardTests: XCTestCase {
  private final class Document: NSObject, UITextDocumentProxy {
    var editor = UITextView()
    var documentIdentifier = UUID()
    var documentInputMode: UITextInputMode? { editor.textInputMode }
    var hasText: Bool { editor.hasText }
    var documentContextBeforeInput: String? {
      (editor.text as NSString).substring(to: editor.selectedRange.location)
    }
    var documentContextAfterInput: String? {
      (editor.text as NSString).substring(from: NSMaxRange(editor.selectedRange))
    }
    var selectedText: String? {
      editor.selectedRange.length == 0 ? nil : (editor.text as NSString).substring(with: editor.selectedRange)
    }
    func insertText(_ text: String) { editor.insertText(text) }
    func deleteBackward() { editor.deleteBackward() }
    func setMarkedText(_ text: String, selectedRange: NSRange) { editor.setMarkedText(text, selectedRange: selectedRange) }
    func unmarkText() { editor.unmarkText() }
    func adjustTextPosition(byCharacterOffset offset: Int) {
      XCTFail("短语定位不能通过方向移动猜测字符单位")
    }
  }

  private func keyboard(_ document: Document, traditional: Bool = false) throws -> KeyboardViewController {
    enableAllInputSchemes()
    let scheme = InputSchemePreference.scheme
    let previousTraditional = ChineseOutputPreference.usesTraditional
    let previousTraditionalDocument = MetasequoiaInputSessionBridge.loadSharedPreferences()?[ChineseOutputPreference.documentKey]
    addTeardownBlock {
      InputSchemePreference.scheme = scheme
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[ChineseOutputPreference.documentKey] = previousTraditionalDocument }
      ChineseOutputPreference.usesTraditional = previousTraditional
    }
    InputSchemePreference.scheme = .quanpin
    XCTAssertTrue(ChineseOutputPreference.save(traditional))
    let keyboard = KeyboardViewController()
    keyboard.loadViewIfNeeded()
    let selector = #selector(getter: UIInputViewController.textDocumentProxy)
    let inherited = try XCTUnwrap(class_getInstanceMethod(UIInputViewController.self, selector))
    let original = method_getImplementation(inherited)
    let block: @convention(block) (AnyObject) -> AnyObject = { _ in document }
    let replacement = imp_implementationWithBlock(block)
    if !class_addMethod(KeyboardViewController.self, selector, replacement, method_getTypeEncoding(inherited)) {
      method_setImplementation(try XCTUnwrap(class_getInstanceMethod(KeyboardViewController.self, selector)), replacement)
    }
    addTeardownBlock {
      if let own = class_getInstanceMethod(KeyboardViewController.self, selector) { method_setImplementation(own, original) }
      imp_removeBlock(replacement)
    }
    return keyboard
  }

  private func editor(_ document: Document, selecting: Bool) async throws -> UIWindow {
    let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    let window = UIWindow(windowScene: scene)
    window.rootViewController = UIViewController()
    window.makeKeyAndVisible()
    document.editor = UITextView(frame: CGRect(x: 0, y: 50, width: 300, height: 160))
    window.rootViewController?.view.addSubview(document.editor)
    document.editor.autocorrectionType = .no
    document.editor.becomeFirstResponder()
    document.editor.text = selecting ? "甲替换丙" : "甲丙"
    document.editor.selectedRange = NSRange(location: 1, length: selecting ? 2 : 0)
    // 先让编辑器应用初始选区；待测的旧组字、短语提交和后续组字可以在同一事件里连续发生。
    try await Task.sleep(for: .milliseconds(50))
    return window
  }

  func testCommitReplacesSelectionOrCompositionAndImmediatelyContinuesBeforeUnicodeSuffix() async throws {
    let document = Document()
    let keyboard = try keyboard(document)
    for selecting in [false, true] {
      for marked in [false, true] {
        for sameEvent in [false, true] {
          for suffix in ["\"", "🌲e\u{301}\"", "אב"] {
            let window = try await editor(document, selecting: selecting)
            defer { window.isHidden = true }
            if marked {
              keyboard.render(.init(isHandled: true, preedit: "Kt", localMode: "K", editingText: "Kt"))
              XCTAssertNotNil(document.editor.markedTextRange)
              if !sameEvent { try await Task.sleep(for: .milliseconds(50)) }
            }
            let text = "title:\"" + suffix
            keyboard.render(.init(isHandled: true, commitText: text, commitCursorSuffix: suffix))
            XCTAssertEqual(document.editor.text, "甲" + text + "丙")
            XCTAssertEqual(document.editor.selectedRange, NSRange(location: 8, length: 0))
            XCTAssertNil(document.editor.markedTextRange)
            keyboard.render(.init(isHandled: true, preedit: "n", editingText: "n"))
            XCTAssertEqual(document.editor.text, "甲title:\"n" + suffix + "丙")
            XCTAssertEqual(document.editor.selectedRange, NSRange(location: 9, length: 0))
            XCTAssertNotNil(document.editor.markedTextRange)
            keyboard.render(.init(isHandled: true, commitText: "你"))
            XCTAssertEqual(document.editor.text, "甲title:\"你" + suffix + "丙")
            XCTAssertNil(document.editor.markedTextRange)
          }
        }
      }
    }
  }

  func testAbsentEmptyAndInvalidSuffixKeepTheCompleteCommitAtItsEnd() async throws {
    let document = Document()
    let keyboard = try keyboard(document)
    for suffix: String? in [nil, "", "other", "é\""] {
      let window = try await editor(document, selecting: true)
      defer { window.isHidden = true }
      // Swift 字符串会把预组合和分解形式视为相等，定位仍须按实际 UTF-16 后缀核对。
      let text = "title:\"e\u{301}\""
      keyboard.render(.init(isHandled: true, preedit: "Kt", localMode: "K", editingText: "Kt"))
      keyboard.render(.init(isHandled: true, commitText: text, commitCursorSuffix: suffix))
      XCTAssertEqual(document.editor.text, "甲" + text + "丙")
      XCTAssertEqual(document.editor.selectedRange, NSRange(location: 1 + text.utf16.count, length: 0))
      XCTAssertNil(document.editor.markedTextRange)
    }
  }

  func testCommitAndNewCompositionInOneSnapshotApplyTheCursorFirst() async throws {
    let document = Document()
    let keyboard = try keyboard(document)
    let window = try await editor(document, selecting: true)
    defer { window.isHidden = true }
    keyboard.render(.init(isHandled: true, preedit: "Kt", localMode: "K", editingText: "Kt"))
    keyboard.render(.init(isHandled: true, commitText: "title:\"🌲\"", commitCursorSuffix: "🌲\"",
                          preedit: "n", editingText: "n"))
    XCTAssertEqual(document.editor.text, "甲title:\"n🌲\"丙")
    XCTAssertEqual(document.editor.selectedRange, NSRange(location: 9, length: 0))
    XCTAssertNotNil(document.editor.markedTextRange)
  }

  func testChineseConversionTransformsSuffixWithCommitAndRespectsConversionBoundary() async throws {
    let document = Document()
    let keyboard = try keyboard(document, traditional: true)
    for (conversion, source, expected) in [(true, TypingSource.local, "漢字"),
                                            (false, .local, "汉字"), (true, .japanese, "汉字")] {
      let window = try await editor(document, selecting: false)
      defer { window.isHidden = true }
      keyboard.render(.init(isHandled: true, commitText: "title:\"汉字\"", commitCursorSuffix: "汉字\"",
                            scriptConversion: conversion), source: source)
      XCTAssertEqual(document.editor.text, "甲title:\"" + expected + "\"丙")
      XCTAssertEqual(document.editor.selectedRange, NSRange(location: 8, length: 0))
      XCTAssertNil(document.editor.markedTextRange)
    }
  }

  func testRealEngineQuickPhraseReachesRendererAndNativeEditor() async throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    var draft = PluginEditorModel.Draft(kind: .phraseTable)
    draft.name = "合成光标短语"
    draft.phrases = [.init(key: "tt", text: "title:\"${cursor}🌲e\u{301}\"")]
    let saved = MetasequoiaInputSessionBridge.plugins(
      ["operation": "authoring_save", "pack": PluginEditorModel.pack(from: draft), "replace": false], stateRoot: state)
    guard case .success(let value) = saved, let pack = PluginPackage(json: value) else {
      return XCTFail("隔离短语表保存失败")
    }
    MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences["plugins"] = ["phrase_tables": [pack.id]]
    }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed)
    _ = bridge.setLearningEnabled(false)
    _ = bridge.setPrivateSession(true)
    let document = Document()
    let keyboard = try keyboard(document)
    let window = try await editor(document, selecting: true)
    defer { window.isHidden = true }
    keyboard.render(bridge.handleCharacter("K", shifted: true))
    keyboard.render(bridge.handleCharacter("t"))
    let candidates = bridge.handleCharacter("t")
    keyboard.render(candidates)
    let index = try XCTUnwrap(candidates.candidates.firstIndex(of: "title:\"🌲e\u{301}\""))
    XCTAssertEqual(candidates.candidateSources[index], 5)
    let commit = bridge.selectCandidate(at: UInt(index))
    XCTAssertEqual(commit.commitText, "title:\"🌲e\u{301}\"")
    XCTAssertEqual(commit.commitCursorSuffix, "🌲e\u{301}\"")
    keyboard.render(commit)
    XCTAssertEqual(document.editor.text, "甲title:\"🌲e\u{301}\"丙")
    XCTAssertEqual(document.editor.selectedRange, NSRange(location: 8, length: 0))
    keyboard.render(bridge.handleCharacter("n"))
    XCTAssertEqual(document.editor.text, "甲title:\"n🌲e\u{301}\"丙")
  }
}
