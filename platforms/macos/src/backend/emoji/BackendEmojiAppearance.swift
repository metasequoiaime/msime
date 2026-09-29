import SwiftUI

/// Palette from the fixed Windows EmojiPanel reference (04a8df56).
struct MacEmojiPalette {
  let background: UInt32
  let text: UInt32
  let muted: UInt32
  let selected: UInt32
  let pressed: UInt32
  let accent: UInt32
  let searchBackground: UInt32
  let searchBorder: UInt32
  let searchFocusOpacity: Double

  init(light: Bool) {
    background = light ? 0xF7F7FA : 0x202027
    text = light ? 0x202027 : 0xF5F5F7
    muted = light ? 0x686873 : 0xAFAFB7
    // The accent is the brand green (#2C7A4B / #5FBF84) in place of the reference's panel purple, and the light hover and press fills are that green at 12% and 20% over the background instead of the purple's lilac tints; the dark fills were already neutral.
    selected = light ? 0xDFE8E5 : 0x3B3B44
    pressed = light ? 0xCEDED7 : 0x555560
    accent = light ? 0x2C7A4B : 0x5FBF84
    searchBackground = light ? 0xFFFFFF : 0x2B2B33
    searchBorder = light ? 0xD0D0D8 : 0x3A3A44
    searchFocusOpacity = light ? 0.75 : 0.70
  }

  func cellFill(hovered: Bool, isPressed: Bool) -> UInt32? {
    isPressed ? pressed : hovered ? selected : nil
  }

  static func color(_ rgb: UInt32) -> Color {
    Color(.sRGB, red: Double((rgb >> 16) & 255) / 255,
      green: Double((rgb >> 8) & 255) / 255, blue: Double(rgb & 255) / 255, opacity: 1)
  }
}

@MainActor final class MacEmojiAppearance: ObservableObject {
  static let shared = MacEmojiAppearance()
  @Published private(set) var colorScheme: ColorScheme?

  func apply(_ preferences: NSDictionary) {
    let resolved: ColorScheme?
    let surface = preferences["emoji_theme"] as? String
    let global = preferences["theme"] as? String
    switch (surface == "dark" || surface == "light") ? surface : global {
    case "light": resolved = .light
    case "system": resolved = nil
    case "dark": resolved = .dark
    default: resolved = nil
    }
    if colorScheme != resolved { colorScheme = resolved }
  }
}

struct MacEmojiCellStyle: ButtonStyle {
  let palette: MacEmojiPalette
  var selected = false
  func makeBody(configuration: Configuration) -> some View {
    Cell(configuration: configuration, palette: palette, selected: selected)
  }

  private struct Cell: View {
    let configuration: ButtonStyleConfiguration
    let palette: MacEmojiPalette
    let selected: Bool
    @State private var hovered = false
    var body: some View {
      configuration.label
        .frame(maxWidth: .infinity, minHeight: 36)
        .background(fill, in: RoundedRectangle(cornerRadius: 10 * 2 / 3))
        .overlay {
          if selected {
            RoundedRectangle(cornerRadius: 10 * 2 / 3)
              .strokeBorder(MacEmojiPalette.color(palette.background == 0xF7F7FA ? palette.accent : 0xF0F0F4), lineWidth: 2 * 2 / 3)
          }
        }
        .contentShape(RoundedRectangle(cornerRadius: 10 * 2 / 3))
        .onHover { hovered = $0 }
    }
    private var fill: Color {
      palette.cellFill(hovered: hovered || selected, isPressed: configuration.isPressed)
        .map(MacEmojiPalette.color) ?? .clear
    }
  }
}
