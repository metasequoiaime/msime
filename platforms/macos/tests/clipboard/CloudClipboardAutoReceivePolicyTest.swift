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
    check(policy.receive(text: "云端新内容", updatedAt: "2026-11-13T00:00:01Z", currentText: "本机复制"),
          "建立水位后，未改变的本机内容应接受新远端复制")
    check(policy.receive(text: "云端新内容", updatedAt: "2026-11-13T00:00:01Z", currentText: "本机复制"),
          "写入失败不应消费远端水位")
    policy.received(text: "云端新内容", updatedAt: "2026-11-13T00:00:01Z")
    check(!policy.receive(text: "云端新内容", updatedAt: "2026-11-13T00:00:02Z", currentText: "云端新内容"),
          "相同内容不应重复写入")
    // 无需等采集器回调，网络返回时也必须识别本机复制。
    check(!policy.receive(text: "云端更晚内容", updatedAt: "2026-11-13T00:00:03Z", currentText: "用户新复制"),
          "用户在请求期间复制后应重新建立水位")
    check(policy.receive(text: "之后的云端复制", updatedAt: "2026-11-13T00:00:04.123Z", currentText: "用户新复制"),
          "带小数秒的后续复制仍可接收")
    policy.received(text: "之后的云端复制", updatedAt: "2026-11-13T00:00:04.123Z")
    check(!policy.receive(text: "旧条目", updatedAt: "2026-11-13T00:00:01Z", currentText: "之后的云端复制"),
          "删除后出现的旧条目不应再次接收")
    policy.localChanged(text: "")
    check(!policy.receive(text: "坏时间", updatedAt: "synthetic-time", currentText: ""),
          "非法时间戳不能建立基线")
    check(!policy.receive(text: "本轮云端", updatedAt: "2026-11-13T00:00:05Z", currentText: ""),
          "清空剪贴板也应保护本轮本机变化")
    check(policy.receive(text: "下一轮云端", updatedAt: "2026-11-13T00:00:06Z", currentText: ""),
          "建立基线后新复制可达")
    policy.reset()
    policy.begin(text: "合成本机内容")
    policy.emptyPage(currentText: "合成本机内容")
    check(policy.receive(text: "云端首条", updatedAt: "2026-11-13T00:00:00Z", currentText: "合成本机内容"),
          "换绑定应重置水位，空云列表也应建立基线")
    print("Cloud clipboard auto-receive policy tests passed")
  }
}
