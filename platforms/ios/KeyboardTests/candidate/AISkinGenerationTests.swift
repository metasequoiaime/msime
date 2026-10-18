import XCTest
import UIKit

/// AI 皮肤经 Rust 的 `msime_client_ai_skin_generate`（#8201 迁移计划 4c）：提示词、模型、三张插画和账号都在 Rust 里，这里只验证请求、进度、插画落成照片背景，以及失败时不交出半成品。
final class AISkinGenerationTests: XCTestCase {
  /// Rust 给出的三套方案（形状与 Tauri 的 AiSkinProposal 相同）：三种键帽造型和材质，各带一张插画。
  private func proposals(image: Data) throws -> [[String: Any]] {
    let names = ["鼠尾草晨雾", "奶油月光", "香草信笺"]
    let shapes: [SkinKeyShape] = [.capsule, .ticket, .pebble]
    let materials: [SkinKeyMaterial] = [.raised, .glass, .paper]
    return try (0..<3).map { index in
      var design = CustomKeyboardSkin.templates[index].1
      design.keyShape = shapes[index]; design.keyMaterial = materials[index]
      let encoded = try JSONSerialization.jsonObject(with: JSONEncoder().encode(design))
      return ["name": names[index], "description": "合成方案", "design": encoded, "artworkPrompt": "原创背景场景 \(index)",
              "artwork": ["b64_json": image.base64EncodedString(), "mime_type": "image/png", "width": 64, "height": 64]]
    }
  }

  private func answer(signedIn: Bool = true, generate: @escaping () -> String) {
    FakeRust.answer { call in
      switch call.entry {
      case "account": return FakeRust.ok(["user": signedIn ? FakeRust.user("user-1") as Any : NSNull()])
      case "ai_skin_generate": return generate()
      default: return FakeRust.failure("rust_client_unavailable")
      }
    }
  }

  @MainActor
  func testThreeIllustrationsAreBoundedAndDoNotAutoSaveOrApply() async throws {
    let current = CustomKeyboardSkinStore.current, saved = CustomSkinLibrary.designs
    let image = UIGraphicsImageRenderer(size: CGSize(width: 64, height: 64)).image {
      UIColor.systemTeal.setFill(); $0.fill(CGRect(x: 0, y: 0, width: 64, height: 64))
    }
    let plans = try proposals(image: try XCTUnwrap(image.pngData()))
    answer { FakeRust.ok(plans) }
    var progress: [Int] = []
    let proposals = try await AISkinService.generate("  合成主题\n验收 ", rust: { FakeRust.account }, progress: { progress.append($0) })
    // 进度在主线程上排队交出，等它们都到了再比。
    for _ in 0..<50 where progress.count < 3 { try await Task.sleep(nanoseconds: 10_000_000) }
    XCTAssertEqual(progress, [1, 2, 3])
    XCTAssertEqual(proposals.map(\.name), ["鼠尾草晨雾", "奶油月光", "香草信笺"])
    XCTAssertEqual(Set(proposals.compactMap { $0.design.keyShape }).count, 3)
    XCTAssertEqual(Set(proposals.compactMap { $0.design.keyMaterial }).count, 3)
    for proposal in proposals {
      let data = try XCTUnwrap(proposal.design.photo)
      XCTAssertLessThanOrEqual(data.count, 512_000)
      XCTAssertEqual(Array(data.prefix(2)), [0xFF, 0xD8], "Native storage uses sanitized JPEG")
      XCTAssertNotNil(UIImage(data: data))
    }
    let call = try XCTUnwrap(FakeRust.calls.first { $0.entry == "ai_skin_generate" })
    XCTAssertEqual(call.request["session_store"] as? Int, 7)
    XCTAssertEqual(call.request["prompt"] as? String, "合成主题 验收", "描述在发出之前折成一行")
    XCTAssertFalse((call.request["request_id"] as? String ?? "").isEmpty)
    XCTAssertEqual(CustomKeyboardSkinStore.current, current)
    XCTAssertEqual(CustomSkinLibrary.designs, saved)
  }

  func testCorruptArtworkStopsBeforeReturningPartialPlans() async throws {
    let plans = try proposals(image: Data("not an image".utf8))
    answer { FakeRust.ok(plans) }
    do {
      _ = try await AISkinService.generate("合成主题验收", rust: { FakeRust.account })
      XCTFail("Corrupt artwork must fail generation")
    } catch { XCTAssertTrue(error.localizedDescription.contains("插画")) }
  }

  func testSignedOutDoesNotGenerate() async throws {
    answer(signedIn: false) { FakeRust.failure("rust_client_unavailable") }
    do {
      _ = try await AISkinService.generate("合成主题验收", rust: { FakeRust.account })
      XCTFail("signed out must not generate")
    } catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure.code, "account_unauthorized") }
    XCTAssertTrue(FakeRust.calls.filter { $0.entry == "ai_skin_generate" }.isEmpty)
  }

  func testRustRejectionsBecomeServiceMessages() async throws {
    answer { FakeRust.failure("ai_skin_cancelled") }
    do { _ = try await AISkinService.generate("合成主题验收", rust: { FakeRust.account }); XCTFail("cancelled") }
    catch is CancellationError { }
    answer { FakeRust.failure("ai_skin_invalid") }
    do { _ = try await AISkinService.generate("合成主题验收", rust: { FakeRust.account }); XCTFail("invalid") }
    catch { XCTAssertEqual(error.localizedDescription, "AI 返回的皮肤方案不可用，请重新生成。") }
    answer { FakeRust.ok([["name": "只有一套"]]) }
    do { _ = try await AISkinService.generate("合成主题验收", rust: { FakeRust.account }); XCTFail("one plan") }
    catch { XCTAssertEqual(error.localizedDescription, "AI 返回的皮肤方案不可用，请重新生成。") }
    answer { FakeRust.failure("account_rate_limited", retryAfter: 7200) }
    do { _ = try await AISkinService.generate("合成主题验收", rust: { FakeRust.account }); XCTFail("limited") }
    catch { XCTAssertEqual(error.localizedDescription, "操作过于频繁，请约 2 小时后再试。") }
  }
}
