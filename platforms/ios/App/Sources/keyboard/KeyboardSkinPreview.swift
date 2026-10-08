import SwiftUI

/// 键盘预览在按键上方画什么。
enum KeyboardPreviewTopStrip: Equatable {
  /// 输入中的候选行，和键盘打字时显示的一样。
  case candidates
  /// 空闲时的工具栏：品牌圆片，依次是这些项目，最后是收起箭头。
  case toolbar([KeyboardPreviewToolbarItem])
  /// 完全不画顶栏。
  case hidden
}

/// 键盘预览里的一个工具栏按钮。图标与键盘自己的工具栏一致；字符类项目画单个字符而不是符号。
enum KeyboardPreviewToolbarItem: String, CaseIterable {
  case emoji, phrases, clipboard, skin, scheme, ai, characterSet, fullwidth, punctuation

  /// 该项目的 SF Symbol；画成字符的项目为 nil。
  var symbol: String? {
    switch self {
    case .emoji: "face.smiling"
    case .phrases: "text.bubble"
    case .clipboard: "doc.on.clipboard"
    case .skin: "paintpalette"
    case .scheme: "keyboard"
    case .ai: "sparkles"
    case .characterSet, .fullwidth, .punctuation: nil
    }
  }

  /// 没有符号的项目所画的字符。
  var character: String {
    switch self {
    case .characterSet: "简"
    case .fullwidth: "全"
    case .punctuation: "，"
    default: ""
    }
  }
}

// A display-only keyboard: no Engine session, document access, or input side effects.
struct KeyboardSkinPreview: View {
  let skin: KeyboardTheme
  let nineKey: Bool
  var layout: KeyboardGeometry = KeyboardLayoutPreference.geometry
  var heightAdjustment: Double = 0
  var topStrip: KeyboardPreviewTopStrip = .candidates
  /// 在 q–p 键的上角画数字提示，和设计稿的键盘缩略图一样。
  var letterHints: Bool = false
  @Environment(\.colorScheme) private var colorScheme

  private func color(_ value: UIColor) -> Color {
    Color(uiColor: resolved(value))
  }

  private func resolved(_ value: UIColor) -> UIColor {
    value.resolvedColor(with: UITraitCollection(userInterfaceStyle: colorScheme == .dark ? .dark : .light))
  }

  var body: some View {
    KeyboardPreviewCanvas(referenceHeight: referenceHeight + heightAdjustment) { keyboard }
      .accessibilityElement(children: .ignore)
      .accessibilityLabel("\(skin.title)，\(nineKey ? "9 键" : "26 键")完整键盘预览")
      .accessibilityIdentifier("fullKeyboardSkinPreview")
  }

  /// 390 点宽时的画布高度。带工具栏和无顶栏两种变体是设计稿的键盘缩略图（390 × 292，去掉工具栏为 234），按键下方以主屏指示条区域收尾；候选行变体保持原先的 260，不带该区域。
  private var referenceHeight: CGFloat {
    switch topStrip {
    case .candidates: 260
    case .toolbar: 292
    case .hidden: 234
    }
  }

  private var drawsHomeIndicator: Bool { topStrip != .candidates }

  // Match the native nine-key sidebar proportions within the 390-point canvas.
  private var nineKeySidebarWidth: CGFloat { (390 - 14) * layout.sidebarRatio }

  private var keyboard: some View {
    VStack(spacing: 0) {
      VStack(spacing: layout.rowSpacing) {
        if topStrip != .hidden { strip }
        keys
      }
      .padding(.horizontal, 7).padding(.top, 7).padding(.bottom, drawsHomeIndicator ? 0 : 7)
      if drawsHomeIndicator { homeIndicator }
    }
    .foregroundStyle(color(skin.keyForeground))
    .background(KeyboardSkinBackdrop(skin: skin))
    .clipShape(RoundedRectangle(cornerRadius: 12))
  }

  @ViewBuilder
  private var strip: some View {
    switch topStrip {
    case .candidates: candidateStrip
    case .toolbar(let items): toolbarStrip(items)
    case .hidden: EmptyView()
    }
  }

  private var candidateStrip: some View {
    HStack(spacing: 10) {
      Text("ni hao").font(.caption).foregroundStyle(color(skin.accent))
      Text("你好").font(.subheadline.weight(.medium))
      Text("你号").font(.subheadline)
      Spacer(minLength: 0)
      if KeyboardLayoutPreference.voiceShortcutEnabled { Image(systemName: "waveform").font(.caption) }
      Text(nineKey ? "九键" : "全拼").font(.caption)

    }.padding(.horizontal, 6).frame(height: 32)
      .background(color(skin.keyBackground).opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
  }

  /// 用皮肤配色画的空闲工具栏：品牌圆片，各项目等宽排列，最后是收起箭头。
  private func toolbarStrip(_ items: [KeyboardPreviewToolbarItem]) -> some View {
    HStack(spacing: 0) {
      brandDisc.frame(maxWidth: .infinity)
      ForEach(Array(items.enumerated()), id: \.offset) { _, item in
        Group {
          if let symbol = item.symbol {
            Image(systemName: symbol).font(.system(size: 17))
          } else {
            Text(item.character).font(.system(size: 16, weight: .medium))
          }
        }
        .frame(maxWidth: .infinity)
      }
      Image(systemName: "chevron.down").font(.system(size: 15, weight: .semibold)).frame(maxWidth: .infinity)
    }
    .lineLimit(1)
    .frame(height: 46)
  }

  /// 22 点圆片上的应用标志，画法同 AppMarkDisc 但取皮肤配色：圆片用 mix(accent 浅色 14% / 深色 22%, 按键色)，白色描边下的标志框用 mix(accent 82%, black)。
  private var brandDisc: some View {
    let accent = resolved(skin.accent)
    let disc = AppThemePalette.mix(accent, colorScheme == .dark ? 22 : 14, resolved(skin.keyBackground).withAlphaComponent(1))
    let frame = AppThemePalette.mix(accent, 82, .black)
    let markSize: CGFloat = 13
    let scale = MSIMELogo.scale(for: CGRect(x: 0, y: 0, width: markSize, height: markSize))
    return ZStack {
      Circle().fill(Color(uiColor: disc))
      ZStack {
        MSIMELogoFrame().fill(Color(uiColor: frame))
        MSIMELogoStroke().stroke(.white, style: StrokeStyle(lineWidth: MSIMELogo.strokeWidth * scale, lineCap: .round, lineJoin: .round))
      }
      .frame(width: markSize, height: markSize)
    }
    .frame(width: 22, height: 22)
  }

  /// 按键下方系统画主屏指示条的区域，指示条用按键色。
  private var homeIndicator: some View {
    Capsule().fill(color(skin.keyForeground).opacity(0.85))
      .frame(width: 134, height: 5)
      .frame(maxWidth: .infinity).frame(height: 24)
  }

  @ViewBuilder
  private var keys: some View {
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
          key("⌫", function: true)
          key(".")
          key("0")
        }.frame(width: nineKeySidebarWidth)
      }.frame(maxHeight: .infinity)
    } else {
      VStack(spacing: layout.rowSpacing) {
        row(Array("qwertyuiop").map(String.init), hints: letterHints ? Array("1234567890").map(String.init) : [])
        row(Array("asdfghjkl").map(String.init)).padding(.horizontal, 376 * layout.letterInsetRatio)
        HStack(spacing: 5) {
          key("⇧", function: true).frame(width: 38)
          row(Array("zxcvbnm").map(String.init))
          key("⌫", function: true).frame(width: 38)
        }
      }.frame(maxHeight: .infinity)
    }
    if drawsHomeIndicator && !nineKey {
      miniatureBottomRow
    } else {
      HStack(spacing: 6) {
        if nineKey || layout.showsFullKeyboardSymbols { key("符", function: true).frame(width: 30) }
        key("123", function: true).frame(width: 38)
        if !nineKey { key("，").frame(width: 38) }
        key("空格")
        if layout.showsBottomLanguage { key("中/英", function: true).frame(width: 30) }
        key("换行", emphasized: true).frame(width: 52)
      }.frame(height: 44)
    }
  }

  /// 设计稿缩略图的 26 键底行：123 / 中 / ， / 带麦克风和方案名的空格键 / 。 / 回车，宽度比例沿用键盘自身的权重（1.25 / 1.05 / 1 / 4 / 1 / 1.9）。
  private var miniatureBottomRow: some View {
    GeometryReader { proxy in
      let spacing: CGFloat = 5
      let unit = (proxy.size.width - spacing * 5) / 10.2
      HStack(spacing: spacing) {
        key("123", function: true).frame(width: unit * 1.25)
        key("中", function: true).frame(width: unit * 1.05)
        key("，").frame(width: unit)
        ZStack {
          keySurface()
          HStack(spacing: 3) {
            Image(systemName: "mic.fill").font(.system(size: 11))
            Text("全拼").font(.system(size: 11))
          }
          .foregroundStyle(color(skin.secondary))
        }
        .frame(width: unit * 4)
        key("。").frame(width: unit)
        ZStack {
          keySurface(emphasized: true)
          Image(systemName: "return").font(.system(size: 15, weight: .medium))
            .foregroundStyle(color(skin.actionForeground))
        }
        .frame(width: unit * 1.9)
      }
    }
    .frame(height: 44)
  }

  /// 一行等宽按键；`hints` 每个键各有一个时，把角标提示和对应标题配对。
  private func row(_ titles: [String], hints: [String] = []) -> some View {
    HStack(spacing: 5) {
      ForEach(Array(titles.enumerated()), id: \.offset) { index, title in
        key(title, hint: index < hints.count ? hints[index] : nil)
      }
    }
  }

  /// 一个按键。`function` 标记 shift、删除、123、中等功能键，键盘用 `functionKeyBackground`（设计稿的 `kb.spec`）填充它们，而不是字母键颜色；`emphasized` 是用操作色的回车键。
  private func key(_ title: String, emphasized: Bool = false, function: Bool = false, hint: String? = nil) -> some View {
    Text(title).font(.system(size: nineKey ? 13 : 15, weight: .medium, design: skin.usesMonospacedFont ? .monospaced : .default))
      .lineLimit(1).minimumScaleFactor(0.7)
      .frame(maxWidth: .infinity, maxHeight: .infinity)
      .foregroundStyle(color(emphasized ? skin.actionForeground : skin.keyForeground))
      .background(keySurface(emphasized: emphasized, function: function))
      .overlay(alignment: .topTrailing) {
        if let hint {
          Text(hint).font(.system(size: 9)).foregroundStyle(color(skin.secondary))
            .padding(.top, 2).padding(.trailing, 3)
        }
      }
  }
  @ViewBuilder
  private func keySurface(emphasized: Bool = false, function: Bool = false) -> some View {
    if let design = skin.design {
      // 键盘设计里的功能键和字母键画法相同，正如 `functionKeyBackground` 所规定。
      SkinKeySurface(design: design, action: emphasized)
    } else {
    RoundedRectangle(cornerRadius: skin.cornerRadius)
      .fill(color(emphasized ? skin.actionBackground : function ? skin.functionKeyBackground : skin.keyBackground))
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
