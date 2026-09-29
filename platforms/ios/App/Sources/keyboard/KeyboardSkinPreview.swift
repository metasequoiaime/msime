import SwiftUI

// A display-only keyboard: no Engine session, document access, or input side effects.
struct KeyboardSkinPreview: View {
  let skin: KeyboardTheme
  let nineKey: Bool
  var layout: KeyboardGeometry = KeyboardLayoutPreference.geometry
  var heightAdjustment: Double = 0
  @Environment(\.colorScheme) private var colorScheme

  private func color(_ value: UIColor) -> Color {
    Color(uiColor: value.resolvedColor(with: UITraitCollection(userInterfaceStyle: colorScheme == .dark ? .dark : .light)))
  }

  var body: some View {
    KeyboardPreviewCanvas(referenceHeight: 260 + heightAdjustment) { keyboard }
      .accessibilityElement(children: .ignore)
      .accessibilityLabel("\(skin.title)，\(nineKey ? "9 键" : "26 键")完整键盘预览")
      .accessibilityIdentifier("fullKeyboardSkinPreview")
  }
  // Match the native nine-key sidebar proportions within the 390-point canvas.
  private var nineKeySidebarWidth: CGFloat { (390 - 14) * layout.sidebarRatio }

  private var keyboard: some View {
    VStack(spacing: layout.rowSpacing) {
      HStack(spacing: 10) {
        Text("ni hao").font(.caption).foregroundStyle(color(skin.accent))
        Text("你好").font(.subheadline.weight(.medium))
        Text("你号").font(.subheadline)
        Spacer(minLength: 0)
        if KeyboardLayoutPreference.voiceShortcutEnabled { Image(systemName: "waveform").font(.caption) }
        Text(nineKey ? "九键" : "全拼").font(.caption)

      }.padding(.horizontal, 6).frame(height: 32)
        .background(color(skin.keyBackground).opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
      if nineKey {
        HStack(spacing: 6) {
          VStack(spacing: 5) {
            ForEach(["，", "。", "？", "！"], id: \.self) { value in key(value) }
          }.frame(width: nineKeySidebarWidth)
          VStack(spacing: layout.rowSpacing) {
            row(["分词", "ABC", "DEF"])
            row(["GHI", "JKL", "MNO"])
            row(["PQRS", "TUV", "WXYZ"])
          }
          VStack(spacing: layout.rowSpacing) {
            key("⌫")
            key(".")
            key("0")
          }.frame(width: nineKeySidebarWidth)
        }.frame(maxHeight: .infinity)
      } else {
        VStack(spacing: layout.rowSpacing) {
          row(Array("qwertyuiop").map(String.init))
          row(Array("asdfghjkl").map(String.init)).padding(.horizontal, 376 * layout.letterInsetRatio)
          HStack(spacing: 5) {
            key("⇧").frame(width: 38)
            row(Array("zxcvbnm").map(String.init))
            key("⌫").frame(width: 38)
          }
        }.frame(maxHeight: .infinity)
      }
      HStack(spacing: 6) {
        if nineKey || layout.showsFullKeyboardSymbols { key("符").frame(width: 30) }
        key("123").frame(width: 38)
        if !nineKey { key("，").frame(width: 38) }
        key("空格")
        if layout.showsBottomLanguage { key("中/英").frame(width: 30) }
        key("换行", emphasized: true).frame(width: 52)
      }.frame(height: 44)
    }
    .foregroundStyle(color(skin.keyForeground))
    .padding(7).background(KeyboardSkinBackdrop(skin: skin))
    .clipShape(RoundedRectangle(cornerRadius: 12))

  }

  private func row(_ titles: [String]) -> some View {
    HStack(spacing: 5) {
      ForEach(titles, id: \.self) { key($0) }
    }
  }

  private func key(_ title: String, emphasized: Bool = false) -> some View {
    Text(title).font(.system(size: nineKey ? 13 : 15, weight: .medium, design: skin.usesMonospacedFont ? .monospaced : .default))
      .lineLimit(1).minimumScaleFactor(0.7)
      .frame(maxWidth: .infinity, maxHeight: .infinity)
      .foregroundStyle(color(emphasized ? skin.actionForeground : skin.keyForeground))
      .background(keySurface(emphasized: emphasized))
  }
  @ViewBuilder
  private func keySurface(emphasized: Bool = false) -> some View {
    if let design = skin.design {
      SkinKeySurface(design: design, action: emphasized)
    } else {
    RoundedRectangle(cornerRadius: skin.cornerRadius)
      .fill(color(emphasized ? skin.actionBackground : skin.keyBackground))
      .overlay(RoundedRectangle(cornerRadius: skin.cornerRadius).stroke(color(skin.borderColor), lineWidth: skin.borderWidth))
      .shadow(color: skin.hasShadow ? color(skin.shadowColor) : .clear, radius: skin.shadowRadius, y: skin.shadowOffset)
    }
  }
}

struct SkinDesignThumbnail: View {
  let skin: KeyboardTheme
  var body: some View {
    HStack(spacing: 4) {
      ForEach(["A", "S", "↵"], id: \.self) { title in
        Text(title).font(.system(size: 15, weight: .medium, design: skin.usesMonospacedFont ? .monospaced : .default))
          .foregroundStyle(Color(uiColor: title == "↵" ? skin.actionForeground : skin.keyForeground))
          .frame(width: 23, height: 32)
          .background {
            if let design = skin.design {
              SkinKeySurface(design: design, action: title == "↵", scale: 0.6)
            } else {
            RoundedRectangle(cornerRadius: skin.cornerRadius * 0.6)
              .fill(Color(uiColor: title == "↵" ? skin.actionBackground : skin.keyBackground))
              .overlay(RoundedRectangle(cornerRadius: skin.cornerRadius * 0.6)
                .stroke(Color(uiColor: skin.borderColor), lineWidth: skin.borderWidth))
              .shadow(color: skin.hasShadow ? Color(uiColor: skin.shadowColor) : .clear, radius: skin.shadowRadius, y: skin.shadowOffset)
            }
          }
      }
    }
    .padding(7).frame(width: 94, height: 56)
    .background(KeyboardSkinBackdrop(skin: skin))
    .clipShape(RoundedRectangle(cornerRadius: 9))
    .accessibilityHidden(true)
  }
}
