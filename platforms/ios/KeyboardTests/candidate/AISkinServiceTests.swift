import Foundation
import XCTest
import UIKit

final class AISkinServiceTests: XCTestCase {
  /// 多行输入框里的换行和制表符折成一个空格再交给 Rust（Rust 拒收一切控制字符）；500 个字的上限按 Unicode 标量计，与 Rust 相同，超出的在调 Rust 之前就拒绝。方案的解析和校验在 Rust（`msime_client_ai_skin_generate`），由 host-api 的测试覆盖。
  func testMultilinePromptIsFoldedBeforeRust() throws {
    let folded = try XCTUnwrap(AISkinService.requestPrompt("  竹林里的熊猫 \r\n\n  茶会\t夜色\n"))
    XCTAssertEqual(folded, "竹林里的熊猫 茶会 夜色")
    XCTAssertNil(AISkinService.requestPrompt(" \n\t "))
    XCTAssertNil(AISkinService.requestPrompt("竹林\u{0007}茶会"))
    // 一个家庭 emoji 是一个字形、七个 Unicode 标量：100 个就是 700 个标量，字形数没超 500，Rust 也会拒绝。
    let family = String(repeating: "👨‍👩‍👧‍👦", count: 100)
    XCTAssertLessThanOrEqual(family.count, 500)
    XCTAssertNil(AISkinService.requestPrompt(family))
    let limit = String(repeating: "景", count: AISkinService.maximumPromptScalars)
    XCTAssertEqual(AISkinService.requestPrompt(limit), limit)
    XCTAssertNil(AISkinService.requestPrompt(limit + "景"))
  }
  @MainActor
  func testKeyStylesSurviveSavingAndRenderDistinctSurfaces() throws {
    var images = Set<Data>()
    for shape in SkinKeyShape.allCases {
      for material in SkinKeyMaterial.allCases {
        var skin = CustomKeyboardSkin()
        skin.keyShape = shape; skin.keyMaterial = material
        let saved = SavedKeyboardSkin(name: "主题", design: skin)
        let restored = try JSONDecoder().decode(SavedKeyboardSkin.self, from: JSONEncoder().encode(saved))
        XCTAssertEqual(restored.design, skin)
        let view = SkinKeySurfaceView(frame: CGRect(x: 0, y: 0, width: 44, height: 48))
        view.design = restored.design; view.fillColor = .systemTeal
        let image = UIGraphicsImageRenderer(size: view.bounds.size).image { view.layer.render(in: $0.cgContext) }
        images.insert(try XCTUnwrap(image.pngData()))
      }
    }
    XCTAssertEqual(images.count, 16, "Every geometry/material pair should visibly differ")
    let rect = CGRect(x: 0, y: 0, width: 44, height: 48)
    let ticket = SkinKeySurfaceView.path(in: rect, shape: .ticket, radius: 8)
    XCTAssertFalse(ticket.contains(CGPoint(x: 1, y: 24)))
    XCTAssertTrue(ticket.contains(CGPoint(x: 22, y: 24)))
  }

}
