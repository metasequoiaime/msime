import SwiftUI
import UIKit
import XCTest

/// 设置项「?」弹出的气泡（`DesignInfoBubble`）。popover 按内容不给宽度时的理想尺寸定气泡大小，这里用 `ImageRenderer`（默认就是不给尺寸）量同一个理想尺寸：长说明的气泡要容得下折行后的每一行，短说明的气泡贴着文字。#10708 在 iPhone XR 上「语音标点」的说明只露出中间三行，就是理想高度按一行算出来的。
@MainActor
final class DesignInfoBubbleTests: XCTestCase {
  /// 与 `VoiceSettingsView.punctuationModeHint` 相同，是现有说明里最长的一类。
  private static let longHint = "识别结果上屏前怎么处理标点，对所有识别服务和 AI 润色后的结果都生效。智能添加：识别出什么标点就上屏什么；省略句末标点：「你好。」上屏为「你好」；省略所有标点：「天啊！他是谁？」上屏为「天啊他是谁」；空格代替标点：上屏为「天啊 他是谁」。小数点、时间里的冒号和引号括号不受影响"
  private static let shortHint = "离线识别，录音不离开设备"
  private static let font = UIFont.systemFont(ofSize: DesignInfoBubble.fontSize)
  /// 气泡左右各 14、上下各 12 的内边距。
  private static let horizontalPadding: CGFloat = 28
  private static let verticalPadding: CGFloat = 24

  func testLongHintBubbleIsTallEnoughForEveryWrappedLine() throws {
    let wrapped = (Self.longHint as NSString).boundingRect(
      with: CGSize(width: DesignInfoBubble.maxTextWidth, height: .greatestFiniteMagnitude),
      options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: [.font: Self.font], context: nil
    ).height
    XCTAssertGreaterThan(wrapped, Self.font.lineHeight * 5, "the fixture has to wrap to many lines to mean anything")

    let size = try idealSize(DesignInfoBubble(text: Self.longHint))
    XCTAssertEqual(size.width, DesignInfoBubble.maxTextWidth + Self.horizontalPadding, accuracy: 1)
    // 允许 SwiftUI 与 TextKit 的行高差一点，但不能少掉一整行。
    XCTAssertGreaterThan(size.height, wrapped + Self.verticalPadding - Self.font.lineHeight, "bubble \(size) cuts off wrapped lines")
  }

  func testShortHintBubbleHugsItsTextOnOneLine() throws {
    let size = try idealSize(DesignInfoBubble(text: Self.shortHint))
    let oneLine = (Self.shortHint as NSString).size(withAttributes: [.font: Self.font])
    XCTAssertLessThan(size.width, DesignInfoBubble.maxTextWidth + Self.horizontalPadding)
    XCTAssertGreaterThanOrEqual(size.width, ceil(oneLine.width) + Self.horizontalPadding)
    XCTAssertLessThan(size.height, Self.font.lineHeight * 2 + Self.verticalPadding, "a one-line hint must not wrap")
  }

  func testTextWidthNeverExceedsTheBubbleLimit() {
    XCTAssertEqual(DesignInfoBubble.textWidth(for: Self.longHint), DesignInfoBubble.maxTextWidth)
    XCTAssertLessThan(DesignInfoBubble.textWidth(for: Self.shortHint), DesignInfoBubble.maxTextWidth)
  }

  /// 不给尺寸时视图的理想大小，单位是点。
  private func idealSize<Content: View>(_ view: Content) throws -> CGSize {
    let renderer = ImageRenderer(content: view)
    renderer.scale = 1
    let image = try XCTUnwrap(renderer.uiImage)
    return image.size
  }
}
