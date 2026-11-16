import AppKit

@MainActor final class MacClipboardMonitor {
  enum Status: Equatable {
    case disabled, monitoring, unavailable, saveFailed
    var message: String {
      switch self {
      case .disabled: return "剪贴板采集已停用"
      case .monitoring: return "输入法运行且历史开启时，保存新复制的文本"
      case .unavailable: return "无法读取共享设置，剪贴板采集已暂停"
      case .saveFailed: return "剪贴板历史保存失败，将重试"
      }
    }
  }
  private let capture: MacClipboardCapture
  init(source: any MacClipboardSource) { capture = MacClipboardCapture(source: source) }

  func run(interval: UInt64 = 50_000_000,
    enabled: () async throws -> Bool,
    save: (String) async throws -> Bool,
    publish: (Status) -> Void
  ) async {
    let buffer = MacClipboardSamplingBuffer(capture: capture)
    let sampler = Task { @MainActor in
      while !Task.isCancelled {
        buffer.sample()
        do { try await Task.sleep(nanoseconds: interval) }
        catch { return }
      }
    }
    defer { sampler.cancel(); buffer.setEnabled(false) }
    var previous: Status?
    await withTaskCancellationHandler {
      while !Task.isCancelled {
        let status: Status
        do {
          let allowed = try await enabled()
          guard !Task.isCancelled else { return }
          buffer.setEnabled(allowed)
          buffer.sample()
          if !allowed {
            status = .disabled
          } else if let sample = buffer.first {
            do {
              let saved = try await save(sample.text)
              guard !Task.isCancelled else { return }
              if saved {
                buffer.saved(sample)
                status = .monitoring
              } else {
                buffer.setEnabled(false)
                status = .disabled
              }
            } catch {
              status = .saveFailed
            }
          } else {
            status = .monitoring
          }
        } catch {
          buffer.setEnabled(false)
          status = .unavailable
        }
        guard !Task.isCancelled else { return }
        if previous != status { publish(status); previous = status }
        do { try await Task.sleep(nanoseconds: interval) }
        catch { return }
      }
    } onCancel: { sampler.cancel() }
  }

  static func run(directory: String, publish: (Status) -> Void) async {
    let monitor = MacClipboardMonitor(source: MacPasteboardSource(pasteboard: .general))
    await monitor.run(enabled: {
      try await Task.detached { try MacEmojiClipboardHistory.captureEnabled(directory: directory) }.value
    }, save: { text in
      try await Task.detached { try MacEmojiClipboardHistory.capture(directory: directory, text: text) }.value
    }, publish: publish)
  }
}
