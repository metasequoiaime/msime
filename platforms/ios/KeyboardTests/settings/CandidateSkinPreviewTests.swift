import XCTest
import UIKit

@MainActor
final class CandidateSkinPreviewTests: XCTestCase {
  private func fixture() throws -> URL {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let package = root.appendingPathComponent("preview-canvas")
    try FileManager.default.createDirectory(at: package, withIntermediateDirectories: true)
    addTeardownBlock { try? FileManager.default.removeItem(at: root) }
    try image(.red).write(to: package.appendingPathComponent("panel.png"))
    try image(.blue).write(to: package.appendingPathComponent("light.png"))
    try """
    schema_version = 2
    id = 'preview-canvas'
    name = '合成预览'
    version = '1'
    base = 'canvas'
    [supports]
    layouts = ['horizontal']
    themes = ['light', 'dark']
    [candidate_window.border_image]
    source = 'panel.png'
    light_source = 'light.png'
    slice_dip = [2]
    width_dip = [4]
    [candidate.light]
    text = '#123456'
    [candidate.dark]
    text = '#abcdef'
    """.write(to: package.appendingPathComponent("skin.toml"), atomically: true, encoding: .utf8)
    return root
  }

  private func image(_ color: UIColor) -> Data {
    let format = UIGraphicsImageRendererFormat()
    format.scale = 1
    return UIGraphicsImageRenderer(size: CGSize(width: 6, height: 6), format: format).pngData { context in
      color.setFill()
      context.fill(CGRect(x: 0, y: 0, width: 6, height: 6))
    }
  }

  private func request(_ root: URL?, dark: Bool = false, revision: UUID = UUID()) throws -> CandidateSkinPreview.Request {
    try XCTUnwrap(CandidateSkinPreview.Request(preferences: ["global_theme": "custom",
      "custom_theme": ["candidate_skin": "preview-canvas", "candidate_skin_dark": "preview-canvas"]],
      systemDark: dark, resourceRevision: revision, root: root))
  }

  private func waitUntil(_ predicate: () -> Bool) async throws {
    for _ in 0..<300 {
      if predicate() { return }
      try await Task.sleep(nanoseconds: 10_000_000)
    }
    XCTFail("预览未在期限内完成")
  }

  private func pixel(_ view: UIView) throws -> [UInt8] {
    view.layoutIfNeeded()
    let format = UIGraphicsImageRendererFormat()
    format.scale = 1
    let image = UIGraphicsImageRenderer(size: view.bounds.size, format: format).image { view.layer.render(in: $0.cgContext) }
    let cg = try XCTUnwrap(image.cgImage?.cropping(to: CGRect(x: 1, y: 1, width: 1, height: 1)))
    var bytes: [UInt8] = [0, 0, 0, 0]
    let context = try XCTUnwrap(CGContext(data: &bytes, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    context.draw(cg, in: CGRect(x: 0, y: 0, width: 1, height: 1))
    return bytes
  }

  private func attach(_ view: UIView, name: String) {
    view.layoutIfNeeded()
    let image = UIGraphicsImageRenderer(size: view.bounds.size).image { view.layer.render(in: $0.cgContext) }
    let attachment = XCTAttachment(image: image)
    attachment.name = name
    attachment.lifetime = .keepAlways
    add(attachment)
  }

  func testModeAndSameIDResourceReloadUseActualCanvasAndPalette() async throws {
    let root = try fixture()
    let coordinator = CandidateSkinPreview.Coordinator()
    let view = CandidateSkinPreviewRow()
    view.frame = CGRect(x: 0, y: 0, width: 320, height: 64)
    coordinator.update(view, request: try request(root))
    try await waitUntil { view.canvas != nil }
    XCTAssertEqual(try pixel(view), [0, 0, 255, 255])
    attach(view, name: "canvas-preview-light")
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(descendants) }
    let label = try XCTUnwrap(descendants(view).compactMap { $0 as? UILabel }.first { $0.text == "输入" })
    XCTAssertEqual(ThemeColor.hex(label.textColor), "#123456")
    coordinator.update(view, request: try request(root, dark: true))
    XCTAssertNil(view.canvas, "加载时先移除旧模式画布")
    try await waitUntil { view.canvas != nil }
    XCTAssertEqual(try pixel(view), [255, 0, 0, 255])
    attach(view, name: "canvas-preview-dark")
    XCTAssertEqual(ThemeColor.hex(label.textColor), "#ABCDEF")
    let previous = view.canvas
    try image(.green).write(to: root.appendingPathComponent("preview-canvas/panel.png"))
    coordinator.update(view, request: try request(root, dark: true))
    try await waitUntil { view.canvas != nil }
    XCTAssertFalse(view.canvas === previous)
    XCTAssertEqual(try pixel(view), [0, 255, 0, 255])
    attach(view, name: "canvas-preview-replaced")
    try Data("broken".utf8).write(to: root.appendingPathComponent("preview-canvas/panel.png"))
    let finished = expectation(description: "坏图加载完成")
    let queue = DispatchQueue(label: "test.preview-broken")
    let broken = CandidateSkinPreview.Coordinator(queue: queue)
    broken.update(view, request: try request(root, dark: true))
    queue.async { DispatchQueue.main.async { finished.fulfill() } }
    await fulfillment(of: [finished], timeout: 5)
    XCTAssertNil(view.canvas)
    XCTAssertNotEqual(ThemeColor.hex(label.textColor), "#ABCDEF", "坏图不得沿用贴图专用文字色")
    coordinator.invalidate(view)
    broken.invalidate(view)
  }

  func testAnimatedPreviewUsesWindowLifecycleAndDismantlingClearsLayers() async throws {
    let root = try fixture()
    let package = root.appendingPathComponent("preview-canvas")
    let gif = try XCTUnwrap(Data(base64Encoded: "R0lGODlhDAAMAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQAEgAAACwAAAAADAAMAAAIFgABCBxIsKDBgwgTKlzIsKHDhxARBgQAIfkEARIAAQAsAAAAAAwADACBAAD/AAAAAAAAAAAACBYAAQgcSLCgwYMIEypcyLChw4cQEQYEADs="))
    try gif.write(to: package.appendingPathComponent("panel.gif"))
    let manifest = package.appendingPathComponent("skin.toml")
    try String(contentsOf: manifest, encoding: .utf8).replacingOccurrences(of: "panel.png", with: "panel.gif")
      .write(to: manifest, atomically: true, encoding: .utf8)
    let view = CandidateSkinPreviewRow()
    view.frame = CGRect(x: 0, y: 0, width: 320, height: 64)
    let controller = UIViewController()
    controller.view.addSubview(view)
    let window = UIWindow(frame: view.bounds)
    window.rootViewController = controller
    window.isHidden = false
    defer { window.isHidden = true }
    let nodes = view.subviews.compactMap { $0 as? CandidateCanvasView }
    for node in nodes { node.reduceMotion = { false } }
    let coordinator = CandidateSkinPreview.Coordinator()
    coordinator.update(view, request: try request(root, dark: true))
    try await waitUntil { view.canvas != nil }
    view.layoutIfNeeded()
    let sprite = try XCTUnwrap(nodes.flatMap { $0.layer.sublayers ?? [] }.first)
    XCTAssertNotNil(sprite.animation(forKey: "canvas-frames"))
    view.removeFromSuperview()
    XCTAssertNil(sprite.animation(forKey: "canvas-frames"))
    coordinator.invalidate(view)
    XCTAssertNil(view.canvas)
    XCTAssertTrue(nodes.allSatisfy { ($0.layer.sublayers ?? []).isEmpty })
  }

  func testLateResultAndDismantledViewCannotRestoreOldAppearance() async throws {
    let first = try request(nil)
    let next = try request(nil, dark: true)
    let gate = DispatchSemaphore(value: 0)
    let started = expectation(description: "旧请求开始")
    let finished = expectation(description: "旧请求完成")
    let coordinator = CandidateSkinPreview.Coordinator(queue: DispatchQueue(label: "test.preview-race", attributes: .concurrent)) { request in
      XCTAssertFalse(Thread.isMainThread)
      var palette = CandidatePalette.palette(.system)
      if request == first {
        started.fulfill()
        _ = gate.wait(timeout: .now() + 5)
        palette.surface = .red
        DispatchQueue.main.async { finished.fulfill() }
      } else { palette.surface = .green }
      return CandidateSkinPreview.Snapshot(palette: palette, canvas: nil)
    }
    let view = CandidateSkinPreviewRow()
    coordinator.update(view, request: first)
    await fulfillment(of: [started], timeout: 5)
    coordinator.update(view, request: next)
    try await waitUntil { view.backgroundColor == .green }
    gate.signal()
    await fulfillment(of: [finished], timeout: 5)
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertEqual(view.backgroundColor, .green)
    coordinator.update(view, request: try request(nil, dark: true))
    coordinator.invalidate(view)
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertNil(view.canvas)
    XCTAssertNotEqual(view.backgroundColor, .green, "离开视图后不得应用在途结果")
  }
}
