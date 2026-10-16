import AppKit
import SwiftUI

@MainActor final class MacEmojiDeliveryNotice {
  static let message = "表情回填未完成，请回到输入位置重试。"
  private var panel: Panel?
  private var dismissal: DispatchWorkItem?

  final class Panel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
  }

  static func makePanel() -> Panel {
    let panel = Panel(contentRect: NSRect(x: 0, y: 0, width: 340, height: 64),
      styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.isReleasedWhenClosed = false
    panel.hidesOnDeactivate = false
    panel.ignoresMouseEvents = true
    panel.level = .floating
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    panel.hasShadow = true
    // The emoji panel's palette in whichever mode the notice is drawn in, so it reads as part of that panel.
    panel.backgroundColor = paletteColor(\.background)
    let label = NSTextField(wrappingLabelWithString: message)
    label.font = .systemFont(ofSize: 14)
    label.textColor = paletteColor(\.text)
    label.frame = NSRect(x: 16, y: 12, width: 308, height: 40)
    panel.contentView?.addSubview(label)
    return panel
  }

  static func paletteColor(_ value: KeyPath<MacEmojiPalette, UInt32>) -> NSColor {
    NSColor(name: nil) { appearance in
      let dark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
      let rgb = MacEmojiPalette(light: !dark)[keyPath: value]
      return NSColor(srgbRed: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
        blue: CGFloat(rgb & 255) / 255, alpha: 1)
    }
  }

  /// 表情面板画成哪种明暗：取颜色模式 `theme`（macOS 的表情面板没有单独的明暗设置），它没指定明暗时跟随系统。
  static func appearance(for scheme: ColorScheme?) -> NSAppearance? {
    switch scheme {
    case .light: NSAppearance(named: .aqua)
    case .dark: NSAppearance(named: .darkAqua)
    default: nil
    }
  }

  func show() {
    dismissal?.cancel()
    let notice = panel ?? Self.makePanel()
    panel = notice
    notice.appearance = Self.appearance(for: MacEmojiAppearance.shared.colorScheme)
    if let screen = NSScreen.main ?? NSScreen.screens.first {
      let visible = screen.visibleFrame
      notice.setFrameOrigin(NSPoint(x: visible.midX - notice.frame.width / 2, y: visible.minY + 32))
    }
    notice.orderFrontRegardless()
    let work = DispatchWorkItem { [weak self] in self?.dismiss() }
    dismissal = work
    DispatchQueue.main.asyncAfter(deadline: .now() + 5, execute: work)
  }

  func dismiss() {
    dismissal?.cancel()
    dismissal = nil
    panel?.orderOut(nil)
    panel = nil
  }
}
