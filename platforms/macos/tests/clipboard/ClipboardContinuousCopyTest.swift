import AppKit

@MainActor private final class CopySource: MacClipboardSource {
  let board = NSPasteboard.withUniqueName()
  var reads = 0
  var changeCount: Int { board.changeCount }
  var types: [NSPasteboard.PasteboardType]? { board.types }
  func text() -> String? { reads += 1; return board.string(forType: .string) }
  func copy(_ text: String) {
    board.clearContents()
    precondition(board.setString(text, forType: .string))
  }
}

@main enum ClipboardContinuousCopyTest {
  @MainActor static func wait(_ condition: () -> Bool) async {
    for _ in 0..<2000 {
      if condition() { return }
      try? await Task.sleep(nanoseconds: 1_000_000)
    }
    preconditionFailure("剪贴板专项没有到达预期状态")
  }

  @MainActor static func main() async {
    let source = CopySource()
    defer { source.board.releaseGlobally() }
    source.copy("synthetic existing")
    let monitor = MacClipboardMonitor(source: source)
    var ready = false
    var allowed = true
    var pending: CheckedContinuation<Void, Never>?
    var saved: [String] = []
    var attempts = 0
    var task: Task<Void, Never>?
    task = Task {
      await monitor.run(interval: 1_000_000, enabled: { allowed }, save: { text in
        attempts += 1
        if attempts == 1 {
          await withCheckedContinuation { pending = $0 }
          throw NSError(domain: "SyntheticSave", code: 1)
        }
        saved.append(text)
        return true
      }, publish: { if $0 == .monitoring { ready = true } })
    }
    await wait { ready }
    precondition(source.reads == 0 && saved.isEmpty, "启动不导入旧内容")
    source.copy("synthetic alpha")
    await wait { pending != nil }
    let firstReads = source.reads
    source.copy("synthetic beta\nsecond line")
    await wait { source.reads > firstReads }
    let secondReads = source.reads
    source.copy("synthetic gamma 😀")
    await wait { source.reads > secondReads }
    pending?.resume(); pending = nil
    await wait { saved.count == 3 }
    precondition(saved == ["synthetic alpha", "synthetic beta\nsecond line", "synthetic gamma 😀"],
      "保存失败后的重试与后续复制均按原顺序保留")

    allowed = false
    await wait { attempts >= 4 }
    try? await Task.sleep(nanoseconds: 10_000_000)
    let disabledReads = source.reads
    source.copy("synthetic while disabled")
    try? await Task.sleep(nanoseconds: 10_000_000)
    precondition(source.reads == disabledReads && saved.count == 3, "关闭不读取正文")
    task?.cancel()
    await task!.value

    let cancelled = MacClipboardMonitor(source: source)
    ready = false
    let cancellation = Task {
      await cancelled.run(interval: 1_000_000, enabled: { true }, save: { _ in
        await withCheckedContinuation { pending = $0 }
        return true
      }, publish: { if $0 == .monitoring { ready = true } })
    }
    await wait { ready }
    source.copy("synthetic pending cancellation")
    await wait { pending != nil }
    cancellation.cancel()
    let cancelledReads = source.reads
    source.copy("synthetic after cancellation")
    try? await Task.sleep(nanoseconds: 10_000_000)
    precondition(source.reads == cancelledReads, "等待落盘结束时取消也必须立即停采")
    pending?.resume(); pending = nil
    await cancellation.value

    // 队列用真实隔离剪贴板检验上限、敏感标记、关闭清空与迟到保存确认。
    let buffer = MacClipboardSamplingBuffer(capture: MacClipboardCapture(source: source))
    buffer.setEnabled(true); buffer.sample()
    source.copy("synthetic head"); buffer.sample()
    let old = buffer.first!
    for index in 0..<MacClipboardSamplingBuffer.capacity {
      source.copy("synthetic bounded \(index)"); buffer.sample()
    }
    buffer.saved(old)
    precondition(buffer.first?.text == "synthetic bounded 0", "溢出后的迟到确认不删除新头部")
    buffer.setEnabled(false); buffer.setEnabled(true); buffer.sample()
    precondition(buffer.first == nil, "重启丢弃旧队列并建立新基线")
    source.copy("synthetic concealed")
    source.board.setData(Data(), forType: NSPasteboard.PasteboardType("org.nspasteboard.ConcealedType"))
    buffer.sample()
    precondition(buffer.first == nil, "敏感类型不能进入异步队列")
    print("隔离系统剪贴板的连续复制、保存重试、关闭、上限和敏感标记通过")
  }
}
