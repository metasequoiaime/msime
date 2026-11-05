import ObjectiveC
import UIKit
import XCTest

/// 选词上屏替换行内预编辑时，宿主里只留正文（#10873）。
///
/// `FlutterHost` 按 Flutter 引擎 flutter/flutter#191062（2026-09）之前的 `FlutterTextInputView` 写：`insertText` 只替换当前选区并清掉标记区，`setMarkedText` 替换标记区（没有标记区时替换选区），`unmarkText` 只清掉标记区。闲鱼等 Flutter 应用的输入框就是这一套；`UITextView` 是遵守 UIKit 约定、`insertText` 自己替换标记区的宿主。
@MainActor
final class MarkedTextCommitHostTests: XCTestCase {
  private final class FlutterHost: NSObject, UITextDocumentProxy {
    let text = NSMutableString()
    var selected = NSRange(location: 0, length: 0)
    var marked: NSRange?
    var calls: [String] = []
    var documentIdentifier = UUID()
    var documentInputMode: UITextInputMode? { nil }
    var hasText: Bool { text.length > 0 }
    var documentContextBeforeInput: String? { text.substring(to: selected.location) }
    var documentContextAfterInput: String? { text.substring(from: NSMaxRange(selected)) }
    var selectedText: String? { selected.length == 0 ? nil : text.substring(with: selected) }

    private func clamp(_ range: NSRange) -> NSRange {
      let location = min(range.location, text.length)
      return NSRange(location: location, length: min(range.length, text.length - location))
    }

    func insertText(_ inserted: String) {
      calls.append("insertText")
      let range = clamp(selected)
      text.replaceCharacters(in: range, with: inserted)
      selected = clamp(NSRange(location: range.location + (inserted as NSString).length, length: 0))
      marked = nil
    }

    func setMarkedText(_ markedText: String, selectedRange: NSRange) {
      calls.append("setMarkedText")
      let replaced = marked.flatMap { $0.length > 0 ? $0 : nil } ?? selected
      text.replaceCharacters(in: replaced, with: markedText)
      let length = (markedText as NSString).length
      marked = length > 0 ? NSRange(location: replaced.location, length: length) : nil
      selected = clamp(NSRange(location: selectedRange.location + replaced.location, length: selectedRange.length))
    }

    func unmarkText() {
      calls.append("unmarkText")
      marked = nil
    }

    func deleteBackward() {
      calls.append("deleteBackward")
      if selected.length == 0, selected.location > 0 { selected = NSRange(location: selected.location - 1, length: 1) }
      text.replaceCharacters(in: selected, with: "")
      selected.length = 0
      marked = nil
    }

    func adjustTextPosition(byCharacterOffset offset: Int) {
      selected = clamp(NSRange(location: max(0, selected.location + offset), length: 0))
    }
  }

  private final class TextViewHost: NSObject, UITextDocumentProxy {
    var editor = UITextView()
    var documentIdentifier = UUID()
    var documentInputMode: UITextInputMode? { editor.textInputMode }
    var hasText: Bool { editor.hasText }
    var documentContextBeforeInput: String? { (editor.text as NSString).substring(to: editor.selectedRange.location) }
    var documentContextAfterInput: String? { (editor.text as NSString).substring(from: NSMaxRange(editor.selectedRange)) }
    var selectedText: String? {
      editor.selectedRange.length == 0 ? nil : (editor.text as NSString).substring(with: editor.selectedRange)
    }
    func insertText(_ text: String) { editor.insertText(text) }
    func deleteBackward() { editor.deleteBackward() }
    func setMarkedText(_ text: String, selectedRange: NSRange) { editor.setMarkedText(text, selectedRange: selectedRange) }
    func unmarkText() { editor.unmarkText() }
    func adjustTextPosition(byCharacterOffset offset: Int) {}
  }

  private func keyboard(_ document: UITextDocumentProxy) throws -> KeyboardViewController {
    enableAllInputSchemes()
    let scheme = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = scheme }
    InputSchemePreference.scheme = .quanpin
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
    keyboard.viewWillAppear(false)
    return keyboard
  }

  private func flutterHost(_ text: String, caret: Int) -> FlutterHost {
    let host = FlutterHost()
    host.text.setString(text)
    host.selected = NSRange(location: caret, length: 0)
    return host
  }

  func testCommitWritesTheMarkedSpellingAwayWhenTheCompositionEnds() {
    XCTAssertEqual(InlineCompositionPolicy.commit("你好", marked: "ni'hao", cursor: nil, continuesComposition: false),
                   .markThenUnmark(caret: 2))
    XCTAssertEqual(InlineCompositionPolicy.commit("🌲", marked: "shu", cursor: nil, continuesComposition: false),
                   .markThenUnmark(caret: 2), "光标位置按 UTF-16 计")
    // 接着组字时保留 #10284 的写法；没有标记区时不必替换。
    XCTAssertEqual(InlineCompositionPolicy.commit("香港地", marked: "hxgsdzyb", cursor: nil, continuesComposition: true), .insert)
    XCTAssertEqual(InlineCompositionPolicy.commit("你好", marked: "", cursor: nil, continuesComposition: false), .insert)
    // 快捷短语的光标后缀照旧走临时标记，不管后面是否还组字。
    XCTAssertEqual(InlineCompositionPolicy.commit("title:\"\"", marked: "Kt", cursor: 7, continuesComposition: true),
                   .markThenUnmark(caret: 7))
  }

  /// 闲鱼里的现象：选词之后拼音和汉字都留在输入框里。
  func testFlutterHostKeepsOnlyTheCandidateAfterSelection() throws {
    let host = flutterHost("甲丙", caret: 1)
    let keyboard = try keyboard(host)
    keyboard.render(.init(isHandled: true, preedit: "ni'hao", editingText: "nihao"))
    XCTAssertEqual(host.text as String, "甲ni'hao丙")
    XCTAssertNotNil(host.marked)
    keyboard.render(.init(isHandled: true, commitText: "你好"))
    XCTAssertEqual(host.text as String, "甲你好丙")
    XCTAssertEqual(host.selected, NSRange(location: 3, length: 0))
    XCTAssertNil(host.marked)
    XCTAssertFalse(host.calls.contains("insertText"))
    // 下一段组字从刚上屏的汉字之后开始。
    keyboard.render(.init(isHandled: true, preedit: "a", editingText: "a"))
    keyboard.render(.init(isHandled: true, commitText: "啊"))
    XCTAssertEqual(host.text as String, "甲你好啊丙")
    XCTAssertEqual(host.selected, NSRange(location: 4, length: 0))
  }

  /// 数字页原样结束组字时，拼写和数字作为同一次替换写进标记区。
  func testFlutterHostReplacesTheSpellingWithARawCommitAndLiteral() throws {
    let host = flutterHost("", caret: 0)
    let keyboard = try keyboard(host)
    keyboard.render(.init(isHandled: true, preedit: "ni", editingText: "ni"))
    keyboard.render(.init(isHandled: true, commitText: "ni"), appendingLiteral: "1")
    XCTAssertEqual(host.text as String, "ni1")
    XCTAssertEqual(host.selected, NSRange(location: 3, length: 0))
    XCTAssertNil(host.marked)
  }

  /// 部分选词仍是 `insertText` 后再标记剩余组字，不在同一事件里先定稿再重标记（#10284）。
  func testPartialSelectionKeepsInsertThenMarksTheRest() throws {
    let host = flutterHost("", caret: 0)
    let keyboard = try keyboard(host)
    keyboard.render(.init(isHandled: true, preedit: "hxgsdzyb", editingText: "hxgsdzyb"))
    host.calls = []
    keyboard.render(.init(isHandled: true, commitText: "香港地", preedit: "yb", editingText: "yb"))
    XCTAssertEqual(host.calls, ["insertText", "setMarkedText"])
  }

  /// 本来就遵守 UIKit 约定的宿主不因显式替换而多写或少写一遍。
  func testTextViewHostReceivesTheCandidateExactlyOnce() throws {
    let host = TextViewHost()
    let keyboard = try keyboard(host)
    // 编辑器不放进窗口、不当第一响应者。当第一响应者时测试宿主会拉起自己的系统键盘，实测按字母排在后面的 `NineKeyKeyboardTests` 常用语面板用例随后会在顶栏显示「输入运行时尚未准备完成」、工具栏被藏起；不当第一响应者时不再出现。标记、选区与定稿仍由 `UITextView` 自己执行。
    host.editor.text = "甲丙"
    host.editor.selectedRange = NSRange(location: 1, length: 0)
    keyboard.render(.init(isHandled: true, preedit: "ni'hao", editingText: "nihao"))
    XCTAssertNotNil(host.editor.markedTextRange)
    keyboard.render(.init(isHandled: true, commitText: "你好"))
    XCTAssertEqual(host.editor.text, "甲你好丙")
    XCTAssertEqual(host.editor.selectedRange, NSRange(location: 3, length: 0))
    XCTAssertNil(host.editor.markedTextRange)
  }
}
