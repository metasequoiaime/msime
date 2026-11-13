import Foundation
import UIKit
import UniformTypeIdentifiers

@MainActor
final class KeyboardCloudClipboardAutoReceive {
  private let service: any KeyboardCloudClipboardService
  private let pasteboard: UIPasteboard
  private var task: Task<Void, Never>?
  private var active = false
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
    policy.begin(changeCount: pasteboard.changeCount, hasStrings: pasteboard.hasStrings)
    task = Task { [weak self] in
      await self?.poll(allowed: allowed)
    }
  }

  func stop() {
    active = false
    task?.cancel()
    task = nil
    binding = nil
  }

  private func poll(allowed: @escaping @MainActor () -> Bool) async {
    while active && !Task.isCancelled {
      await receive(allowed: allowed)
      do { try await Task.sleep(nanoseconds: 3_000_000_000) }
      catch { return }
    }
  }

  private func receive(allowed: @escaping @MainActor () -> Bool) async {
    guard active, allowed() else { return }
    let changeCount = pasteboard.changeCount
    let hasStrings = pasteboard.hasStrings
    if policy.localChangeCount != changeCount {
      policy.localChanged(changeCount: changeCount, hasStrings: hasStrings)
    }
    do {
      guard let current = try await service.binding() else { return }
      if let binding, binding == current, !(await service.matches(current)) { return }
      if binding != current {
        binding = current
        policy = KeyboardCloudClipboardReceivePolicy()
        policy.begin(changeCount: pasteboard.changeCount, hasStrings: pasteboard.hasStrings)
      }
      let page = try await service.page(matching: current)
      guard active, allowed(), await service.matches(current), page.enabled, !page.isPending,
            let item = page.items.first else { return }
      let currentCount = pasteboard.changeCount
      let currentHasStrings = pasteboard.hasStrings
      if policy.localChangeCount != currentCount {
        policy.localChanged(changeCount: currentCount, hasStrings: currentHasStrings)
      }
      guard policy.receive(item, changeCount: currentCount, hasStrings: currentHasStrings) else { return }
      pasteboard.setItems([[UTType.utf8PlainText.identifier: item.text]], options: [.localOnly: true])
      policy.markRemote(item)
      policy.received(changeCount: pasteboard.changeCount)
    } catch {
      // 输入过程中网络失败静默放弃，下一轮只重新读取云端最新页。
    }
  }
}
