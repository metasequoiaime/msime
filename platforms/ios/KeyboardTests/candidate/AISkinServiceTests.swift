import Foundation
import XCTest
import UIKit

final class AISkinServiceTests: XCTestCase {
  /// 三套合格的合成方案：造型、材质和背景场景各不相同，按键文字和强调色的对比度不足，留给解析修正。
  private func response(_ transform: (inout [[String:Any]]) -> Void = { _ in }) throws -> String {
    let shapes = ["rounded", "capsule", "ticket"], materials = ["flat", "raised", "glass"], scenes = ["山", "海", "林"]
    var designs: [[String:Any]] = (0..<3).map { index in
      ["name":"测试设计 \(index)","description":"合成测试方案","artworkPrompt":String(repeating: scenes[index], count: 50),
       "background":"#FFFFFF","keyBackground":"#F0F0F0",
       "keyForeground":"#FFFFFF","accent":"#FFFFFF","actionBackground":"#185C47","gradientEnd":NSNull(),
       "gradientHorizontal":false,"keyShape":shapes[index],"keyMaterial":materials[index],
       "cornerRadius":index * 5,"borderWidth":0,"shadow":0.1,"pattern":index,"monospaced":false]
    }
    transform(&designs)
    return String(data:try JSONSerialization.data(withJSONObject:["skins":designs]),encoding:.utf8)!
  }
  func testThreeEditableDesignsRepairContrastAndRoundTrip() throws {
    let proposals = try AISkinService.parse(response())
    XCTAssertEqual(proposals.count,3)
    XCTAssertEqual(Set(proposals.map(\.design)).count,3)
    for proposal in proposals {
      XCTAssertTrue(proposal.design.hasReadableText)
      XCTAssertNil(proposal.design.photo)
      let saved = SavedKeyboardSkin(name:proposal.name,design:proposal.design)
      let restored = try JSONDecoder().decode(SavedKeyboardSkin.self,from:JSONEncoder().encode(saved))
      XCTAssertEqual(restored.design,proposal.design)
    }
  }
  /// 早先录下的云端回答没有键帽造型、材质和背景场景。原来 `generate` 解析后才因此拒绝，现在 Rust 的解析一步就拒绝，两条路的结论相同。
  func testRecordedCloudResponseWithoutKeyStylesIsRefused() throws {
    let file = try XCTUnwrap(Bundle(for:Self.self).url(forResource:"AISkinReference",withExtension:"json"))
    XCTAssertThrowsError(try AISkinService.parse(String(contentsOf:file,encoding:.utf8)))
  }
  /// 系统提示词由 Rust 给出，用户的描述原样排在后面；Rust 拒绝的描述不发请求。
  func testPromptComesFromRust() throws {
    let messages = try AISkinService.messages(prompt: "竹林里的熊猫茶会", model: "synthetic-model")
    XCTAssertEqual(messages.map(\.role), ["system", "user"])
    XCTAssertTrue(messages[0].content.contains("keyShape"))
    XCTAssertEqual(messages[1].content, "竹林里的熊猫茶会")
    XCTAssertThrowsError(try AISkinService.messages(prompt: String(repeating: "景", count: 501), model: "synthetic-model"))
    XCTAssertThrowsError(try AISkinService.messages(prompt: "竹林", model: ""))
  }
  /// 多行输入框里的换行和制表符折成一个空格再交给 Rust（Rust 拒收一切控制字符）；500 个字的上限按 Unicode 标量计，与 Rust 相同，超出的在联网之前就拒绝。
  func testMultilinePromptIsFoldedBeforeRust() throws {
    XCTAssertThrowsError(try AISkinService.messages(prompt: "竹林里的熊猫\n茶会", model: "synthetic-model"))
    let folded = try XCTUnwrap(AISkinService.requestPrompt("  竹林里的熊猫 \r\n\n  茶会\t夜色\n"))
    XCTAssertEqual(folded, "竹林里的熊猫 茶会 夜色")
    XCTAssertEqual(try AISkinService.messages(prompt: folded, model: "synthetic-model")[1].content, folded)
    XCTAssertNil(AISkinService.requestPrompt(" \n\t "))
    XCTAssertNil(AISkinService.requestPrompt("竹林\u{0007}茶会"))
    // 一个家庭 emoji 是一个字形、七个 Unicode 标量：100 个就是 700 个标量，字形数没超 500，Rust 也会拒绝。
    let family = String(repeating: "👨‍👩‍👧‍👦", count: 100)
    XCTAssertLessThanOrEqual(family.count, 500)
    XCTAssertNil(AISkinService.requestPrompt(family))
    XCTAssertThrowsError(try AISkinService.messages(prompt: family, model: "synthetic-model"))
    let limit = String(repeating: "景", count: AISkinService.maximumPromptScalars)
    XCTAssertEqual(AISkinService.requestPrompt(limit), limit)
    XCTAssertNoThrow(try AISkinService.messages(prompt: limit, model: "synthetic-model"))
  }
  func testInvalidOrDuplicateDesignNeverBecomesASkin() throws {
    for text in ["```json\n{}\n```", "{}", try response { $0.removeLast() },
                 try response { $0[0]["background"] = "https://example.com/image.png" },
                 try response { $0[0]["cornerRadius"] = 100 },
                 try response { $0[0]["keyShape"] = "external.svg" },
                 try response { $0[0]["keyMaterial"] = "unknown" },
                 try response { $0[1] = $0[0] },
                 try response { $0[0]["background"] = "#000000"; $0[0]["keyBackground"] = "#FFFFFF" },
                 // 以下几种原来的 Swift 解析照收，Rust 拒绝：多出字段、名称里有控制字符、两套造型相同、缺背景场景。
                 try response { $0[0]["unexpected"] = 1 },
                 try response { $0[0]["name"] = "测试\u{0007}设计" },
                 try response { $0[1]["keyShape"] = "rounded" },
                 try response { $0[0].removeValue(forKey: "artworkPrompt") }] {
      XCTAssertThrowsError(try AISkinService.parse(text))
    }
  }
  func testArtworkPromptUsesFortyToOneHundredUnicodeScalars() throws {
    for length in [39, 101] {
      let text = try response { $0[0]["artworkPrompt"] = String(repeating: "景", count: length) }
      XCTAssertThrowsError(try AISkinService.parse(text), "Accepted \(length) Unicode scalars")
    }
    for length in [40, 100] {
      let text = try response { $0[0]["artworkPrompt"] = String(repeating: "景", count: length) }
      XCTAssertEqual(try AISkinService.parse(text)[0].artworkPrompt?.unicodeScalars.count, length)
    }
    let composed = String(repeating: "e\u{301}", count: 20)
    XCTAssertEqual(composed.count, 20)
    XCTAssertEqual(composed.unicodeScalars.count, 40)
    let text = try response { $0[0]["artworkPrompt"] = composed }
    XCTAssertEqual(try AISkinService.parse(text)[0].artworkPrompt, composed)
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
