import UIKit

enum KeyboardHostContext {
  static func documentIdentifier(for proxy: UITextDocumentProxy) -> UUID? {
    // 系统代理只遵循 UITextDocumentProxy；不能要求它声明项目自定义协议。
    // 经 Objective-C 读取公开 getter，保留无文档时返回 nil 的保护，避免非空 UUID 桥接崩溃。
    let selector = #selector(getter: UITextDocumentProxy.documentIdentifier)
    guard proxy.responds(to: selector) else { return nil }
    return proxy.perform(selector)?.takeUnretainedValue() as? UUID
  }
}
