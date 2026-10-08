// 由 platforms/android/scripts/generate_keyboard_icons.py --swift 生成，不要手工编辑；改图标请改脚本里的 ICONS / IOS_ICONS / SWIFT_CASES 表再重新运行。
// 来源：设计令牌 §6.2（iOS 工具栏描边图标）、§6.3（按键描边图标）、§6.4（功能面板描边图标），viewBox 0 0 24 24；设计未给图标的几项是同网格的 Lucide 风格补充。椭圆弧已在脚本里转成三次贝塞尔。
import UIKit

/// 键盘绘制的全部矢量图标，路径数据取自设计稿的 24 单位网格。
///
/// 图标要么填充，要么以 `lineWidth`（viewBox 单位）描边，端点与拐角均为圆形。
enum KeyboardIcon: String, CaseIterable {
  /// design iOS toolbar 表情 (tbIcons)
  case toolbarEmoji
  /// design iOS toolbar 常用语 (tbIcons)
  case toolbarPhrase
  /// design iOS toolbar 剪贴板 (tbIcons)
  case toolbarClipboard
  /// design iOS toolbar 皮肤 (tbIcons)
  case toolbarSkin
  /// design iOS toolbar 输入方式 (tbIcons)
  case toolbarScheme
  /// design chevron-down (21dp)
  case collapse
  /// design candidate expand chevron (20dp)
  case candidateExpand
  /// design key shift
  case shift
  /// design key caps lock
  case capsLock
  /// design key backspace
  case backspace
  /// design key return
  case returnKey
  /// design space mic
  case mic
  /// design key emoji (123 layer)
  case emoji
  /// design panel 手写
  case handwriting
  /// design panel 词库
  case lexicon
  /// design panel 键盘高度
  case keyboardHeight
  /// design panel 设置
  case settings
  /// design panel 按键音
  case keySound
  /// design panel 振动
  case vibration
  /// design panel 单手模式
  case oneHand
  /// design panel 隐私模式
  case incognito
  /// design panel 反馈
  case feedback
  /// design panel 关于
  case about
  /// design on-state check badge
  case check
  /// Lucide-style sparkles (AI 回复与润色)
  case aiAssist
  /// Lucide-style laptop (本地输入)
  case localInput
  /// Lucide-style mic (语音结果)
  case voiceResult
  /// Lucide-style activity (振动强度)
  case vibrationStrength
  /// Lucide-style clipboard-list (剪贴板历史)
  case clipboardHistory
  /// design iOS toolbar 输入方式 (tbIcons)；与 `toolbarScheme` 同一轮廓
  case keyboardLayout
  /// Lucide-style plus (添加语言)
  case plus
  /// Lucide-style chevron-left (单手换边)
  case oneHandSwapLeft
  /// Lucide-style chevron-right (单手换到右侧)
  case oneHandSwapRight
  /// Lucide-style maximize-2 (退出单手)
  case oneHandExit

  /// 图标坐标空间的边长（SVG viewBox 0 0 24 24）。
  static let viewBox: CGFloat = 24

  /// 描边宽度，单位为 viewBox；填充图标为 0。
  var lineWidth: CGFloat {
    switch self {
    case .toolbarEmoji, .toolbarPhrase, .toolbarClipboard, .toolbarSkin, .toolbarScheme, .shift, .capsLock, .backspace, .returnKey, .mic, .emoji, .handwriting, .lexicon, .keyboardHeight, .settings, .keySound, .vibration, .oneHand, .incognito, .feedback, .about, .aiAssist, .localInput, .voiceResult, .vibrationStrength, .clipboardHistory, .keyboardLayout, .plus, .oneHandExit: return 1.7
    case .collapse, .candidateExpand, .oneHandSwapLeft, .oneHandSwapRight: return 1.8
    case .check: return 4
    }
  }

  /// 填充图形为真，以 `lineWidth` 描边的轮廓为假。
  var isFilled: Bool {
    false
  }

  /// 把图标等比缩放进 `rect` 并居中。每次调用都新建一条路径，调用方可以随意修改。
  func path(in rect: CGRect) -> UIBezierPath {
    let p = UIBezierPath()
    append(to: p)
    let scale = min(rect.width, rect.height) / Self.viewBox
    let side = Self.viewBox * scale
    p.apply(
      CGAffineTransform(translationX: rect.midX - side / 2, y: rect.midY - side / 2)
        .scaledBy(x: scale, y: scale))
    return p
  }

  /// 把图标画进边长 `pointSize` 的正方形：填充，或以 `lineWidth`（viewBox 单位，为 nil 时用图标自身宽度）圆端圆角描边。
  ///
  /// `color` 为 nil 时得到模板图像，跟随显示它的视图的 tint；否则保持 `color`。
  func image(pointSize: CGFloat, color: UIColor? = nil, lineWidth: CGFloat? = nil) -> UIImage {
    let bounds = CGRect(x: 0, y: 0, width: pointSize, height: pointSize)
    let rendered = UIGraphicsImageRenderer(bounds: bounds).image { _ in
      let p = path(in: bounds)
      (color ?? .black).set()
      if isFilled {
        p.fill()
      } else {
        p.lineWidth = (lineWidth ?? self.lineWidth) * pointSize / Self.viewBox
        p.lineCapStyle = .round
        p.lineJoinStyle = .round
        p.stroke()
      }
    }
    return rendered.withRenderingMode(color == nil ? .alwaysTemplate : .alwaysOriginal)
  }

  /// 把图标在 viewBox 坐标下的轮廓追加到 `p`。
  private func append(to p: UIBezierPath) {
    switch self {
    case .toolbarEmoji: Self.appendIosToolbarEmoji(p)
    case .toolbarPhrase: Self.appendIosToolbarPhrase(p)
    case .toolbarClipboard: Self.appendIosToolbarClipboard(p)
    case .toolbarSkin: Self.appendIosToolbarSkin(p)
    case .toolbarScheme: Self.appendIosToolbarScheme(p)
    case .collapse: Self.appendToolbarDismiss(p)
    case .candidateExpand: Self.appendChevron(p)
    case .shift: Self.appendShift(p)
    case .capsLock: Self.appendCapsLock(p)
    case .backspace: Self.appendBackspace(p)
    case .returnKey: Self.appendReturn(p)
    case .mic: Self.appendMic(p)
    case .emoji: Self.appendKeyEmoji(p)
    case .handwriting: Self.appendHandwriting(p)
    case .lexicon: Self.appendLexicon(p)
    case .keyboardHeight: Self.appendKeyboardHeight(p)
    case .settings: Self.appendSettings(p)
    case .keySound: Self.appendKeySound(p)
    case .vibration: Self.appendVibration(p)
    case .oneHand: Self.appendOneHand(p)
    case .incognito: Self.appendIncognito(p)
    case .feedback: Self.appendFeedback(p)
    case .about: Self.appendAbout(p)
    case .check: Self.appendCheck(p)
    case .aiAssist: Self.appendAiAssist(p)
    case .localInput: Self.appendLocalInput(p)
    case .voiceResult: Self.appendVoiceResult(p)
    case .vibrationStrength: Self.appendVibrationStrength(p)
    case .clipboardHistory: Self.appendClipboardHistory(p)
    case .keyboardLayout: Self.appendIosToolbarScheme(p)
    case .plus: Self.appendIosPlus(p)
    case .oneHandSwapLeft: Self.appendSwapSide(p)
    case .oneHandSwapRight: Self.appendIosSwapSideRight(p)
    case .oneHandExit: Self.appendExitOneHand(p)
    }
  }

  private static func appendIosToolbarEmoji(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 21))
    p.addCurve(to: CGPoint(x: 21, y: 12), controlPoint1: CGPoint(x: 16.9706, y: 21), controlPoint2: CGPoint(x: 21, y: 16.9706))
    p.addCurve(to: CGPoint(x: 12, y: 3), controlPoint1: CGPoint(x: 21, y: 7.0294), controlPoint2: CGPoint(x: 16.9706, y: 3))
    p.addCurve(to: CGPoint(x: 3, y: 12), controlPoint1: CGPoint(x: 7.0294, y: 3), controlPoint2: CGPoint(x: 3, y: 7.0294))
    p.addCurve(to: CGPoint(x: 12, y: 21), controlPoint1: CGPoint(x: 3, y: 16.9706), controlPoint2: CGPoint(x: 7.0294, y: 21))
    p.close()
    p.move(to: CGPoint(x: 8.5, y: 14))
    p.addCurve(to: CGPoint(x: 12, y: 15.8), controlPoint1: CGPoint(x: 8.5, y: 14), controlPoint2: CGPoint(x: 9.8, y: 15.8))
    p.addCurve(to: CGPoint(x: 15.5, y: 14), controlPoint1: CGPoint(x: 14.2, y: 15.8), controlPoint2: CGPoint(x: 15.5, y: 14))
    p.move(to: CGPoint(x: 9, y: 9.5))
    p.addLine(to: CGPoint(x: 9.01, y: 9.5))
    p.move(to: CGPoint(x: 15, y: 9.5))
    p.addLine(to: CGPoint(x: 15.01, y: 9.5))
  }

  private static func appendIosToolbarPhrase(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 5, y: 4))
    p.addLine(to: CGPoint(x: 19, y: 4))
    p.addCurve(to: CGPoint(x: 21, y: 6), controlPoint1: CGPoint(x: 20.1046, y: 4), controlPoint2: CGPoint(x: 21, y: 4.8954))
    p.addLine(to: CGPoint(x: 21, y: 15))
    p.addCurve(to: CGPoint(x: 19, y: 17), controlPoint1: CGPoint(x: 21, y: 16.1046), controlPoint2: CGPoint(x: 20.1046, y: 17))
    p.addLine(to: CGPoint(x: 9, y: 17))
    p.addLine(to: CGPoint(x: 4, y: 21))
    p.addLine(to: CGPoint(x: 4, y: 6))
    p.addCurve(to: CGPoint(x: 5, y: 4), controlPoint1: CGPoint(x: 3.8947, y: 5.1919), controlPoint2: CGPoint(x: 4.2903, y: 4.4007))
    p.close()
    p.move(to: CGPoint(x: 8.5, y: 9))
    p.addLine(to: CGPoint(x: 15.5, y: 9))
    p.move(to: CGPoint(x: 8.5, y: 12.5))
    p.addLine(to: CGPoint(x: 13, y: 12.5))
  }

  private static func appendIosToolbarClipboard(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 15, y: 4.5))
    p.addLine(to: CGPoint(x: 17, y: 4.5))
    p.addCurve(to: CGPoint(x: 19, y: 6.5), controlPoint1: CGPoint(x: 18.1046, y: 4.5), controlPoint2: CGPoint(x: 19, y: 5.3954))
    p.addLine(to: CGPoint(x: 19, y: 19))
    p.addCurve(to: CGPoint(x: 17, y: 21), controlPoint1: CGPoint(x: 19, y: 20.1046), controlPoint2: CGPoint(x: 18.1046, y: 21))
    p.addLine(to: CGPoint(x: 7, y: 21))
    p.addCurve(to: CGPoint(x: 5, y: 19), controlPoint1: CGPoint(x: 5.8954, y: 21), controlPoint2: CGPoint(x: 5, y: 20.1046))
    p.addLine(to: CGPoint(x: 5, y: 6.5))
    p.addCurve(to: CGPoint(x: 7, y: 4.5), controlPoint1: CGPoint(x: 5, y: 5.3954), controlPoint2: CGPoint(x: 5.8954, y: 4.5))
    p.addLine(to: CGPoint(x: 9, y: 4.5))
    p.move(to: CGPoint(x: 9.5, y: 3))
    p.addLine(to: CGPoint(x: 14.5, y: 3))
    p.addCurve(to: CGPoint(x: 15, y: 3.5), controlPoint1: CGPoint(x: 14.7761, y: 3), controlPoint2: CGPoint(x: 15, y: 3.2239))
    p.addLine(to: CGPoint(x: 15, y: 5.5))
    p.addCurve(to: CGPoint(x: 14.5, y: 6), controlPoint1: CGPoint(x: 15, y: 5.7761), controlPoint2: CGPoint(x: 14.7761, y: 6))
    p.addLine(to: CGPoint(x: 9.5, y: 6))
    p.addCurve(to: CGPoint(x: 9, y: 5.5), controlPoint1: CGPoint(x: 9.2239, y: 6), controlPoint2: CGPoint(x: 9, y: 5.7761))
    p.addLine(to: CGPoint(x: 9, y: 3.5))
    p.addCurve(to: CGPoint(x: 9.5, y: 3), controlPoint1: CGPoint(x: 9, y: 3.2239), controlPoint2: CGPoint(x: 9.2239, y: 3))
    p.close()
    p.move(to: CGPoint(x: 9, y: 11.5))
    p.addLine(to: CGPoint(x: 15, y: 11.5))
    p.move(to: CGPoint(x: 9, y: 15.5))
    p.addLine(to: CGPoint(x: 13, y: 15.5))
  }

  private static func appendIosToolbarSkin(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 3))
    p.addCurve(to: CGPoint(x: 3, y: 12), controlPoint1: CGPoint(x: 7.0294, y: 3), controlPoint2: CGPoint(x: 3, y: 7.0294))
    p.addCurve(to: CGPoint(x: 12, y: 21), controlPoint1: CGPoint(x: 3, y: 16.9706), controlPoint2: CGPoint(x: 7.0294, y: 21))
    p.addCurve(to: CGPoint(x: 13.7, y: 19.2), controlPoint1: CGPoint(x: 13.1, y: 21), controlPoint2: CGPoint(x: 13.7, y: 20.2))
    p.addCurve(to: CGPoint(x: 13.2, y: 17.9), controlPoint1: CGPoint(x: 13.7, y: 18.7), controlPoint2: CGPoint(x: 13.5, y: 18.3))
    p.addCurve(to: CGPoint(x: 12.8896, y: 15.9838), controlPoint1: CGPoint(x: 12.7315, y: 17.3766), controlPoint2: CGPoint(x: 12.6103, y: 16.6284))
    p.addCurve(to: CGPoint(x: 14.5, y: 14.9), controlPoint1: CGPoint(x: 13.1689, y: 15.3393), controlPoint2: CGPoint(x: 13.7977, y: 14.9161))
    p.addLine(to: CGPoint(x: 17, y: 14.9))
    p.addCurve(to: CGPoint(x: 21, y: 10.9), controlPoint1: CGPoint(x: 19.2091, y: 14.9), controlPoint2: CGPoint(x: 21, y: 13.1091))
    p.addCurve(to: CGPoint(x: 12, y: 3), controlPoint1: CGPoint(x: 21, y: 6.4), controlPoint2: CGPoint(x: 17, y: 3))
    p.close()
    p.move(to: CGPoint(x: 7.5, y: 11.5))
    p.addLine(to: CGPoint(x: 7.51, y: 11.5))
    p.move(to: CGPoint(x: 9.5, y: 7.5))
    p.addLine(to: CGPoint(x: 9.51, y: 7.5))
    p.move(to: CGPoint(x: 14.5, y: 7.5))
    p.addLine(to: CGPoint(x: 14.51, y: 7.5))
    p.move(to: CGPoint(x: 17, y: 11))
    p.addLine(to: CGPoint(x: 17.01, y: 11))
  }

  private static func appendIosToolbarScheme(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 4.5, y: 5.5))
    p.addLine(to: CGPoint(x: 19.5, y: 5.5))
    p.addCurve(to: CGPoint(x: 21.5, y: 7.5), controlPoint1: CGPoint(x: 20.6046, y: 5.5), controlPoint2: CGPoint(x: 21.5, y: 6.3954))
    p.addLine(to: CGPoint(x: 21.5, y: 16.5))
    p.addCurve(to: CGPoint(x: 19.5, y: 18.5), controlPoint1: CGPoint(x: 21.5, y: 17.6046), controlPoint2: CGPoint(x: 20.6046, y: 18.5))
    p.addLine(to: CGPoint(x: 4.5, y: 18.5))
    p.addCurve(to: CGPoint(x: 2.5, y: 16.5), controlPoint1: CGPoint(x: 3.3954, y: 18.5), controlPoint2: CGPoint(x: 2.5, y: 17.6046))
    p.addLine(to: CGPoint(x: 2.5, y: 7.5))
    p.addCurve(to: CGPoint(x: 4.5, y: 5.5), controlPoint1: CGPoint(x: 2.5, y: 6.3954), controlPoint2: CGPoint(x: 3.3954, y: 5.5))
    p.close()
    p.move(to: CGPoint(x: 6.5, y: 9.5))
    p.addLine(to: CGPoint(x: 6.51, y: 9.5))
    p.move(to: CGPoint(x: 9.5, y: 9.5))
    p.addLine(to: CGPoint(x: 9.51, y: 9.5))
    p.move(to: CGPoint(x: 12.5, y: 9.5))
    p.addLine(to: CGPoint(x: 12.51, y: 9.5))
    p.move(to: CGPoint(x: 15.5, y: 9.5))
    p.addLine(to: CGPoint(x: 15.51, y: 9.5))
    p.move(to: CGPoint(x: 17.5, y: 12.5))
    p.addLine(to: CGPoint(x: 17.51, y: 12.5))
    p.move(to: CGPoint(x: 6.5, y: 12.5))
    p.addLine(to: CGPoint(x: 6.51, y: 12.5))
    p.move(to: CGPoint(x: 8.5, y: 15))
    p.addLine(to: CGPoint(x: 15.5, y: 15))
  }

  private static func appendToolbarDismiss(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 6, y: 9))
    p.addLine(to: CGPoint(x: 12, y: 15))
    p.addLine(to: CGPoint(x: 18, y: 9))
  }

  private static func appendChevron(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 6, y: 9.5))
    p.addLine(to: CGPoint(x: 12, y: 15.5))
    p.addLine(to: CGPoint(x: 18, y: 9.5))
  }

  private static func appendShift(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 4.5))
    p.addLine(to: CGPoint(x: 4.5, y: 12.5))
    p.addLine(to: CGPoint(x: 8.5, y: 12.5))
    p.addLine(to: CGPoint(x: 8.5, y: 19))
    p.addLine(to: CGPoint(x: 15.5, y: 19))
    p.addLine(to: CGPoint(x: 15.5, y: 12.5))
    p.addLine(to: CGPoint(x: 19.5, y: 12.5))
    p.close()
  }

  private static func appendCapsLock(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 4.5))
    p.addLine(to: CGPoint(x: 4.5, y: 12.5))
    p.addLine(to: CGPoint(x: 8.5, y: 12.5))
    p.addLine(to: CGPoint(x: 8.5, y: 16))
    p.addLine(to: CGPoint(x: 15.5, y: 16))
    p.addLine(to: CGPoint(x: 15.5, y: 12.5))
    p.addLine(to: CGPoint(x: 19.5, y: 12.5))
    p.close()
    p.move(to: CGPoint(x: 8.5, y: 19.5))
    p.addLine(to: CGPoint(x: 15.5, y: 19.5))
  }

  private static func appendBackspace(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 9, y: 5.5))
    p.addLine(to: CGPoint(x: 19.5, y: 5.5))
    p.addCurve(to: CGPoint(x: 21, y: 7), controlPoint1: CGPoint(x: 20.3284, y: 5.5), controlPoint2: CGPoint(x: 21, y: 6.1716))
    p.addLine(to: CGPoint(x: 21, y: 17))
    p.addCurve(to: CGPoint(x: 19.5, y: 18.5), controlPoint1: CGPoint(x: 21, y: 17.8284), controlPoint2: CGPoint(x: 20.3284, y: 18.5))
    p.addLine(to: CGPoint(x: 9, y: 18.5))
    p.addLine(to: CGPoint(x: 3, y: 12))
    p.close()
    p.move(to: CGPoint(x: 11.5, y: 9.5))
    p.addLine(to: CGPoint(x: 16.5, y: 14.5))
    p.move(to: CGPoint(x: 16.5, y: 9.5))
    p.addLine(to: CGPoint(x: 11.5, y: 14.5))
  }

  private static func appendReturn(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 19, y: 6))
    p.addLine(to: CGPoint(x: 19, y: 11.5))
    p.addCurve(to: CGPoint(x: 17, y: 13.5), controlPoint1: CGPoint(x: 19, y: 12.6046), controlPoint2: CGPoint(x: 18.1046, y: 13.5))
    p.addLine(to: CGPoint(x: 6, y: 13.5))
    p.move(to: CGPoint(x: 9.5, y: 10))
    p.addLine(to: CGPoint(x: 6, y: 13.5))
    p.addLine(to: CGPoint(x: 9.5, y: 17))
  }

  private static func appendMic(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 5))
    p.addCurve(to: CGPoint(x: 14, y: 7), controlPoint1: CGPoint(x: 13.1046, y: 5), controlPoint2: CGPoint(x: 14, y: 5.8954))
    p.addLine(to: CGPoint(x: 14, y: 11))
    p.addCurve(to: CGPoint(x: 12, y: 13), controlPoint1: CGPoint(x: 14, y: 12.1046), controlPoint2: CGPoint(x: 13.1046, y: 13))
    p.addCurve(to: CGPoint(x: 10, y: 11), controlPoint1: CGPoint(x: 10.8954, y: 13), controlPoint2: CGPoint(x: 10, y: 12.1046))
    p.addLine(to: CGPoint(x: 10, y: 7))
    p.addCurve(to: CGPoint(x: 12, y: 5), controlPoint1: CGPoint(x: 10, y: 5.8954), controlPoint2: CGPoint(x: 10.8954, y: 5))
    p.close()
    p.move(to: CGPoint(x: 8, y: 10.5))
    p.addCurve(to: CGPoint(x: 12, y: 14.5), controlPoint1: CGPoint(x: 8, y: 12.7091), controlPoint2: CGPoint(x: 9.7909, y: 14.5))
    p.addCurve(to: CGPoint(x: 16, y: 10.5), controlPoint1: CGPoint(x: 14.2091, y: 14.5), controlPoint2: CGPoint(x: 16, y: 12.7091))
    p.move(to: CGPoint(x: 12, y: 14.5))
    p.addLine(to: CGPoint(x: 12, y: 17))
  }

  private static func appendKeyEmoji(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 20.5))
    p.addCurve(to: CGPoint(x: 20.5, y: 12), controlPoint1: CGPoint(x: 16.6944, y: 20.5), controlPoint2: CGPoint(x: 20.5, y: 16.6944))
    p.addCurve(to: CGPoint(x: 12, y: 3.5), controlPoint1: CGPoint(x: 20.5, y: 7.3056), controlPoint2: CGPoint(x: 16.6944, y: 3.5))
    p.addCurve(to: CGPoint(x: 3.5, y: 12), controlPoint1: CGPoint(x: 7.3056, y: 3.5), controlPoint2: CGPoint(x: 3.5, y: 7.3056))
    p.addCurve(to: CGPoint(x: 12, y: 20.5), controlPoint1: CGPoint(x: 3.5, y: 16.6944), controlPoint2: CGPoint(x: 7.3056, y: 20.5))
    p.close()
    p.move(to: CGPoint(x: 8.5, y: 14.2))
    p.addCurve(to: CGPoint(x: 12, y: 16), controlPoint1: CGPoint(x: 8.5, y: 14.2), controlPoint2: CGPoint(x: 9.7, y: 16))
    p.addCurve(to: CGPoint(x: 15.5, y: 14.2), controlPoint1: CGPoint(x: 14.3, y: 16), controlPoint2: CGPoint(x: 15.5, y: 14.2))
    p.move(to: CGPoint(x: 9, y: 9.8))
    p.addLine(to: CGPoint(x: 9.01, y: 9.8))
    p.move(to: CGPoint(x: 15, y: 9.8))
    p.addLine(to: CGPoint(x: 15.01, y: 9.8))
  }

  private static func appendHandwriting(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 20))
    p.addLine(to: CGPoint(x: 21, y: 20))
    p.move(to: CGPoint(x: 16.4, y: 3.6))
    p.addCurve(to: CGPoint(x: 18.449, y: 3.051), controlPoint1: CGPoint(x: 16.9359, y: 3.0641), controlPoint2: CGPoint(x: 17.717, y: 2.8548))
    p.addCurve(to: CGPoint(x: 19.949, y: 4.551), controlPoint1: CGPoint(x: 19.1811, y: 3.2471), controlPoint2: CGPoint(x: 19.7529, y: 3.8189))
    p.addCurve(to: CGPoint(x: 19.4, y: 6.6), controlPoint1: CGPoint(x: 20.1452, y: 5.283), controlPoint2: CGPoint(x: 19.9359, y: 6.0641))
    p.addLine(to: CGPoint(x: 7, y: 19))
    p.addLine(to: CGPoint(x: 3, y: 20))
    p.addLine(to: CGPoint(x: 4, y: 16))
    p.close()
  }

  private static func appendLexicon(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 2, y: 4))
    p.addLine(to: CGPoint(x: 8, y: 4))
    p.addCurve(to: CGPoint(x: 12, y: 8), controlPoint1: CGPoint(x: 10.2091, y: 4), controlPoint2: CGPoint(x: 12, y: 5.7909))
    p.addLine(to: CGPoint(x: 12, y: 21))
    p.addCurve(to: CGPoint(x: 9, y: 18), controlPoint1: CGPoint(x: 12, y: 19.3431), controlPoint2: CGPoint(x: 10.6569, y: 18))
    p.addLine(to: CGPoint(x: 2, y: 18))
    p.close()
    p.move(to: CGPoint(x: 22, y: 4))
    p.addLine(to: CGPoint(x: 16, y: 4))
    p.addCurve(to: CGPoint(x: 12, y: 8), controlPoint1: CGPoint(x: 13.7909, y: 4), controlPoint2: CGPoint(x: 12, y: 5.7909))
    p.addLine(to: CGPoint(x: 12, y: 21))
    p.addCurve(to: CGPoint(x: 15, y: 18), controlPoint1: CGPoint(x: 12, y: 19.3431), controlPoint2: CGPoint(x: 13.3431, y: 18))
    p.addLine(to: CGPoint(x: 22, y: 18))
    p.close()
  }

  private static func appendKeyboardHeight(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 22))
    p.addLine(to: CGPoint(x: 12, y: 16))
    p.move(to: CGPoint(x: 12, y: 8))
    p.addLine(to: CGPoint(x: 12, y: 2))
    p.move(to: CGPoint(x: 4, y: 12))
    p.addLine(to: CGPoint(x: 2, y: 12))
    p.move(to: CGPoint(x: 10, y: 12))
    p.addLine(to: CGPoint(x: 8, y: 12))
    p.move(to: CGPoint(x: 16, y: 12))
    p.addLine(to: CGPoint(x: 14, y: 12))
    p.move(to: CGPoint(x: 22, y: 12))
    p.addLine(to: CGPoint(x: 20, y: 12))
    p.move(to: CGPoint(x: 15, y: 19))
    p.addLine(to: CGPoint(x: 12, y: 22))
    p.addLine(to: CGPoint(x: 9, y: 19))
    p.move(to: CGPoint(x: 15, y: 5))
    p.addLine(to: CGPoint(x: 12, y: 2))
    p.addLine(to: CGPoint(x: 9, y: 5))
  }

  private static func appendSettings(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 20, y: 7))
    p.addLine(to: CGPoint(x: 11, y: 7))
    p.move(to: CGPoint(x: 14, y: 17))
    p.addLine(to: CGPoint(x: 5, y: 17))
    p.move(to: CGPoint(x: 17, y: 20))
    p.addCurve(to: CGPoint(x: 20, y: 17), controlPoint1: CGPoint(x: 18.6569, y: 20), controlPoint2: CGPoint(x: 20, y: 18.6569))
    p.addCurve(to: CGPoint(x: 17, y: 14), controlPoint1: CGPoint(x: 20, y: 15.3431), controlPoint2: CGPoint(x: 18.6569, y: 14))
    p.addCurve(to: CGPoint(x: 14, y: 17), controlPoint1: CGPoint(x: 15.3431, y: 14), controlPoint2: CGPoint(x: 14, y: 15.3431))
    p.addCurve(to: CGPoint(x: 17, y: 20), controlPoint1: CGPoint(x: 14, y: 18.6569), controlPoint2: CGPoint(x: 15.3431, y: 20))
    p.close()
    p.move(to: CGPoint(x: 7, y: 10))
    p.addCurve(to: CGPoint(x: 10, y: 7), controlPoint1: CGPoint(x: 8.6569, y: 10), controlPoint2: CGPoint(x: 10, y: 8.6569))
    p.addCurve(to: CGPoint(x: 7, y: 4), controlPoint1: CGPoint(x: 10, y: 5.3431), controlPoint2: CGPoint(x: 8.6569, y: 4))
    p.addCurve(to: CGPoint(x: 4, y: 7), controlPoint1: CGPoint(x: 5.3431, y: 4), controlPoint2: CGPoint(x: 4, y: 5.3431))
    p.addCurve(to: CGPoint(x: 7, y: 10), controlPoint1: CGPoint(x: 4, y: 8.6569), controlPoint2: CGPoint(x: 5.3431, y: 10))
    p.close()
  }

  private static func appendKeySound(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 11, y: 4.7))
    p.addCurve(to: CGPoint(x: 10.5693, y: 4.0437), controlPoint1: CGPoint(x: 11.0042, y: 4.4137), controlPoint2: CGPoint(x: 10.8336, y: 4.1538))
    p.addCurve(to: CGPoint(x: 9.8, y: 4.2), controlPoint1: CGPoint(x: 10.305, y: 3.9336), controlPoint2: CGPoint(x: 10.0003, y: 3.9955))
    p.addLine(to: CGPoint(x: 6.4, y: 7.6))
    p.addCurve(to: CGPoint(x: 5.4, y: 8), controlPoint1: CGPoint(x: 6.1333, y: 7.8613), controlPoint2: CGPoint(x: 5.7733, y: 8.0053))
    p.addLine(to: CGPoint(x: 3, y: 8))
    p.addCurve(to: CGPoint(x: 2, y: 9), controlPoint1: CGPoint(x: 2.4477, y: 8), controlPoint2: CGPoint(x: 2, y: 8.4477))
    p.addLine(to: CGPoint(x: 2, y: 15))
    p.addCurve(to: CGPoint(x: 3, y: 16), controlPoint1: CGPoint(x: 2, y: 15.5523), controlPoint2: CGPoint(x: 2.4477, y: 16))
    p.addLine(to: CGPoint(x: 5.4, y: 16))
    p.addCurve(to: CGPoint(x: 6.4, y: 16.4), controlPoint1: CGPoint(x: 5.7733, y: 15.9947), controlPoint2: CGPoint(x: 6.1333, y: 16.1387))
    p.addLine(to: CGPoint(x: 9.8, y: 19.8))
    p.addCurve(to: CGPoint(x: 10.5693, y: 19.9563), controlPoint1: CGPoint(x: 10.0003, y: 20.0045), controlPoint2: CGPoint(x: 10.305, y: 20.0664))
    p.addCurve(to: CGPoint(x: 11, y: 19.3), controlPoint1: CGPoint(x: 10.8336, y: 19.8462), controlPoint2: CGPoint(x: 11.0042, y: 19.5863))
    p.close()
    p.move(to: CGPoint(x: 16, y: 9))
    p.addCurve(to: CGPoint(x: 16, y: 15), controlPoint1: CGPoint(x: 17.3333, y: 10.7778), controlPoint2: CGPoint(x: 17.3333, y: 13.2222))
    p.move(to: CGPoint(x: 19.4, y: 18.4))
    p.addCurve(to: CGPoint(x: 22.0723, y: 12), controlPoint1: CGPoint(x: 21.11, y: 16.7093), controlPoint2: CGPoint(x: 22.0723, y: 14.4047))
    p.addCurve(to: CGPoint(x: 19.4, y: 5.6), controlPoint1: CGPoint(x: 22.0723, y: 9.5953), controlPoint2: CGPoint(x: 21.11, y: 7.2907))
  }

  private static func appendVibration(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 2, y: 8))
    p.addLine(to: CGPoint(x: 4, y: 10))
    p.addLine(to: CGPoint(x: 2, y: 12))
    p.addLine(to: CGPoint(x: 4, y: 14))
    p.addLine(to: CGPoint(x: 2, y: 16))
    p.move(to: CGPoint(x: 22, y: 8))
    p.addLine(to: CGPoint(x: 20, y: 10))
    p.addLine(to: CGPoint(x: 22, y: 12))
    p.addLine(to: CGPoint(x: 20, y: 14))
    p.addLine(to: CGPoint(x: 22, y: 16))
    p.move(to: CGPoint(x: 9, y: 5))
    p.addLine(to: CGPoint(x: 15, y: 5))
    p.addCurve(to: CGPoint(x: 16, y: 6), controlPoint1: CGPoint(x: 15.5523, y: 5), controlPoint2: CGPoint(x: 16, y: 5.4477))
    p.addLine(to: CGPoint(x: 16, y: 18))
    p.addCurve(to: CGPoint(x: 15, y: 19), controlPoint1: CGPoint(x: 16, y: 18.5523), controlPoint2: CGPoint(x: 15.5523, y: 19))
    p.addLine(to: CGPoint(x: 9, y: 19))
    p.addCurve(to: CGPoint(x: 8, y: 18), controlPoint1: CGPoint(x: 8.4477, y: 19), controlPoint2: CGPoint(x: 8, y: 18.5523))
    p.addLine(to: CGPoint(x: 8, y: 6))
    p.addCurve(to: CGPoint(x: 9, y: 5), controlPoint1: CGPoint(x: 8, y: 5.4477), controlPoint2: CGPoint(x: 8.4477, y: 5))
    p.close()
  }

  private static func appendOneHand(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 18, y: 11))
    p.addLine(to: CGPoint(x: 18, y: 6))
    p.addCurve(to: CGPoint(x: 16, y: 4), controlPoint1: CGPoint(x: 18, y: 4.8954), controlPoint2: CGPoint(x: 17.1046, y: 4))
    p.addCurve(to: CGPoint(x: 14, y: 6), controlPoint1: CGPoint(x: 14.8954, y: 4), controlPoint2: CGPoint(x: 14, y: 4.8954))
    p.move(to: CGPoint(x: 14, y: 10))
    p.addLine(to: CGPoint(x: 14, y: 4))
    p.addCurve(to: CGPoint(x: 12, y: 2), controlPoint1: CGPoint(x: 14, y: 2.8954), controlPoint2: CGPoint(x: 13.1046, y: 2))
    p.addCurve(to: CGPoint(x: 10, y: 4), controlPoint1: CGPoint(x: 10.8954, y: 2), controlPoint2: CGPoint(x: 10, y: 2.8954))
    p.addLine(to: CGPoint(x: 10, y: 6))
    p.move(to: CGPoint(x: 10, y: 10.5))
    p.addLine(to: CGPoint(x: 10, y: 6))
    p.addCurve(to: CGPoint(x: 8, y: 4), controlPoint1: CGPoint(x: 10, y: 4.8954), controlPoint2: CGPoint(x: 9.1046, y: 4))
    p.addCurve(to: CGPoint(x: 6, y: 6), controlPoint1: CGPoint(x: 6.8954, y: 4), controlPoint2: CGPoint(x: 6, y: 4.8954))
    p.addLine(to: CGPoint(x: 6, y: 14))
    p.move(to: CGPoint(x: 18, y: 8))
    p.addCurve(to: CGPoint(x: 20, y: 6), controlPoint1: CGPoint(x: 18, y: 6.8954), controlPoint2: CGPoint(x: 18.8954, y: 6))
    p.addCurve(to: CGPoint(x: 22, y: 8), controlPoint1: CGPoint(x: 21.1046, y: 6), controlPoint2: CGPoint(x: 22, y: 6.8954))
    p.addLine(to: CGPoint(x: 22, y: 14))
    p.addCurve(to: CGPoint(x: 14, y: 22), controlPoint1: CGPoint(x: 22, y: 18.4183), controlPoint2: CGPoint(x: 18.4183, y: 22))
    p.addLine(to: CGPoint(x: 12, y: 22))
    p.addCurve(to: CGPoint(x: 6, y: 19.66), controlPoint1: CGPoint(x: 9.2, y: 22), controlPoint2: CGPoint(x: 7.5, y: 21.14))
    p.addLine(to: CGPoint(x: 2.4, y: 16.06))
    p.addCurve(to: CGPoint(x: 2.4727, y: 13.303), controlPoint1: CGPoint(x: 1.6854, y: 15.2686), controlPoint2: CGPoint(x: 1.7174, y: 14.0556))
    p.addCurve(to: CGPoint(x: 5.23, y: 13.24), controlPoint1: CGPoint(x: 3.2281, y: 12.5503), controlPoint2: CGPoint(x: 4.4411, y: 12.5226))
    p.addLine(to: CGPoint(x: 7, y: 15))
  }

  private static func appendIncognito(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 20, y: 13))
    p.addCurve(to: CGPoint(x: 12.34, y: 21.95), controlPoint1: CGPoint(x: 20, y: 18), controlPoint2: CGPoint(x: 16.5, y: 20.5))
    p.addCurve(to: CGPoint(x: 11.67, y: 21.94), controlPoint1: CGPoint(x: 12.1222, y: 22.0238), controlPoint2: CGPoint(x: 11.8855, y: 22.0203))
    p.addCurve(to: CGPoint(x: 4, y: 13), controlPoint1: CGPoint(x: 7.5, y: 20.5), controlPoint2: CGPoint(x: 4, y: 18))
    p.addLine(to: CGPoint(x: 4, y: 6))
    p.addCurve(to: CGPoint(x: 5, y: 5), controlPoint1: CGPoint(x: 4, y: 5.4477), controlPoint2: CGPoint(x: 4.4477, y: 5))
    p.addCurve(to: CGPoint(x: 11.24, y: 2.28), controlPoint1: CGPoint(x: 7, y: 5), controlPoint2: CGPoint(x: 9.5, y: 3.8))
    p.addCurve(to: CGPoint(x: 12.76, y: 2.28), controlPoint1: CGPoint(x: 11.6777, y: 1.9061), controlPoint2: CGPoint(x: 12.3223, y: 1.9061))
    p.addCurve(to: CGPoint(x: 19, y: 5), controlPoint1: CGPoint(x: 14.51, y: 3.81), controlPoint2: CGPoint(x: 17, y: 5))
    p.addCurve(to: CGPoint(x: 20, y: 6), controlPoint1: CGPoint(x: 19.5523, y: 5), controlPoint2: CGPoint(x: 20, y: 5.4477))
    p.close()
    p.move(to: CGPoint(x: 9, y: 12))
    p.addLine(to: CGPoint(x: 11, y: 14))
    p.addLine(to: CGPoint(x: 15, y: 10))
  }

  private static func appendFeedback(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 21, y: 15))
    p.addCurve(to: CGPoint(x: 19, y: 17), controlPoint1: CGPoint(x: 21, y: 16.1046), controlPoint2: CGPoint(x: 20.1046, y: 17))
    p.addLine(to: CGPoint(x: 7, y: 17))
    p.addLine(to: CGPoint(x: 3, y: 21))
    p.addLine(to: CGPoint(x: 3, y: 5))
    p.addCurve(to: CGPoint(x: 5, y: 3), controlPoint1: CGPoint(x: 3, y: 3.8954), controlPoint2: CGPoint(x: 3.8954, y: 3))
    p.addLine(to: CGPoint(x: 19, y: 3))
    p.addCurve(to: CGPoint(x: 21, y: 5), controlPoint1: CGPoint(x: 20.1046, y: 3), controlPoint2: CGPoint(x: 21, y: 3.8954))
    p.close()
    p.move(to: CGPoint(x: 13, y: 8))
    p.addLine(to: CGPoint(x: 7, y: 8))
    p.move(to: CGPoint(x: 17, y: 12))
    p.addLine(to: CGPoint(x: 7, y: 12))
  }

  private static func appendAbout(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 22))
    p.addCurve(to: CGPoint(x: 22, y: 12), controlPoint1: CGPoint(x: 17.5228, y: 22), controlPoint2: CGPoint(x: 22, y: 17.5228))
    p.addCurve(to: CGPoint(x: 12, y: 2), controlPoint1: CGPoint(x: 22, y: 6.4772), controlPoint2: CGPoint(x: 17.5228, y: 2))
    p.addCurve(to: CGPoint(x: 2, y: 12), controlPoint1: CGPoint(x: 6.4772, y: 2), controlPoint2: CGPoint(x: 2, y: 6.4772))
    p.addCurve(to: CGPoint(x: 12, y: 22), controlPoint1: CGPoint(x: 2, y: 17.5228), controlPoint2: CGPoint(x: 6.4772, y: 22))
    p.close()
    p.move(to: CGPoint(x: 12, y: 16))
    p.addLine(to: CGPoint(x: 12, y: 12))
    p.move(to: CGPoint(x: 12, y: 8))
    p.addLine(to: CGPoint(x: 12.01, y: 8))
  }

  private static func appendCheck(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 5, y: 12.5))
    p.addLine(to: CGPoint(x: 9.5, y: 17))
    p.addLine(to: CGPoint(x: 19, y: 7.5))
  }

  private static func appendAiAssist(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 3))
    p.addLine(to: CGPoint(x: 13.9, y: 8.1))
    p.addLine(to: CGPoint(x: 19, y: 10))
    p.addLine(to: CGPoint(x: 13.9, y: 11.9))
    p.addLine(to: CGPoint(x: 12, y: 17))
    p.addLine(to: CGPoint(x: 10.1, y: 11.9))
    p.addLine(to: CGPoint(x: 5, y: 10))
    p.addLine(to: CGPoint(x: 10.1, y: 8.1))
    p.close()
    p.move(to: CGPoint(x: 19, y: 15))
    p.addLine(to: CGPoint(x: 19.8, y: 17.2))
    p.addLine(to: CGPoint(x: 22, y: 18))
    p.addLine(to: CGPoint(x: 19.8, y: 18.8))
    p.addLine(to: CGPoint(x: 19, y: 21))
    p.addLine(to: CGPoint(x: 18.2, y: 18.8))
    p.addLine(to: CGPoint(x: 16, y: 18))
    p.addLine(to: CGPoint(x: 18.2, y: 17.2))
    p.close()
  }

  private static func appendLocalInput(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 4, y: 6))
    p.addCurve(to: CGPoint(x: 6, y: 4), controlPoint1: CGPoint(x: 4, y: 4.8954), controlPoint2: CGPoint(x: 4.8954, y: 4))
    p.addLine(to: CGPoint(x: 18, y: 4))
    p.addCurve(to: CGPoint(x: 20, y: 6), controlPoint1: CGPoint(x: 19.1046, y: 4), controlPoint2: CGPoint(x: 20, y: 4.8954))
    p.addLine(to: CGPoint(x: 20, y: 15))
    p.addLine(to: CGPoint(x: 4, y: 15))
    p.close()
    p.move(to: CGPoint(x: 2, y: 19))
    p.addLine(to: CGPoint(x: 22, y: 19))
  }

  private static func appendVoiceResult(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 9, y: 5))
    p.addCurve(to: CGPoint(x: 12, y: 2), controlPoint1: CGPoint(x: 9, y: 3.3431), controlPoint2: CGPoint(x: 10.3431, y: 2))
    p.addCurve(to: CGPoint(x: 15, y: 5), controlPoint1: CGPoint(x: 13.6569, y: 2), controlPoint2: CGPoint(x: 15, y: 3.3431))
    p.addLine(to: CGPoint(x: 15, y: 11))
    p.addCurve(to: CGPoint(x: 12, y: 14), controlPoint1: CGPoint(x: 15, y: 12.6569), controlPoint2: CGPoint(x: 13.6569, y: 14))
    p.addCurve(to: CGPoint(x: 9, y: 11), controlPoint1: CGPoint(x: 10.3431, y: 14), controlPoint2: CGPoint(x: 9, y: 12.6569))
    p.close()
    p.move(to: CGPoint(x: 5, y: 10))
    p.addCurve(to: CGPoint(x: 12, y: 17), controlPoint1: CGPoint(x: 5, y: 13.866), controlPoint2: CGPoint(x: 8.134, y: 17))
    p.addCurve(to: CGPoint(x: 19, y: 10), controlPoint1: CGPoint(x: 15.866, y: 17), controlPoint2: CGPoint(x: 19, y: 13.866))
    p.move(to: CGPoint(x: 12, y: 17))
    p.addLine(to: CGPoint(x: 12, y: 21))
    p.move(to: CGPoint(x: 8, y: 21))
    p.addLine(to: CGPoint(x: 16, y: 21))
  }

  private static func appendVibrationStrength(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 22, y: 12))
    p.addLine(to: CGPoint(x: 18, y: 12))
    p.addLine(to: CGPoint(x: 15, y: 21))
    p.addLine(to: CGPoint(x: 9, y: 3))
    p.addLine(to: CGPoint(x: 6, y: 12))
    p.addLine(to: CGPoint(x: 2, y: 12))
  }

  private static func appendClipboardHistory(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 9, y: 2))
    p.addLine(to: CGPoint(x: 15, y: 2))
    p.addCurve(to: CGPoint(x: 16, y: 3), controlPoint1: CGPoint(x: 15.5523, y: 2), controlPoint2: CGPoint(x: 16, y: 2.4477))
    p.addLine(to: CGPoint(x: 16, y: 5))
    p.addCurve(to: CGPoint(x: 15, y: 6), controlPoint1: CGPoint(x: 16, y: 5.5523), controlPoint2: CGPoint(x: 15.5523, y: 6))
    p.addLine(to: CGPoint(x: 9, y: 6))
    p.addCurve(to: CGPoint(x: 8, y: 5), controlPoint1: CGPoint(x: 8.4477, y: 6), controlPoint2: CGPoint(x: 8, y: 5.5523))
    p.addLine(to: CGPoint(x: 8, y: 3))
    p.addCurve(to: CGPoint(x: 9, y: 2), controlPoint1: CGPoint(x: 8, y: 2.4477), controlPoint2: CGPoint(x: 8.4477, y: 2))
    p.close()
    p.move(to: CGPoint(x: 16, y: 4))
    p.addLine(to: CGPoint(x: 18, y: 4))
    p.addCurve(to: CGPoint(x: 20, y: 6), controlPoint1: CGPoint(x: 19.1046, y: 4), controlPoint2: CGPoint(x: 20, y: 4.8954))
    p.addLine(to: CGPoint(x: 20, y: 20))
    p.addCurve(to: CGPoint(x: 18, y: 22), controlPoint1: CGPoint(x: 20, y: 21.1046), controlPoint2: CGPoint(x: 19.1046, y: 22))
    p.addLine(to: CGPoint(x: 6, y: 22))
    p.addCurve(to: CGPoint(x: 4, y: 20), controlPoint1: CGPoint(x: 4.8954, y: 22), controlPoint2: CGPoint(x: 4, y: 21.1046))
    p.addLine(to: CGPoint(x: 4, y: 6))
    p.addCurve(to: CGPoint(x: 6, y: 4), controlPoint1: CGPoint(x: 4, y: 4.8954), controlPoint2: CGPoint(x: 4.8954, y: 4))
    p.addLine(to: CGPoint(x: 8, y: 4))
    p.move(to: CGPoint(x: 12, y: 11))
    p.addLine(to: CGPoint(x: 16, y: 11))
    p.move(to: CGPoint(x: 12, y: 16))
    p.addLine(to: CGPoint(x: 16, y: 16))
    p.move(to: CGPoint(x: 8, y: 11))
    p.addLine(to: CGPoint(x: 8.01, y: 11))
    p.move(to: CGPoint(x: 8, y: 16))
    p.addLine(to: CGPoint(x: 8.01, y: 16))
  }

  private static func appendIosPlus(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 12, y: 5))
    p.addLine(to: CGPoint(x: 12, y: 19))
    p.move(to: CGPoint(x: 5, y: 12))
    p.addLine(to: CGPoint(x: 19, y: 12))
  }

  private static func appendSwapSide(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 15, y: 18))
    p.addLine(to: CGPoint(x: 9, y: 12))
    p.addLine(to: CGPoint(x: 15, y: 6))
  }

  private static func appendIosSwapSideRight(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 9, y: 18))
    p.addLine(to: CGPoint(x: 15, y: 12))
    p.addLine(to: CGPoint(x: 9, y: 6))
  }

  private static func appendExitOneHand(_ p: UIBezierPath) {
    p.move(to: CGPoint(x: 15, y: 3))
    p.addLine(to: CGPoint(x: 21, y: 3))
    p.addLine(to: CGPoint(x: 21, y: 9))
    p.move(to: CGPoint(x: 9, y: 21))
    p.addLine(to: CGPoint(x: 3, y: 21))
    p.addLine(to: CGPoint(x: 3, y: 15))
    p.move(to: CGPoint(x: 21, y: 3))
    p.addLine(to: CGPoint(x: 14, y: 10))
    p.move(to: CGPoint(x: 3, y: 21))
    p.addLine(to: CGPoint(x: 10, y: 14))
  }
}
