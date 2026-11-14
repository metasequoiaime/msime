import Foundation
import UIKit
import UniformTypeIdentifiers

@MainActor
final class KeyboardCloudClipboardAutoReceive {
  private let service: any KeyboardCloudClipboardService
  private let pasteboard: UIPasteboard
  private var task: Task<Void, Never>?
  private var active = false
  private var generation = 0
  private var policy = KeyboardCloudClipboardReceivePolicy()
  private var binding: KeyboardCloudClipboardBinding?

  init(service: any KeyboardCloudClipboardService = BackendKeyboardCloudClipboardService(),
       pasteboard: UIPasteboard = .general) {
    self.service = service
    self.pasteboard = pasteboard
  }

  func start(allowed: @escaping @MainActor () -> Bool) {
    stop()
    guard allowed() else { return }
    active = true
    policy.begin(changeCount: pasteboard.changeCount, hasItems: pasteboard.numberOfItems > 0)
    let runGeneration = generation
    task = Task { [weak self] in
      await self?.poll(generation: runGeneration, allowed: allowed)
    }
  }

  func stop() {
    generation &+= 1
    active = false
    task?.cancel()
    task = nil
    binding = nil
  }

  private func current(_ generation: Int) -> Bool {
    active && self.generation == generation && !Task.isCancelled
  }

  private func poll(generation: Int, allowed: @escaping @MainActor () -> Bool) async {
    while current(generation) {
      await receive(generation: generation, allowed: allowed)
      do { try await Task.sleep(nanoseconds: 3_000_000_000) }
      catch { return }
    }
  }

  private func receive(generation: Int, allowed: @escaping @MainActor () -> Bool) async {
    guard current(generation), allowed() else { return }
    let changeCount = pasteboard.changeCount
    let locallyChanged = policy.localChangeCount != changeCount
    if locallyChanged {
      policy.localChanged(changeCount: changeCount)
    }
    do {
      guard let captured = try await service.binding(), current(generation), allowed() else { return }
      guard await service.matches(captured), current(generation), allowed() else { return }
      let sameBinding = binding == captured
      if binding != captured {
        binding = captured
        policy = KeyboardCloudClipboardReceivePolicy()
        policy.begin(changeCount: pasteboard.changeCount, hasItems: pasteboard.numberOfItems > 0)
        if changeCount != pasteboard.changeCount {
          policy.localChanged(changeCount: pasteboard.changeCount)
        }
      }
      let page = try await service.page(matching: captured)
      guard current(generation), allowed(), page.enabled, !page.isPending else { return }
      guard await service.matches(captured), current(generation), allowed() else { return }
      guard pasteboard.numberOfItems == 0 || pasteboard.hasStrings else {
        if let item = page.items.first { policy.markRemote(item) }
        return
      }
      let currentCount = pasteboard.changeCount
      if sameBinding && (locallyChanged || currentCount != changeCount) {
        // 正文读取服从系统粘贴授权；拒绝或失败只丢弃这次复制，不持续弹出重试。
        _ = policy.receive(page.items.first, changeCount: currentCount)
        guard let text = pasteboard.string, KeyboardCloudClipboard.validUploadText(text),
              pasteboard.changeCount == currentCount, current(generation), allowed() else { return }
        if page.items.first?.text != text {
          let uploaded = try await service.uploadIfEnabled(text, matching: captured)
          guard await service.matches(captured), current(generation), allowed() else { return }
          if let item = uploaded?.items.first, item.text == text { policy.markRemote(item) }
        }
        return
      }
      guard policy.receive(page.items.first, changeCount: currentCount), let item = page.items.first else { return }
      pasteboard.setItems([[UTType.utf8PlainText.identifier: item.text]], options: [.localOnly: true])
      policy.markRemote(item)
      policy.received(changeCount: pasteboard.changeCount)
    } catch {
      // 输入过程中网络失败静默放弃，下一轮只重新读取云端最新页。
    }
  }
}
