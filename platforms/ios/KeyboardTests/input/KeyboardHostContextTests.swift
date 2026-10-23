import ObjectiveC
import UIKit
import XCTest

/// 真实代理只遵循系统协议，不会声明项目私有协议；文档身份必须从公开 getter 读取。
@MainActor
final class KeyboardHostContextTests: XCTestCase {
  private final class Document: NSObject, UITextDocumentProxy {
    var documentIdentifier = UUID()
    var documentContextBeforeInput: String? { "abcdef" }
    var documentContextAfterInput: String? { nil }
    var selectedText: String? { nil }
    var documentInputMode: UITextInputMode? { nil }
    var hasText: Bool { true }
    func insertText(_ text: String) {}
    func deleteBackward() {}
    func adjustTextPosition(byCharacterOffset offset: Int) {}
    func setMarkedText(_ text: String, selectedRange: NSRange) {}
    func unmarkText() {}
  }

  func testThePublicProxyGetterPreservesDocumentIdentity() {
    let document = Document()
    XCTAssertEqual(KeyboardHostContext.documentIdentifier(for: document), document.documentIdentifier)
    XCTAssertEqual(KeyboardHostContext.documentIdentifier(for: document), document.documentIdentifier)
  }

  func testChangingTheDocumentIsVisibleToCursorMovement() throws {
    let document = Document()
    var movement = SpaceCursorMovement()
    movement.begin(at: 0, document: try XCTUnwrap(KeyboardHostContext.documentIdentifier(for: document)))
    XCTAssertEqual(movement.advance(to: 24,
      document: try XCTUnwrap(KeyboardHostContext.documentIdentifier(for: document))), 2)
    document.documentIdentifier = UUID()
    XCTAssertEqual(movement.advance(to: 48,
      document: try XCTUnwrap(KeyboardHostContext.documentIdentifier(for: document))), 0)
    XCTAssertFalse(movement.isActive)
  }

  /// 无输入文档的测试宿主可能返回 nil，不能经 Swift 的非空 UUID 桥接而崩溃。
  func testANullObjectiveCGetterSafelyReturnsNoDocument() throws {
    let selector = #selector(getter: UITextDocumentProxy.documentIdentifier)
    let method = try XCTUnwrap(class_getInstanceMethod(Document.self, selector))
    let original = method_getImplementation(method)
    let block: @convention(block) (AnyObject) -> NSUUID? = { _ in nil }
    let replacement = imp_implementationWithBlock(block)
    method_setImplementation(method, replacement)
    defer {
      method_setImplementation(method, original)
      imp_removeBlock(replacement)
    }
    XCTAssertNil(KeyboardHostContext.documentIdentifier(for: Document()))
  }
}
