import Foundation

private func check(_ condition: @autoclosure () -> Bool, _ message: String, line: UInt = #line) {
  if !condition() {
    FileHandle.standardError.write(Data("CloudClipboardAutoReceivePolicyTest:\(line): \(message)\n".utf8))
    exit(1)
  }
}

@main enum CloudClipboardAutoReceivePolicyTest {
  static func main() {
    var policy = MacCloudClipboardReceivePolicy()
    policy.begin(text: "本机复制")
    check(!policy.receive(text: "云端旧内容", updatedAt: "2026-11-13T00:00:00Z", currentText: "本机复制"),
          "用户已有剪贴板内容时不应覆盖")
    policy.localChanged(text: "")
    check(policy.receive(text: "云端新内容", updatedAt: "2026-11-13T00:00:01Z", currentText: ""),
          "空剪贴板应接收新条目")
    policy.received(text: "云端新内容")
    check(!policy.receive(text: "云端新内容", updatedAt: "2026-11-13T00:00:02Z", currentText: "云端新内容"),
          "相同内容不应重复写入")
    policy.localChanged(text: "用户新复制")
    check(!policy.receive(text: "云端更晚内容", updatedAt: "2026-11-13T00:00:03Z", currentText: "用户新复制"),
          "用户复制后不应被云端覆盖")
    policy.localChanged(text: "")
    check(!policy.receive(text: "坏时间", updatedAt: "synthetic-time", currentText: ""),
          "非法时间戳应被拒绝")
    check(!policy.receive(text: "旧条目", updatedAt: "2026-11-13T00:00:01Z", currentText: ""),
          "旧条目不应再次接收")
    print("Cloud clipboard auto-receive policy tests passed")
  }
}
