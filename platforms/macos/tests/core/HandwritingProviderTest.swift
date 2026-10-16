import AppKit
import SwiftUI

@objc(MSIMEClientSession) final class HandwritingSessionStub: NSObject {
  static var request: NSDictionary?

  @objc class func handwritingProviderRequest(_ request: NSDictionary) -> NSDictionary {
    self.request = request
    return ["candidates": ["synthetic-candidate"]]
  }
}

@main enum HandwritingProviderTest {
  @MainActor static func main() throws {
    let appearance = MacHandwritingAppearance()
    assert(appearance.colorScheme == nil)
    // macOS 的手写识别板只看颜色模式，它单独的明暗设置不读。
    appearance.apply(["theme": "light", "handwriting_theme": "dark"])
    assert(appearance.colorScheme == .light)
    appearance.apply(["theme": "dark", "handwriting_theme": "light"])
    assert(appearance.colorScheme == .dark)
    appearance.apply(["theme": "dark", "handwriting_theme": "follow"])
    assert(appearance.colorScheme == .dark)
    appearance.apply(["theme": "system", "handwriting_theme": "follow"])
    assert(appearance.colorScheme == nil)
    appearance.apply(["theme": "light", "handwriting_theme": "invalid"])
    assert(appearance.colorScheme == .light)
    let strokes = [MacInkStroke(points: [CGPoint(x: 10, y: 12), CGPoint(x: 20, y: 24)])]
    let candidates = try MacHandwritingProvider.recognize(strokes, socketPath: "/tmp/synthetic-handwriting.sock")
    assert(candidates == ["synthetic-candidate"])
    let request = HandwritingSessionStub.request!
    assert(request["language"] as? String == "zh-CN")
    let rows = request["strokes"] as? [[NSDictionary]]
    assert(rows?.count == 1 && rows?.first?.count == 2)
    assert((rows?.first?.first?["x"] as? NSNumber)?.floatValue == 10)
    assert((rows?.first?.first?["y"] as? NSNumber)?.floatValue == 12)

    do {
      _ = try MacHandwritingProvider.recognizeLocal([])
      assertionFailure("empty local handwriting input was accepted")
    } catch { }

    let snapshotDirectory = ProcessInfo.processInfo.environment["MSIME_HANDWRITING_SNAPSHOT_DIR"]
    for (name, theme) in [("light", "light"), ("dark", "dark")] {
      appearance.apply(["theme": theme, "handwriting_theme": "follow"])
      let renderer = ImageRenderer(content: MacHandwritingToolView(appearance: appearance, snapshotCanvas: true,
        previewCandidates: ["水", "永", "冰", "泳", "木", "本", "氵", "泉", "求"])
        .frame(width: 720, height: 510))
      renderer.scale = 2
      guard let image = renderer.cgImage else { fatalError("handwriting UI render unavailable") }
      assert(image.width == 1440 && image.height == 1020)
      if let snapshotDirectory {
        let bitmap = NSBitmapImageRep(cgImage: image)
        let data = bitmap.representation(using: .png, properties: [:])!
        try data.write(to: URL(fileURLWithPath: snapshotDirectory).appendingPathComponent("handwriting-\(name).png"))
      }
    }
    print("macOS handwriting provider payload, local-input bounds and light/dark UI rendering passed")
  }
}
