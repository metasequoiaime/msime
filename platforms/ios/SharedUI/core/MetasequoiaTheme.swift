import SwiftUI
import UIKit

enum MetasequoiaTheme {
  // Fixed brand green (#2C7A4B). Use it for fills that carry white labels in both appearances (5.3:1); for text, icons and adaptive fills use `accent`.
  static let forest = Color(red: 44 / 255, green: 122 / 255, blue: 75 / 255)

  // ---- 季节令牌 ----
  // 强调色、页面色和卡片色跟随应用主题（AppThemePalette）：默认是水杉四季，季节随月份变化。每个 provider 按 trait 的明暗模式解析调色板，解析不出时退回经典绿色令牌。动态 provider 只在 trait 变化时重新解析，所以应用以 `AppThemePalette.version` 作为内容的 key，季节或所选主题变化时才会重绘。

  // 文字、图标、选中态和开关用的强调色：季节强调色，回退为经典 #2C7A4B / #5FBF84。
  static let accentUIColor = UIColor { traits in
    palette(traits)?.accent ?? forestUIColor.resolvedColor(with: traits)
  }
  static let accent = Color(uiColor: accentUIColor)
  // 强调色图标背后的着色底：强调色加 22（浅色）或 40（深色）的 alpha；经典值 rgba(44,122,75,.14) / rgba(95,191,132,.26)。
  static let accentSoft = Color(uiColor: UIColor { traits in
    if let palette = palette(traits) { return palette.accentSoft }
    return traits.userInterfaceStyle == .dark
      ? UIColor(red: 95 / 255, green: 191 / 255, blue: 132 / 255, alpha: 0.26)
      : UIColor(red: 44 / 255, green: 122 / 255, blue: 75 / 255, alpha: 0.14)
  })
  // `accent` 填充上的前景色：浅色下为白色；深色下用 Rust 的 mix(accent 25%, #000)，因为白色放在深色模式偏浅的强调色上对比度不到 2:1。
  static let onAccent = Color(uiColor: UIColor { traits in
    palette(traits)?.onAccent ?? onForestUIColor.resolvedColor(with: traits)
  })
  // 宿主应用里所有开关打开时的轨道色：强调色，取代早先的 iOS 绿。
  static let switchOn = accent
  // Grouped-list section headers (13pt): #6D6D72 / #8E8E93.
  static let groupTitle = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 142 / 255, green: 142 / 255, blue: 147 / 255, alpha: 1)
      : UIColor(red: 109 / 255, green: 109 / 255, blue: 114 / 255, alpha: 1)
  })
  // 页面背景：季节 bg；经典值 #F2F2F7 / #000。
  static let canvasUIColor = UIColor { traits in
    if let palette = palette(traits) { return palette.background }
    return traits.userInterfaceStyle == .dark ? .black : UIColor(red: 242 / 255, green: 242 / 255, blue: 247 / 255, alpha: 1)
  }
  static let canvas = Color(uiColor: canvasUIColor)
  // 卡片和行背景：季节 card；经典值 #FFF / #1C1C1E。
  static let surfaceUIColor = UIColor { traits in
    if let palette = palette(traits) { return palette.card }
    return traits.userInterfaceStyle == .dark ? UIColor(red: 28 / 255, green: 28 / 255, blue: 30 / 255, alpha: 1) : .white
  }
  static let surface = Color(uiColor: surfaceUIColor)
  // 细线和分隔线：浅色下用季节 hair（经典：系统分隔线色）；深色下固定为 rgba(84,84,88,.6)，按 iOS 设计稿覆盖 Rust 的深色 hair。
  static let hairUIColor = UIColor { traits in
    if traits.userInterfaceStyle == .dark { return UIColor(red: 84 / 255, green: 84 / 255, blue: 88 / 255, alpha: 0.6) }
    return palette(traits)?.hair ?? UIColor.separator.resolvedColor(with: traits)
  }
  static let hair = Color(uiColor: hairUIColor)

  // ---- 中性令牌（不随应用主题变化）----

  // 次要文字、数值和箭头：#8A8A8E / #8E8E93。
  static let subUIColor = UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 142 / 255, green: 142 / 255, blue: 147 / 255, alpha: 1)
      : UIColor(red: 138 / 255, green: 138 / 255, blue: 142 / 255, alpha: 1)
  }
  static let sub = Color(uiColor: subUIColor)
  // 退出登录这类破坏性操作，两种模式相同。
  static let danger = Color(red: 1, green: 59 / 255, blue: 48 / 255)
  // 分段控件轨道，rgba(118,118,128,.12) / .24。
  static let segBg = Color(uiColor: .tertiarySystemFill)
  // 按下时的行背景：rgba(0,0,0,.07) / rgba(255,255,255,.10)。
  static let pressFill = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark ? UIColor(white: 1, alpha: 0.10) : UIColor(white: 0, alpha: 0.07)
  })
  // 指针悬停时的行背景：rgba(0,0,0,.05) / rgba(255,255,255,.08)。
  static let hoverFill = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark ? UIColor(white: 1, alpha: 0.08) : UIColor(white: 0, alpha: 0.05)
  })

  // 设置标签页上分组卡片的圆角。
  static let cardRadius: CGFloat = 26
  // 社区、统计、我的三个标签页及其子页面上卡片的圆角。
  static let tabCardRadius: CGFloat = 10

  /// 设计稿里的 `aM(p)`：按 trait 的明暗模式取 color-mix(accent p%, card)，深色模式下百分比用 `dark ?? light`。统计条、热度档位、标志圆片和着色轨道都由它绘制。
  static func accentMix(_ light: Double, _ dark: Double? = nil) -> Color {
    Color(uiColor: mixUIColor(light, dark))
  }

  /// `accentMix` 的 UIKit 版本。
  static func mixUIColor(_ light: Double, _ dark: Double? = nil) -> UIColor {
    UIColor { traits in
      let percent = traits.userInterfaceStyle == .dark ? dark ?? light : light
      return AppThemePalette.mix(accentUIColor.resolvedColor(with: traits), percent, surfaceUIColor.resolvedColor(with: traits))
    }
  }

  private static func palette(_ traits: UITraitCollection) -> AppThemePalette.Resolved? {
    AppThemePalette.resolved(dark: traits.userInterfaceStyle == .dark)
  }

  static let needle = Color(red: 77 / 255, green: 138 / 255, blue: 114 / 255)
  static let mist = Color(red: 243 / 255, green: 247 / 255, blue: 245 / 255)
  static let cone = Color(red: 167 / 255, green: 103 / 255, blue: 59 / 255)
  static let ink = Color(red: 20 / 255, green: 35 / 255, blue: 29 / 255)

  // Text and glyphs drawn on top of forestUIColor. The two shades sit on opposite sides of the contrast line, so a single foreground fails one of them: white reads 5.3:1 on the light shade but 2.3:1 on the dark one, which is below even the large-text floor.
  static let onForestUIColor = UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 20 / 255, green: 35 / 255, blue: 29 / 255, alpha: 1)
      : .white
  }

  // 经典品牌强调色 #2C7A4B / #5FBF84：应用主题解析不出时作为 `accentUIColor` 的回退，也用在需要固定品牌绿的地方。
  static let forestUIColor = UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 95 / 255, green: 191 / 255, blue: 132 / 255, alpha: 1)
      : UIColor(red: 44 / 255, green: 122 / 255, blue: 75 / 255, alpha: 1)
  }

  static let coneUIColor = UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 214 / 255, green: 150 / 255, blue: 105 / 255, alpha: 1)
      : UIColor(red: 167 / 255, green: 103 / 255, blue: 59 / 255, alpha: 1)
  }

  static let keyboardBackground = UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 24 / 255, green: 30 / 255, blue: 27 / 255, alpha: 1)
      : UIColor(red: 232 / 255, green: 239 / 255, blue: 235 / 255, alpha: 1)
  }

  static let keyBackground = UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 48 / 255, green: 56 / 255, blue: 52 / 255, alpha: 1)
      : .white
  }

  /// `steps` 档明度梯度,给饼图和分布条用。
  ///
  /// 图表要的是相邻区块能分开,不是八种色相。一条梯度既做到这一点,又不会让统计页变成整个应用里唯一一处另有配色的地方。两端都从季节强调色推导：从 accent 走到 mix(accent 30%, card)。浅色下 accent 最深、卡片最浅，梯度由深到浅；深色下 accent 最亮、卡片最暗，同一个公式自然就反过来由亮到暗 —— 否则深色模式里最深的那一档和背景糊在一起。
  static func chartRamp(_ steps: Int) -> [Color] {
    (0..<steps).map { step in
      let t = steps > 1 ? Double(step) / Double(steps - 1) : 0
      return Color(uiColor: mixUIColor(100 - 70 * t))
    }
  }
}

struct MetasequoiaMark: Shape {
  func path(in rect: CGRect) -> Path {
    var path = Path()
    let centerX = rect.midX
    path.move(to: CGPoint(x: centerX, y: rect.minY + rect.height * 0.08))
    path.addLine(to: CGPoint(x: centerX, y: rect.maxY * 0.9))

    for level in 0..<4 {
      let y = rect.minY + rect.height * (0.25 + CGFloat(level) * 0.18)
      let reach = rect.width * (0.2 + CGFloat(level) * 0.08)
      path.move(to: CGPoint(x: centerX, y: y - rect.height * 0.11))
      path.addLine(to: CGPoint(x: centerX - reach, y: y))
      path.move(to: CGPoint(x: centerX, y: y - rect.height * 0.11))
      path.addLine(to: CGPoint(x: centerX + reach, y: y))
    }
    return path
  }
}
