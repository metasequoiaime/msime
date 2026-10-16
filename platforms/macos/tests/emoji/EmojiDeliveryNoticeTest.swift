import AppKit
import SwiftUI

@main enum EmojiDeliveryNoticeTest {
  @MainActor static func main() {
    _ = NSApplication.shared
    let panel = MacEmojiDeliveryNotice.makePanel()
    assert(!panel.canBecomeKey && !panel.canBecomeMain)
    assert(panel.styleMask.contains(.nonactivatingPanel))
    assert(panel.ignoresMouseEvents && !panel.hidesOnDeactivate)
    assert(panel.level == .floating)
    assert(!panel.isVisible)
    let label = panel.contentView?.subviews.compactMap { $0 as? NSTextField }.first
    assert(label?.stringValue == MacEmojiDeliveryNotice.message)
    assert(label?.isEditable == false)
    assert(panel.frame.size == NSSize(width: 340, height: 64))
    // The notice is drawn in the emoji panel's mode and palette, not the system window colours.
    assert(MacEmojiDeliveryNotice.appearance(for: .light)?.name == .aqua)
    assert(MacEmojiDeliveryNotice.appearance(for: .dark)?.name == .darkAqua)
    assert(MacEmojiDeliveryNotice.appearance(for: nil) == nil)
    func rgb(_ color: NSColor?, in name: NSAppearance.Name) -> UInt32 {
      var resolved: NSColor?
      NSAppearance(named: name)!.performAsCurrentDrawingAppearance { resolved = color?.usingColorSpace(.sRGB) }
      guard let resolved else { return 0 }
      return UInt32((resolved.redComponent * 255).rounded()) << 16 | UInt32((resolved.greenComponent * 255).rounded()) << 8
        | UInt32((resolved.blueComponent * 255).rounded())
    }
    assert(rgb(panel.backgroundColor, in: .aqua) == MacEmojiPalette(light: true).background)
    assert(rgb(panel.backgroundColor, in: .darkAqua) == MacEmojiPalette(light: false).background)
    assert(rgb(label?.textColor, in: .darkAqua) == MacEmojiPalette(light: false).text)
    MacEmojiAppearance.shared.apply(["theme": "dark"])
    let notice = MacEmojiDeliveryNotice()
    notice.show()
    let shown = NSApp.windows.compactMap { $0 as? MacEmojiDeliveryNotice.Panel }.first { $0.isVisible }
    assert(shown?.appearance?.name == .darkAqua)
    notice.dismiss()
    panel.close()
    print("Emoji delivery notice configuration checks passed")
  }
}
