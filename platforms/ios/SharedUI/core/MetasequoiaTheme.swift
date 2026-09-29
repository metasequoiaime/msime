import SwiftUI
import UIKit

enum MetasequoiaTheme {
  // Fixed brand green (#2C7A4B). Use it for fills that carry white labels in both appearances (5.3:1); for text, icons and adaptive fills use `accent`.
  static let forest = Color(red: 44 / 255, green: 122 / 255, blue: 75 / 255)
  // Adaptive brand accent for text, icons and selected states: #2C7A4B in light, #5FBF84 in dark.
  static let accent = Color(uiColor: forestUIColor)
  // Tinted background behind accent glyphs (rgba(44,122,75,.14) / rgba(95,191,132,.26)).
  static let accentSoft = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 95 / 255, green: 191 / 255, blue: 132 / 255, alpha: 0.26)
      : UIColor(red: 44 / 255, green: 122 / 255, blue: 75 / 255, alpha: 0.14)
  })
  // Foreground on an `accent` fill. White fails on the dark-mode accent (2.3:1), so dark mode uses ink instead (7.2:1).
  static let onAccent = Color(uiColor: onForestUIColor)
  // iOS system switch green used for every toggle in the host app.
  static let switchOn = Color(red: 52 / 255, green: 199 / 255, blue: 89 / 255)
  // Grouped-list section headers (13pt): #6D6D72 / #8E8E93.
  static let groupTitle = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 142 / 255, green: 142 / 255, blue: 147 / 255, alpha: 1)
      : UIColor(red: 109 / 255, green: 109 / 255, blue: 114 / 255, alpha: 1)
  })
  // Grouped page background: #F2F2F7 / #000.
  static let canvas = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? .black
      : UIColor(red: 242 / 255, green: 242 / 255, blue: 247 / 255, alpha: 1)
  })
  // Grouped card background: #FFF / #1C1C1E.
  static let surface = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 28 / 255, green: 28 / 255, blue: 30 / 255, alpha: 1)
      : .white
  })
  // Corner radius shared by grouped cards and extra-page cards on iOS.
  static let cardRadius: CGFloat = 26
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
  /// 图表要的是相邻区块能分开,不是八种色相。一条梯度既做到这一点,又不会让统计页变成整个应用里唯一一处另有配色的地方。两端在浅色下由深到浅,深色下反过来 —— 否则深色模式里最深的那一档和背景糊在一起。
  static func chartRamp(_ steps: Int) -> [Color] {
    let light: ((Int, Int, Int), (Int, Int, Int)) = ((44, 122, 75), (174, 214, 187))
    let dark: ((Int, Int, Int), (Int, Int, Int)) = ((140, 215, 165), (38, 90, 58))
    return (0..<steps).map { step in
      let t = steps > 1 ? CGFloat(step) / CGFloat(steps - 1) : 0
      return Color(uiColor: UIColor { traits in
        let (from, to) = traits.userInterfaceStyle == .dark ? dark : light
        let channel = { (a: Int, b: Int) in (CGFloat(a) + (CGFloat(b) - CGFloat(a)) * t) / 255 }
        return UIColor(red: channel(from.0, to.0), green: channel(from.1, to.1),
                       blue: channel(from.2, to.2), alpha: 1)
      })
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
