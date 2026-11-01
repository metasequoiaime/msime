import SwiftUI

/// 键盘预览在按键上方画什么。
enum KeyboardPreviewTopStrip: Equatable {
  /// 输入中的候选行，和键盘打字时显示的一样。
  case candidates
  /// 空闲时的工具栏：品牌圆片，依次是这些项目，最后是收起箭头。
  case toolbar([KeyboardPreviewToolbarItem])
  /// 完全不画顶栏，用于不需要保留键盘高度的预览。
  case hidden
  /// 不画工具栏内容，但保留 iOS 键盘稳定高度所需的顶栏空间。
  case reserved
}

/// 仅用于展示的键盘，不创建引擎会话，也不访问输入文档。字母层底行（26 键、14/18 键、9 键含右侧大回车）和顶部数字行按用户的「按键布局」画（`keyLayout`、`KeyboardPreviewLetterLayer`），与键盘对得上；不做高亮。
struct KeyboardSkinPreview: View {
  let skin: KeyboardTheme
  let nineKey: Bool
  /// 14 键或 18 键的字母网格；nil 画 26 键字母或九键。
  var letterGrid: LetterKeyGrid?
  /// 字母网格第三排左端画「分词」还是「符」：五笔 14 键没有音节可分，键盘上这个键总是「符」。
  var gridSplitsSyllables: Bool = true
  var layout: KeyboardGeometry = KeyboardLayoutPreference.geometry
  var heightAdjustment: Double = 0
  var topStrip: KeyboardPreviewTopStrip = .candidates
  /// 「候选栏大小」的倍数：顶栏随它变矮或变高，按键不变；工具栏图标只随它缩小，与键盘的 `KeyboardToolbarButton.contentScale` 一致（#10137）。
  var stripScale: CGFloat = 1
  /// 在 q–p 键的上角画数字提示，和设计稿的键盘缩略图一样。
  var letterHints: Bool = false
  /// 字母层底行和顶部数字行按的「按键布局」（底行样式、「符」与「123」互换、顶部数字行）。默认读 App Group 镜像，即这台设备上用户自己的选择：皮肤、工具栏和 AI 皮肤各页想让人看到皮肤装到自己键盘上的样子，所以都按它画；键盘页按正在用的方案校正后传进来。中/英键跟 `layout.showsBottomLanguage`。
  var keyLayout: TouchKeyLayoutStyle = TouchKeyLayoutStyle.mirrored
  /// 九键双拼字母层空格后面的 0。
  var nineKeyZero: Bool = false
  @Environment(\.colorScheme) private var colorScheme

  private func color(_ value: UIColor) -> Color {
    Color(uiColor: resolved(value))
  }

  private func resolved(_ value: UIColor) -> UIColor {
    value.resolvedColor(with: UITraitCollection(userInterfaceStyle: colorScheme == .dark ? .dark : .light))
  }

  var body: some View {
    KeyboardPreviewCanvas(referenceHeight: canvasHeight) { keyboard }
      .accessibilityElement(children: .ignore)
      .accessibilityLabel("\(skin.title)，\(layoutTitle)完整键盘预览")
      .accessibilityIdentifier("fullKeyboardSkinPreview")
  }

  /// 390 点宽时的画布高度。带工具栏和无顶栏两种变体是设计稿的键盘缩略图（390 × 292，去掉工具栏为 234），按键下方以主屏指示条区域收尾；候选行变体保持原先的 260，不带该区域。
  private var referenceHeight: CGFloat {
    switch topStrip {
    case .candidates: 260 - Self.candidateStripHeight * (1 - stripScale)
    case .toolbar: 292 - Self.toolbarStripHeight * (1 - stripScale)
    case .hidden: 234
    case .reserved: 292 - Self.toolbarStripHeight * (1 - stripScale)
    }
  }

  /// 画布高度：设计稿高度加上用户拖出的高度调整；画顶部数字行时再加一排字母行和一道行距，字母键的大小因此不变，就像键盘打开数字行时整块变高一排。
  private var canvasHeight: CGFloat {
    let height: CGFloat = referenceHeight + heightAdjustment
    guard letterLayer.numberRow else { return height }
    let spacing = CGFloat(layout.rowSpacing)
    let strip: CGFloat = switch topStrip {
    case .candidates: Self.candidateStripHeight * stripScale + spacing
    case .toolbar, .reserved: Self.toolbarStripHeight * stripScale + spacing
    case .hidden: 0
    }
    let padding = 7 + (drawsHomeIndicator ? Self.homeIndicatorHeight : 7)
    // 底行以上三排字母行和它们之间两道行距的高度。
    let letters = height - padding - strip - spacing - Self.bottomRowHeight
    return height + max((letters - 2 * spacing) / 3, 0) + spacing
  }

  /// 设计稿缩略图里两种顶栏的高度；缩放顶栏时画布高度同步增减这一段，按键大小因此不变。
  private static let candidateStripHeight: CGFloat = 32
  private static let toolbarStripHeight: CGFloat = 46
  private static let homeIndicatorHeight: CGFloat = 24
  private static let bottomRowHeight: CGFloat = 44
  /// 按键区宽度：画布按 390 点宽画，左右各留 7 点。
  private static let keysWidth: CGFloat = 390 - 14

  /// 工具栏图标和标志只随顶栏缩小、不随它放大。
  private var toolbarContentScale: CGFloat { min(1, stripScale) }

  private var drawsHomeIndicator: Bool { topStrip != .candidates }

  private var layoutTitle: String { nineKey ? "9 键" : letterGrid?.title ?? "26 键" }

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
    case .reserved: Color.clear.frame(height: Self.toolbarStripHeight * stripScale)
    }
  }

  private var candidateStrip: some View {
    HStack(spacing: 10) {
      Text("ni hao").font(.caption).foregroundStyle(color(skin.accent))
      Text("你好").font(.subheadline.weight(.medium))
      Text("你号").font(.subheadline)
      Spacer(minLength: 0)
      Text(nineKey ? "九键" : letterGrid?.title ?? "全拼").font(.caption)

    }.padding(.horizontal, 6).frame(height: Self.candidateStripHeight * stripScale)
      .background(color(skin.keyBackground).opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
  }

  /// 用皮肤配色画的空闲工具栏：品牌圆片，各项目等宽排列，最后是收起箭头。
  private func toolbarStrip(_ items: [KeyboardPreviewToolbarItem]) -> some View {
    HStack(spacing: 0) {
      brandMark.frame(maxWidth: .infinity)
      ForEach(Array(items.enumerated()), id: \.offset) { _, item in
        Group {
          if let symbol = item.symbol {
            Image(systemName: symbol).font(.system(size: 17 * toolbarContentScale))
          } else {
            Text(item.character).font(.system(size: 16 * toolbarContentScale, weight: .medium))
          }
        }
        .frame(maxWidth: .infinity)
      }
      Image(systemName: "chevron.down").font(.system(size: 15 * toolbarContentScale, weight: .semibold))
        .frame(maxWidth: .infinity)
    }
    .lineLimit(1)
    .frame(height: Self.toolbarStripHeight * stripScale)
  }

  /// 键盘品牌键：固定配色的水杉标志本身就是外框，不再垫圆片；占位仍是原来 22 点的圆片，标志按原来的 13 点画。
  private var brandMark: some View {
    MSIMELogoMark()
      .frame(width: 13 * toolbarContentScale, height: 13 * toolbarContentScale)
      .frame(width: 22 * toolbarContentScale, height: 22 * toolbarContentScale)
  }

  /// 按键下方系统画主屏指示条的区域，指示条用按键色。
  private var homeIndicator: some View {
    Capsule().fill(color(skin.keyForeground).opacity(0.85))
      .frame(width: 134, height: 5)
      .frame(maxWidth: .infinity).frame(height: Self.homeIndicatorHeight)
  }

  /// 字母层的框架、顶部数字行和底行，按用户在「按键布局」里选的样式（`keyLayout`）和中/英键设置排，与键盘取自同一张表（`KeyboardPreviewLetterLayer`）。
  private var letterLayer: KeyboardPreviewLetterLayer {
    KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid != nil, style: keyLayout,
                               language: layout.showsBottomLanguage, nineKeyZero: nineKeyZero)
  }

  /// 带工具栏或不带顶栏的 26 键与 14/18 键缩略图照设计稿画底行键面：空格上是麦克风和方案名，回车是图标；候选行变体和九键写「空格」「换行」。
  private var miniatureBottomRow: Bool { drawsHomeIndicator && !nineKey }

  @ViewBuilder
  private var keys: some View {
    let layer = letterLayer
    if layer.tallReturn {
      nineKeyTallReturn(layer)
    } else {
      letterBlock(layer)
      bottomRow(layer.bottomRow, width: Self.keysWidth)
    }
  }

  /// 底行以上的按键：九键是符号栏、九宫格和右列，其余是（顶部数字行和）字母行。
  @ViewBuilder
  private func letterBlock(_ layer: KeyboardPreviewLetterLayer) -> some View {
    if nineKey {
      HStack(spacing: 6) {
        nineKeySidebar(layer.nineKeySidebar)
        nineKeyGrid
        nineKeyControls(layer.nineKeyControls)
      }.frame(maxHeight: .infinity)
    } else if letterGrid == .eighteenKey {
      // 第一、三排七个等宽键（第三排两端的分词和 ⌫ 与字母键同宽），第二排六个键平分整行，与键盘一样从同一左右边界铺开。
      GeometryReader { proxy in
        let unit = (proxy.size.width - 5 * 6) / 7
        let middle = (proxy.size.width - 5 * 5) / 6
        VStack(spacing: layout.rowSpacing) {
          if layer.numberRow { numberRow }
          ForEach(Array(EighteenKeyLayout.rows.enumerated()), id: \.offset) { index, keys in
            HStack(spacing: 5) {
              if index == EighteenKeyLayout.rows.count - 1 { key("分词", function: true).frame(width: unit) }
              ForEach(keys.map(\.face), id: \.self) { face in key(face).frame(width: index == 1 ? middle : unit) }
              if index == EighteenKeyLayout.rows.count - 1 { key("⌫", function: true).frame(width: unit) }
            }
            .frame(maxWidth: .infinity)
          }
        }
      }.frame(maxHeight: .infinity)
    } else if letterGrid == .fourteenKey {
      VStack(spacing: layout.rowSpacing) {
        if layer.numberRow { numberRow }
        ForEach(Array(FourteenKeyLayout.rows.enumerated()), id: \.offset) { index, keys in
          if index == FourteenKeyLayout.rows.count - 1 {
            HStack(spacing: 5) {
              key(gridSplitsSyllables ? "分词" : "符", function: true).frame(width: 38)
              row(keys.map(\.face))
              key("⌫", function: true).frame(width: 38)
            }
          } else {
            row(keys.map(\.face))
          }
        }
      }.frame(maxHeight: .infinity)
    } else {
      VStack(spacing: layout.rowSpacing) {
        if layer.numberRow { numberRow }
        row(Array("qwertyuiop").map(String.init), hints: letterHints ? Array("1234567890").map(String.init) : [])
        row(Array("asdfghjkl").map(String.init)).padding(.horizontal, 376 * layout.letterInsetRatio)
        HStack(spacing: 5) {
          key("⇧", function: true).frame(width: 38)
          row(Array("zxcvbnm").map(String.init))
          key("⌫", function: true).frame(width: 38)
        }
      }.frame(maxHeight: .infinity)
    }
  }

  /// 「按键布局」的顶部数字行：一排等宽的 1 到 0，与热力图同一张表。
  private var numberRow: some View {
    row(TypingKeyHeatmap.numberRow.map(\.face))
  }

  /// 九键左侧的标点栏，标点自上而下各占一格。
  private func nineKeySidebar(_ marks: [String]) -> some View {
    VStack(spacing: 5) {
      ForEach(Array(marks.enumerated()), id: \.offset) { _, mark in key(mark) }
    }.frame(width: nineKeySidebarWidth)
  }

  // Same frame as the keyboard's pinyin grid: key 1 opens symbols, the right column holds ⌫, the syllable separator and the closing mark.
  private var nineKeyGrid: some View {
    VStack(spacing: layout.rowSpacing) {
      row(["@#", "ABC", "DEF"])
      row(["GHI", "JKL", "MNO"])
      row(["PQRS", "TUV", "WXYZ"])
    }
  }

  /// 九键右列在九宫格旁的几格，都画成功能键。
  private func nineKeyControls(_ titles: [String]) -> some View {
    VStack(spacing: layout.rowSpacing) {
      ForEach(Array(titles.enumerated()), id: \.offset) { _, title in key(title, function: true) }
    }.frame(width: nineKeySidebarWidth)
  }

  /// 九键「右侧大回车」：符号栏下方是 符，九宫格下方是底行中间几格，右列 ⌫ 和分词下面的回车从第三行一直伸到底行，与键盘和「按键布局」页的画法相同。整块占九宫格加底行的高度，回车的高度是一排九宫格、一道行距和底行之和。
  private func nineKeyTallReturn(_ layer: KeyboardPreviewLetterLayer) -> some View {
    let side = nineKeySidebarWidth
    let gridWidth = Self.keysWidth - side * 2 - 6 * 2
    let spacing = CGFloat(layout.rowSpacing)
    return GeometryReader { proxy in
      let rowHeight = max((proxy.size.height - Self.bottomRowHeight - spacing * 3) / 3, 0)
      HStack(alignment: .top, spacing: 6) {
        VStack(spacing: spacing) {
          nineKeySidebar(layer.nineKeySidebar)
          if let symbols = layer.bottomRow.first { bottomKey(symbols.slot).frame(height: Self.bottomRowHeight) }
        }.frame(width: side)
        VStack(spacing: spacing) {
          nineKeyGrid
          bottomRow(Array(layer.bottomRow.dropFirst().dropLast()), width: gridWidth)
        }.frame(width: gridWidth)
        VStack(spacing: spacing) {
          ForEach(Array(layer.nineKeyControls.enumerated()), id: \.offset) { _, title in
            key(title, function: true).frame(height: rowHeight)
          }
          if let enter = layer.bottomRow.last {
            bottomKey(enter.slot).frame(height: rowHeight + spacing + Self.bottomRowHeight)
          }
        }.frame(width: side)
      }
    }.frame(maxHeight: .infinity)
  }

  /// 一排按份额分宽的底行键：`width` 扣掉键距后按 `TouchBottomRowLayout` 的份额分给各键。
  private func bottomRow(_ keys: [TouchBottomRowLayout.Key], width: CGFloat) -> some View {
    let spacing: CGFloat = miniatureBottomRow ? 5 : 6
    let total = keys.reduce(0) { $0 + $1.weight }
    let unit = total > 0 ? max(width - spacing * CGFloat(max(keys.count - 1, 0)), 0) / total : 0
    return HStack(spacing: spacing) {
      ForEach(Array(keys.enumerated()), id: \.offset) { _, key in bottomKey(key.slot).frame(width: unit * key.weight) }
    }
    .frame(height: Self.bottomRowHeight)
  }

  /// 底行一格的键面，与键盘一致：符、123、表情键和 中 是功能键，回车用操作色。
  @ViewBuilder
  private func bottomKey(_ slot: TouchBottomRowLayout.Slot) -> some View {
    switch slot {
    case .symbols: key("符", function: true)
    case .layer: key("123", function: true)
    case .globe: symbolKey("globe", function: true)
    case .emoji: symbolKey("face.smiling", function: true)
    case .comma: key("，")
    case .period: key("。")
    case .zero: key("0")
    case .language: key("中", function: true)
    case .space:
      if miniatureBottomRow {
        ZStack {
          keySurface()
          HStack(spacing: 3) {
            Image(systemName: "mic.fill").font(.system(size: 11))
            Text(letterGrid?.title ?? "全拼").font(.system(size: 11))
          }
          .foregroundStyle(color(skin.secondary))
        }
      } else {
        key("空格")
      }
    case .returnKey:
      if miniatureBottomRow { symbolKey("return", emphasized: true) } else { key("换行", emphasized: true) }
    }
  }

  /// 画图标的按键，底色规则同 `key`。
  private func symbolKey(_ name: String, emphasized: Bool = false, function: Bool = false) -> some View {
    ZStack {
      keySurface(emphasized: emphasized, function: function)
      Image(systemName: name).font(.system(size: 15, weight: .medium))
        .foregroundStyle(color(emphasized ? skin.actionForeground : skin.keyForeground))
    }
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
