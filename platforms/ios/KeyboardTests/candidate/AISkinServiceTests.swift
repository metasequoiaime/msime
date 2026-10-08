import Foundation
import XCTest
import UIKit

final class AISkinServiceTests: XCTestCase {
  private func response(_ transform: (inout [[String:Any]]) -> Void = { _ in }) throws -> String {
    var designs: [[String:Any]] = (0..<3).map { index in
      ["name":"测试设计 \(index)","description":"合成测试方案","background":"#FFFFFF","keyBackground":"#F0F0F0",
       "keyForeground":"#FFFFFF","accent":"#FFFFFF","actionBackground":"#185C47","gradientEnd":NSNull(),
       "gradientHorizontal":false,"cornerRadius":index * 5,"borderWidth":0,"shadow":0.1,"pattern":index,"monospaced":false]
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
  func testRecordedCloudResponseProducesThreeSaveableDesigns() throws {
    let file = try XCTUnwrap(Bundle(for:Self.self).url(forResource:"AISkinReference",withExtension:"json"))
    let proposals = try AISkinService.parse(String(contentsOf:file,encoding:.utf8))
    XCTAssertEqual(proposals.count,3)
    XCTAssertEqual(proposals.first?.name,"静谧奶油")
    for proposal in proposals {
      XCTAssertTrue(proposal.design.hasReadableText)
      XCTAssertEqual(try JSONDecoder().decode(CustomKeyboardSkin.self,from:JSONEncoder().encode(proposal.design)),proposal.design)
    }
  }
  func testInvalidOrDuplicateDesignNeverBecomesASkin() throws {
    for text in ["```json\n{}\n```", "{}", try response { $0.removeLast() },
                 try response { $0[0]["background"] = "https://example.com/image.png" },
                 try response { $0[0]["cornerRadius"] = 100 },
                 try response { $0[0]["keyShape"] = "external.svg" },
                 try response { $0[0]["keyMaterial"] = "unknown" },
                 try response { $0[1] = $0[0] },
                 try response { $0[0]["background"] = "#000000"; $0[0]["keyBackground"] = "#FFFFFF" }] {
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
