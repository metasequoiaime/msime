import AppKit
import Foundation

/// 云剪贴板开关打开后，桌面输入法在运行期间同步系统剪贴板。网络请求和端到端加密仍经共享 Rust 驱动；本类只负责生命周期、剪贴板和“不要把接收内容再上传”的边界。
@MainActor
final class MacCloudClipboardAutoSync {
  private let source: any MacClipboardSource
  private let writer: (any MacClipboardWriter)?
  private var captureTask: Task<Void, Never>?
  private var pollTask: Task<Void, Never>?
  private var active = false
  private var cloudEnabled = false
  private var route: BackendCloudClipboardRoute?
  private var policy = MacCloudClipboardReceivePolicy()
  private var generation: UInt64 = 0
  private var localSequence: Int?
  private var captureNeedsBaseline = true

  init(source: any MacClipboardSource = MacPasteboardSource(pasteboard: .general)) {
    self.source = source
    self.writer = source as? any MacClipboardWriter
  }

  func start(directory: String) {
    stop()
    guard NSString(string: directory).isAbsolutePath else { return }
    active = true
    generation &+= 1
    let token = generation
    let monitor = MacClipboardMonitor(source: source)
    captureTask = Task { [weak self] in
      guard let self else { return }
      await monitor.run(interval: 400_000_000, enabled: {
        if self.captureNeedsBaseline {
          self.captureNeedsBaseline = false
          return false
        }
        return self.active && self.cloudEnabled && self.generation == token
      }, save: { text in
        await self.upload(text, token: token)
        return true
      }, publish: { _ in })
    }
    pollTask = Task { [weak self] in
      guard let self else { return }
      await self.pollLoop(token: token)
    }
  }

  func stop() {
    active = false
    cloudEnabled = false
    route = nil
    generation &+= 1
    captureTask?.cancel()
    pollTask?.cancel()
    captureTask = nil
    pollTask = nil
    policy.reset()
    localSequence = nil
    captureNeedsBaseline = true
  }

  private func pollLoop(token: UInt64) async {
    while active && generation == token && !Task.isCancelled {
      let delay = await poll(token: token)
      do { try await Task.sleep(nanoseconds: delay) }
      catch { return }
    }
  }

  private func poll(token: UInt64) async -> UInt64 {
    guard active, generation == token, let rust = BackendRustAccount.shared else {
      cloudEnabled = false
      return 30_000_000_000
    }
    do {
      let selected = try await BackendCloudClipboardRoute.selected(rust: rust)
      guard active, generation == token else { return 30_000_000_000 }
      if selected != route {
        route = selected
        captureNeedsBaseline = true
        policy.reset()
        if let text = source.text() { policy.begin(text: text) }
        localSequence = source.changeCount
      }
      guard let selected else {
        cloudEnabled = false
        return 30_000_000_000
      }
      let pageData = try await selected.request(rust, ["operation": "list", "search": ""],
                                                deviceName: MacCloudClipboardDriver.deviceName)
      let page = try BackendRustAccount.decode(BackendAccountClient.ClipboardPage.self, pageData)
      guard active, generation == token, selected == route else { return 30_000_000_000 }
      cloudEnabled = page.enabled && !page.isPending
      guard cloudEnabled else { return 30_000_000_000 }
      // 图片、受保护类型和读取失败均不代表空剪贴板。
      guard let types = source.types,
            !types.contains(where: { MacClipboardCapture.excludedTypes.contains($0.rawValue) }),
            types.isEmpty || types.contains(.string) else { return 3_000_000_000 }
      let sequence = source.changeCount
      guard let current = types.isEmpty ? "" : source.text(), source.changeCount == sequence else {
        return 3_000_000_000
      }
      if localSequence != sequence { policy.localChanged(text: current) }
      localSequence = sequence
      guard let item = page.items.first else {
        policy.emptyPage(currentText: current)
        return 3_000_000_000
      }
      if policy.receive(text: item.text, updatedAt: item.updated_at, currentText: current), let writer {
        writer.setText(item.text)
        if source.text() == item.text {
          policy.received(text: item.text, updatedAt: item.updated_at)
          localSequence = source.changeCount
        }
      }
      return 3_000_000_000
    } catch {
      cloudEnabled = false
      return 30_000_000_000
    }
  }

  private func upload(_ text: String, token: UInt64) async {
    guard active, generation == token, cloudEnabled, let selected = route,
          let rust = BackendRustAccount.shared else { return }
    // 接收端写入系统剪贴板也会被采集器观察到；消费这一次回调，不把内容再次发回服务端。
    // 用户之后复制不同内容时重新开始一轮同步。
    if policy.receiverOwned && policy.localText == text {
      return
    }
    policy.localChanged(text: text)
    let sequence = source.changeCount
    do {
      let pageData = try await selected.request(rust, ["operation": "list", "search": ""],
                                                deviceName: MacCloudClipboardDriver.deviceName)
      let page = try BackendRustAccount.decode(BackendAccountClient.ClipboardPage.self, pageData)
      guard active, generation == token, selected == route, page.enabled, !page.isPending,
            source.changeCount == sequence else { return }
      _ = try await selected.request(rust, ["operation": "add", "text": text],
                                     deviceName: MacCloudClipboardDriver.deviceName)
    } catch {
      // 失败不留持久队列；下一次用户复制会重新尝试。
    }
  }
}
