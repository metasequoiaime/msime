import AppKit
import SwiftUI
import os

/// Objective-C entry points for native macOS controllers that need to present
/// the shared SwiftUI backend surfaces. Each window owns its hosting controller
/// and is rebuilt after closing or when the requesting account changes.
@MainActor @objc(MSIMEBackendWindowBridge)
final class BackendWindowBridge: NSObject {
  @objc static let shared = BackendWindowBridge()
  @objc func startClipboardCapture(withOptions options: NSDictionary) {
    MacClipboardService.shared.start(directory: options["preferences_directory"] as? String ?? "")
  }
  @objc func stopClipboardCapture() { MacClipboardService.shared.stop() }
  private let windows = BackendAccountWindowCache<NSWindowController>()
  private let emojiDeliveryNotice = MacEmojiDeliveryNotice()

  @objc func showEmojiDeliveryFailure() { emojiDeliveryNotice.show() }

  @objc func applyEmojiPreferences(_ preferences: NSDictionary) { MacEmojiAppearance.shared.apply(preferences) }
  @objc func applyHandwritingPreferences(_ preferences: NSDictionary) { MacHandwritingAppearance.shared.apply(preferences) }

  func closeAll() {
    emojiDeliveryNotice.dismiss()
    windows.closeAll { controller in
      controller.close()
      controller.window?.contentViewController = nil
    }
  }

  @objc func showDictionary(forAccountID accountID: String) { show("dictionary", accountID: accountID, title: "云词库", size: NSSize(width: 660, height: 650)) { MacCloudDictionaryView(accountID: accountID) } }
  @objc func showClipboard(forAccountID accountID: String) { show("clipboard", accountID: accountID, title: "云剪贴板", size: NSSize(width: 560, height: 560)) { MacCloudClipboardView(accountID: accountID) } }
  @objc func showSnapshot(forAccountID accountID: String) { show("snapshot", accountID: accountID, title: "词库快照", size: NSSize(width: 560, height: 460)) { MacCloudSnapshotView(accountID: accountID) } }
  @objc func showSettings(forAccountID accountID: String) { show("settings", accountID: accountID, title: "桌面设置同步", size: NSSize(width: 540, height: 520)) { MacCloudSettingsView(accountID: accountID) } }
  @objc func showHandwriting() { show("handwriting", accountID: "local", title: "水杉手写识别板", size: NSSize(width: 720, height: 510)) { MacHandwritingToolView() } }
  @objc func showHandwriting(selectionAttempt selection: @escaping (String) -> Bool) {
    weak var presented: NSWindowController?
    presented = show("handwriting", accountID: UUID().uuidString, title: "水杉手写识别板", size: NSSize(width: 720, height: 510)) {
      MacHandwritingToolView(onCandidate: { text in
        let accepted = selection(text)
        if accepted { presented?.close() }
        return accepted
      })
    }
  }
  @objc func showEmoji(withOptions options: NSDictionary, selectionAttempt selection: @escaping (String) -> Bool) {
    weak var presented: NSWindowController?
    // Each presentation binds a new target; never reuse an older selection closure.
    presented = show("emoji", accountID: UUID().uuidString, title: "表情与符号", size: NSSize(width: 420, height: 560)) {
      MacEmojiView(resources: options["resources"] as? String ?? "", preferencesDirectory: options["preferences_directory"] as? String ?? "", onSelect: { text in
        let accepted = selection(text)
        if accepted { presented?.close() }
        return accepted
      })
    }
  }
  @objc func showCommunityResources(forAccountID accountID: String) { show("resources", accountID: accountID, title: "词包与回复模板", size: NSSize(width: 650, height: 650)) { BackendCommunityResourcesView(accountID: accountID) } }

  @discardableResult private func show<Content: View>(_ key: String, accountID: String, title: String, size: NSSize, @ViewBuilder content: () -> Content) -> NSWindowController {
    // The key only; the account identifier is not logged.
    backendUILog.log("backend_window_requested key=\(key, privacy: .public)")
    let controller = windows.window(for: key, accountID: accountID,
      reusable: { $0.window?.isVisible == true || $0.window?.isMiniaturized == true },
      close: { controller in
        controller.close()
        controller.window?.contentViewController = nil
      }) {
      let host = NSHostingController(rootView: content())
      let window = key == "emoji" ? MacEmojiPanelWindow(contentViewController: host) : NSWindow(contentViewController: host)
      window.title = title; window.setContentSize(size)
      window.styleMask = key == "emoji" ? [.borderless, .closable, .resizable] : [.titled, .closable, .miniaturizable, .resizable]
      window.isReleasedWhenClosed = false
      let controller: NSWindowController
      if key == "emoji" {
        let emoji = MacEmojiWindowController(window: window)
        window.delegate = emoji
        controller = emoji
      } else {
        controller = NSWindowController(window: window)
      }
      window.center()
      return controller
    }
    controller.window?.deminiaturize(nil)
    controller.showWindow(nil)
    // Emoji and handwriting insert into the editor the user is typing in, which therefore has to stay the active application; they are only lifted above it.
    presentBackendWindow(controller.window, activating: key != "emoji" && key != "handwriting")
    return controller
  }
}

// The Swift side of MSIMEPresentWindow (src/core/WindowPresentation.h). The input method is LSBackgroundOnly: a prohibited application cannot become active, so a window it opens lands behind the app the user was typing in. Accessory lets it activate without a Dock icon, and ordering front regardless keeps the window visible when activation is declined, as it may be since macOS 14.
@MainActor func presentBackendWindow(_ window: NSWindow?, activating: Bool = true) {
  logBackendWindowState("present_begin activating=\(activating)", window)
  guard let window else { return }
  if activating {
    if NSApp.activationPolicy() == .prohibited { NSApp.setActivationPolicy(.accessory) }
    NSApp.activate(ignoringOtherApps: true)
  }
  window.makeKeyAndOrderFront(nil)
  window.orderFrontRegardless()
  logBackendWindowState("present_end", window)
  // The state a moment later says what the window server and the other applications made of the request.
  for delay in [0.5, 2.0] {
    DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak window] in
      guard let window else { return }
      MainActor.assumeIsolated { logBackendWindowState("present_after_\(Int(delay * 1000))ms", window) }
    }
  }
}

// 子系统、类别和字段都与 WindowPresentationLog.h 相同，一条 `log show --predicate 'subsystem == "app.msime.inputmethod.MetasequoiaIME" && category == "ui"'` 就能读到两边（子系统是本版本输入法的 bundle id，上面是 full 的）。只记状态：编号、类名、标题和标志。
private let backendUILog = Logger(subsystem: BackendEdition.inputMethodBundleIdentifier, category: "ui")

// The window's place among on-screen windows of its own level, 0 being frontmost; -1 when it is not on screen.
@MainActor private func backendWindowFrontIndex(_ window: NSWindow) -> Int {
  guard window.windowNumber > 0,
        let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]],
        let layer = list.first(where: { ($0[kCGWindowNumber as String] as? Int) == window.windowNumber })?[kCGWindowLayer as String] as? Int
  else { return -1 }
  let sameLayer = list.filter { ($0[kCGWindowLayer as String] as? Int) == layer }
  return sameLayer.firstIndex(where: { ($0[kCGWindowNumber as String] as? Int) == window.windowNumber }) ?? -1
}

@MainActor private func logBackendWindowState(_ stage: String, _ window: NSWindow?) {
  let policy: String
  switch NSApp.activationPolicy() {
  case .regular: policy = "regular"
  case .accessory: policy = "accessory"
  case .prohibited: policy = "prohibited"
  @unknown default: policy = "unknown"
  }
  let frontmost = NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "-"
  guard let window else {
    backendUILog.log("\(stage, privacy: .public) window=nil app_active=\(NSApp.isActive) policy=\(policy, privacy: .public) frontmost=\(frontmost, privacy: .public)")
    return
  }
  let occluded = !window.occlusionState.contains(.visible)
  backendUILog.log("\(stage, privacy: .public) window=\(window.windowNumber) class=\(String(describing: type(of: window)), privacy: .public) title=\(window.title, privacy: .public) visible=\(window.isVisible) key=\(window.isKeyWindow) occluded=\(occluded) level=\(window.level.rawValue) front_index=\(backendWindowFrontIndex(window)) app_active=\(NSApp.isActive) policy=\(policy, privacy: .public) frontmost=\(frontmost, privacy: .public)")
}
