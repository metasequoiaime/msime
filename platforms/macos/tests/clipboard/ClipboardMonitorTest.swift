import AppKit

@MainActor private final class MonitorSource: MacClipboardSource {
  var changeCount = 0
  var types: [NSPasteboard.PasteboardType]? = [.string]
  var value = "synthetic initial"
  var reads = 0
  func text() -> String? { reads += 1; return value }
  func change(_ value: String) { self.value = value; changeCount += 1 }
}

@main enum ClipboardMonitorTest {
  @MainActor static func main() async {
    let source = MonitorSource()
    let monitor = MacClipboardMonitor(source: source)
    var cycle = 0
    var saved: [String] = []
    var statuses: [MacClipboardMonitor.Status] = []
    var task: Task<Void, Never>?
    task = Task {
      await monitor.run(interval: 1_000_000, enabled: {
        cycle += 1
        switch cycle {
        case 1: return false
        case 3: source.change("synthetic alpha")
        case 6: source.change("synthetic while disabled"); return false
        case 8: throw NSError(domain: "SyntheticPreferences", code: 1)
        case 10: source.change("synthetic beta")
        case 12: source.change("synthetic gamma")
        case 13: task?.cancel()
        default: break
        }
        return true
      }, save: { text in
        saved.append(text)
        if saved.count == 1 { throw NSError(domain: "SyntheticSave", code: 1) }
        return text != "synthetic beta"
      }, publish: { statuses.append($0) })
    }
    await task!.value
    // 保存重试使用队列内的原文，每条变化只读取一次系统正文。
    assert(cycle == 13 && source.reads == 3)
    assert(saved == ["synthetic alpha", "synthetic alpha", "synthetic beta", "synthetic gamma"])
    assert(statuses == [.disabled, .monitoring, .saveFailed, .monitoring, .disabled,
      .monitoring, .unavailable, .monitoring, .disabled, .monitoring])

    // Cancellation while preferences are in flight must not sample or save.
    let lateSource = MonitorSource()
    let late = MacClipboardMonitor(source: lateSource)
    var continuation: CheckedContinuation<Bool, Never>?
    var published = false
    let pending = Task {
      await late.run(enabled: {
        await withCheckedContinuation { continuation = $0 }
      }, save: { _ in assertionFailure("cancelled monitor saved"); return true },
        publish: { _ in published = true })
    }
    for _ in 0..<1000 {
      if continuation != nil { break }
      try? await Task.sleep(nanoseconds: 1_000_000)
    }
    assert(continuation != nil)
    pending.cancel()
    continuation?.resume(returning: true)
    await pending.value
    assert(!published && lateSource.reads == 0)
    print("Monitor enablement, persistence retry, baseline recovery and cancellation tests passed")
  }
}
