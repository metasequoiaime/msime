import XCTest
import UIKit

@MainActor
final class CandidateCanvasTests: XCTestCase {
  private func image(_ colors: [UIColor], width: Int = 6, height: Int = 6) -> Data {
    let format = UIGraphicsImageRendererFormat()
    format.scale = 1
    return UIGraphicsImageRenderer(size: CGSize(width: width, height: height), format: format).pngData { context in
      for y in 0..<height {
        for x in 0..<width {
          colors[min(colors.count - 1, (y / 2) * 3 + x / 2)].setFill()
          context.fill(CGRect(x: x, y: y, width: 1, height: 1))
        }
      }
    }
  }

  private func root(extra: String = "", border: String = "slice_dip = [2]\nwidth_dip = [4]\nfill = true") throws -> URL {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let package = root.appendingPathComponent("canvas-fixture")
    try FileManager.default.createDirectory(at: package, withIntermediateDirectories: true)
    addTeardownBlock { try? FileManager.default.removeItem(at: root) }
    try image([.red, .green, .blue, .cyan, .magenta, .yellow, .orange, .purple, .white])
      .write(to: package.appendingPathComponent("panel.png"))
    try image([.black]).write(to: package.appendingPathComponent("light.png"))
    try """
    schema_version = 2
    id = 'canvas-fixture'
    name = '合成画布'
    version = '1.0.0'
    base = 'canvas'
    [supports]
    layouts = ['horizontal', 'vertical']
    themes = ['light', 'dark']
    [candidate_window]
    item_corner_radius_dip = 5
    [candidate_window.border_image]
    source = 'panel.png'
    \(border)
    \(extra)
    [candidate.light]
    text = '#123456'
    selected_text = '#456789'
    [candidate.dark]
    text = '#abcdef'
    """.write(to: package.appendingPathComponent("skin.toml"), atomically: true, encoding: .utf8)
    return root
  }

  private func pixel(_ view: UIView, _ x: Int, _ y: Int) throws -> [UInt8] {
    view.layoutIfNeeded()
    let format = UIGraphicsImageRendererFormat()
    format.scale = 1
    let image = UIGraphicsImageRenderer(size: view.bounds.size, format: format).image { view.layer.render(in: $0.cgContext) }
    let cg = try XCTUnwrap(image.cgImage?.cropping(to: CGRect(x: x, y: y, width: 1, height: 1)))
    var bytes: [UInt8] = [0, 0, 0, 0]
    let context = try XCTUnwrap(CGContext(data: &bytes, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    context.draw(cg, in: CGRect(x: 0, y: 0, width: 1, height: 1))
    return bytes
  }

  func testNineSlicesHaveCorrectOrientationAndDoNotBleedAcrossEdges() throws {
    let canvas = try XCTUnwrap(ExternalCandidateSkin.canvas("canvas-fixture", dark: true, root: root()))
    XCTAssertTrue(canvas.panelLoaded)
    let view = CandidateCanvasView(level: 0)
    view.frame = CGRect(x: 0, y: 0, width: 40, height: 32)
    view.canvas = canvas
    XCTAssertEqual(try pixel(view, 1, 1), [255, 0, 0, 255])
    XCTAssertEqual(try pixel(view, 38, 1), [0, 0, 255, 255])
    XCTAssertEqual(try pixel(view, 1, 30), [255, 128, 0, 255])
    XCTAssertEqual(try pixel(view, 38, 30), [255, 255, 255, 255])
    XCTAssertEqual(try pixel(view, 20, 1), [0, 255, 0, 255])
    XCTAssertEqual(try pixel(view, 20, 15), [255, 0, 255, 255])
    XCTAssertNil(view.hitTest(CGPoint(x: 20, y: 15), with: nil))
  }

  func testStretchOverridesSliceTwoValuesExpandLikeCSSAndInsetsContainBorders() throws {
    let root = try root(border: "slice_dip = [2]\nstretch_dip = [1, 2]\nwidth_dip = [3, 4]\nfill = false")
    let canvas = try XCTUnwrap(ExternalCandidateSkin.canvas("canvas-fixture", dark: true, root: root))
    XCTAssertEqual(canvas.tiles[0]?.images.first?.width, 2)
    XCTAssertEqual(canvas.tiles[0]?.images.first?.height, 1)
    XCTAssertEqual(canvas.tiles[6]?.images.first?.height, 1)
    XCTAssertEqual(canvas.contentInsets, UIEdgeInsets(top: 3, left: 4, bottom: 3, right: 4))
    let view = CandidateCanvasView(level: 0)
    view.frame = CGRect(x: 0, y: 0, width: 40, height: 32)
    view.canvas = canvas
    XCTAssertEqual(try pixel(view, 20, 15), [0, 0, 0, 0])
    view.frame.size = CGSize(width: 2, height: 1)
    view.layoutIfNeeded()
    for layer in view.layer.sublayers ?? [] { XCTAssertGreaterThanOrEqual(layer.frame.width, 0); XCTAssertGreaterThanOrEqual(layer.frame.height, 0) }
  }

  func testLightSourceAndPanelCapabilityControlPalette() throws {
    let root = try root(border: "light_source = 'light.png'\nslice_dip = [2]\nwidth_dip = [4]")
    let canvas = try XCTUnwrap(ExternalCandidateSkin.canvas("canvas-fixture", dark: false, root: root))
    let view = CandidateCanvasView(level: 0)
    view.frame = CGRect(x: 0, y: 0, width: 40, height: 32)
    view.canvas = canvas
    XCTAssertEqual(try pixel(view, 20, 15), [0, 0, 0, 255])
    let preferences: [String: Any] = ["global_theme": "custom", "custom_theme": ["candidate_skin": "canvas-fixture"]]
    let fallback = try XCTUnwrap(CandidatePalette.resolveTheme(preferences, systemDark: false, skinsRoot: root))
    let panel = try XCTUnwrap(CandidatePalette.resolveTheme(preferences, systemDark: false, skinsRoot: root, canvasPanelDrawn: true))
    XCTAssertNotEqual(fallback.candidate?.text, panel.candidate?.text)
    XCTAssertEqual(ThemeColor.hex(try XCTUnwrap(panel.candidate?.text)), "#123456")
    let path = root.appendingPathComponent("canvas-fixture/panel.png")
    try FileManager.default.removeItem(at: path)
    XCTAssertNil(ExternalCandidateSkin.canvas("canvas-fixture", dark: true, root: root))
  }

  func testDecorationsAreStableAndDrawBehindPanelOrAboveText() throws {
    let root = try root(extra: """
    [[candidate_window.decorations]]
    image = 'light.png'
    anchor = 'center'
    width_dip = 12
    height_dip = 12
    z_index = -1
    [[candidate_window.decorations]]
    image = 'light.png'
    anchor = 'top-left'
    width_dip = 6
    height_dip = 6
    z_index = 2
    [[candidate_window.decorations]]
    image = 'panel.png'
    anchor = 'top-left'
    width_dip = 6
    height_dip = 6
    z_index = 2
    """)
    let canvas = try XCTUnwrap(ExternalCandidateSkin.canvas("canvas-fixture", dark: true, root: root))
    XCTAssertEqual(canvas.decorations.count, 3)
    let host = CandidateCanvasHostView(frame: CGRect(x: 0, y: 0, width: 40, height: 32))
    host.minimumContentHeight = 0
    host.minimumContentWidth = 0
    host.canvas = canvas
    let content = UIView(frame: host.bounds)
    content.backgroundColor = .cyan
    host.addSubview(content)
    host.layoutIfNeeded()
    XCTAssertEqual(host.subviews.last?.layer.sublayers?.count, 2)
    XCTAssertEqual(host.subviews.last?.layer.sublayers?.last?.frame, CGRect(x: 0, y: 0, width: 6, height: 6))
    XCTAssertEqual(try pixel(host, 1, 1), [255, 0, 0, 255])
    XCTAssertEqual(try pixel(host, 20, 15), [0, 255, 255, 255])
    XCTAssertTrue(host.hitTest(CGPoint(x: 1, y: 1), with: nil) === content)
    content.isHidden = true
    XCTAssertEqual(try pixel(host, 20, 15), [255, 0, 255, 255])
  }

  func testAllNineAnchorsContainAspectRatioAndOffsetsPointDown() throws {
    var remaining = 1_000
    let frames = try XCTUnwrap(ExternalCandidateSkin.Canvas.Frames.decode(image([.red], width: 6, height: 3), remaining: &remaining))
    let card = CGRect(x: 10, y: 20, width: 80, height: 60)
    for (anchor, x, y) in [("top-left", 12, 26), ("top", 47, 26), ("top-right", 82, 26),
      ("left", 12, 51), ("center", 47, 51), ("right", 82, 51),
      ("bottom-left", 12, 76), ("bottom", 47, 76), ("bottom-right", 82, 76)] {
      let decoration = ExternalCandidateSkin.Canvas.Decoration(frames: frames, anchor: anchor, offsetX: 2, offsetY: 3, width: 10, height: 10, zIndex: 1)
      let rect = CandidateCanvasView.decorationRect(decoration, card: card)
      XCTAssertEqual(rect.minX, CGFloat(x), accuracy: 0.01, anchor)
      XCTAssertEqual(rect.minY, CGFloat(y) - 0.5, accuracy: 0.01, anchor)
      XCTAssertEqual(rect.size, CGSize(width: 10, height: 5))
    }
  }

  func testFixedStripHeightKeepsReadableContentAndExpandedGridUsesInsets() throws {
    let canvas = try XCTUnwrap(ExternalCandidateSkin.canvas("canvas-fixture", dark: true, root: root(extra: """
    [candidate_window.outer_insets_dip]
    top = 30
    bottom = 22
    """)))
    let host = CandidateCanvasHostView(frame: CGRect(x: 0, y: 0, width: 390, height: 44))
    host.minimumContentHeight = 28
    host.canvas = canvas
    host.layoutIfNeeded()
    XCTAssertEqual(host.candidateContentGuide.layoutFrame.height, 28, accuracy: 0.01)
    let left = UIView(), right = UIView()
    let panel = KeyboardCandidatePanelView(candidates: ["合成候选", "另一候选"], preedit: "synthetic", showsHeader: false,
      canvas: canvas, display: { $0 }, onSelect: { _ in }, onClose: {}, columns: (left, right))
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 300)
    panel.layoutIfNeeded()
    XCTAssertEqual(left.frame.minY, 34, accuracy: 0.01)
    XCTAssertEqual(left.frame.maxY, 274, accuracy: 0.01)
    let stripHeight = host.frame.height
    host.canvas = nil
    host.layoutIfNeeded()
    XCTAssertEqual(host.candidateContentGuide.layoutFrame, host.bounds)
    XCTAssertEqual(host.frame.height, stripHeight)
  }
  func testAnimationFramesTimingLoopsAndLifecycle() throws {
    let fixtures: [String: String] = [
      "png": "iVBORw0KGgoAAAANSUhEUgAAAAwAAAAMCAYAAABWdVznAAAACGFjVEwAAAACAAAAAPONk3AAAAAaZmNUTAAAAAAAAAAMAAAADAAAAAAAAAAAALQD6AAA8a7+pwAAABxJREFUeJxj/M/A8J+BBMBEimIQGNVADBgOoQQArIUCFiFVnUUAAAAaZmNUTAAAAAEAAAAMAAAADAAAAAAAAAAAALQD6AAAat0UcwAAACBmZEFUAAAAAnicY2Rg+P+fgQTARIpiEBjVQAwYDqEEAKqHAhZF3Mt4AAAAAElFTkSuQmCC",
      "gif": "R0lGODlhDAAMAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQAEgAAACwAAAAADAAMAAAIFgABCBxIsKDBgwgTKlzIsKHDhxARBgQAIfkEARIAAQAsAAAAAAwADACBAAD/AAAAAAAAAAAACBYAAQgcSLCgwYMIEypcyLChw4cQEQYEADs=",
      "webp": "UklGRoQAAABXRUJQVlA4WAoAAAACAAAACwAACwAAQU5JTQYAAAAAAAAAAABBTk1GKAAAAAAAAAAAAAsAAAsAALQAAAJWUDhMDwAAAC8LwAIABxD9j/4HIqL/AQBBTk1GKAAAAAAAAAAAAAsAAAsAALQAAABWUDhMDwAAAC8LwAIABxDR//4HIqL/AQA="
    ]
    for (name, base64) in fixtures {
      var remaining = 8_000_000
      let frames = try XCTUnwrap(ExternalCandidateSkin.Canvas.Frames.decode(try XCTUnwrap(Data(base64Encoded: base64)), remaining: &remaining), name)
      XCTAssertEqual(frames.images.count, 2, name)
      XCTAssertEqual(frames.duration, 0.36, accuracy: 0.001, name)
      XCTAssertEqual(frames.times, [0, 0.5, 1], name)
      XCTAssertEqual(frames.loops, 0, name)
      let canvas = ExternalCandidateSkin.Canvas(tiles: [nil, nil, nil, nil, frames, nil, nil, nil, nil], widths: [0, 0, 0, 0], fill: true,
        contentInsets: .zero, outerInsets: .zero, decorations: [], itemCornerRadius: 0, lineGap: 0)
      let view = CandidateCanvasView(level: 0)
      view.canvas = canvas
      view.frame = CGRect(x: 0, y: 0, width: 40, height: 32)
      let controller = UIViewController()
      controller.view.addSubview(view)
      let window = UIWindow(frame: view.bounds)
      window.rootViewController = controller
      window.isHidden = false
      view.reduceMotion = { false }
      view.refreshPlayback()
      let sprite = try XCTUnwrap(view.layer.sublayers?.first)
      XCTAssertNotNil(sprite.animation(forKey: "canvas-frames"), name)
      view.reduceMotion = { true }
      view.refreshPlayback()
      XCTAssertNil(sprite.animation(forKey: "canvas-frames"), name)
      view.reduceMotion = { false }
      view.refreshPlayback()
      view.isHidden = true
      XCTAssertNil(sprite.animation(forKey: "canvas-frames"), name)
      view.isHidden = false
      XCTAssertNotNil(sprite.animation(forKey: "canvas-frames"), name)
      NotificationCenter.default.post(name: UIApplication.didEnterBackgroundNotification, object: nil)
      XCTAssertNil(sprite.animation(forKey: "canvas-frames"), name)
      NotificationCenter.default.post(name: UIApplication.didBecomeActiveNotification, object: nil)
      XCTAssertNotNil(sprite.animation(forKey: "canvas-frames"), name)
      view.removeFromSuperview()
      XCTAssertNil(sprite.animation(forKey: "canvas-frames"), name)
      window.isHidden = true
    }
  }

  func testRealControllerSharesCanvasWithExpandedPanelAndClearsItWhenDisabled() async throws {
    preserveSharedTheme()
    let defaults = CandidatePalette.defaults
    let follows = defaults.object(forKey: CandidatePalette.followsDesktopKey)
    let oldScheme = InputSchemePreference.scheme
    let oldFontSize = MetasequoiaInputSessionBridge.loadSharedPreferences()?[CandidateFontPreference.candidateKey]
    addTeardownBlock {
      defaults.set(follows, forKey: CandidatePalette.followsDesktopKey)
      InputSchemePreference.scheme = oldScheme
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[CandidateFontPreference.candidateKey] = oldFontSize }
    }
    defaults.set(true, forKey: CandidatePalette.followsDesktopKey)
    InputSchemePreference.scheme = .quanpin
    let fixtureRoot = try root()
    let source = fixtureRoot.appendingPathComponent("canvas-fixture")
    let destination = fixtureRoot.appendingPathComponent("installed-skins")
    let id = "ios-canvas-" + UUID().uuidString.lowercased()
    let renamed = source.deletingLastPathComponent().appendingPathComponent(id)
    try FileManager.default.moveItem(at: source, to: renamed)
    let manifest = renamed.appendingPathComponent("skin.toml")
    let text = try String(contentsOf: manifest, encoding: .utf8).replacingOccurrences(of: "id = 'canvas-fixture'", with: "id = '\(id)'")
    try text.write(to: manifest, atomically: true, encoding: .utf8)
    XCTAssertEqual(ExternalCandidateSkin.importFolder(renamed, root: destination), .success(id))
    addTeardownBlock { _ = ExternalCandidateSkin.remove(id, root: destination) }
    XCTAssertTrue(GlobalThemePreference.update {
      $0["global_theme"] = "custom"
      $0[CandidateFontPreference.candidateKey] = 24
      $0["custom_theme"] = ["candidate_skin": id, "candidate_skin_dark": id]
    })
    let controller = KeyboardViewController()
    controller.candidateSkinsRoot = destination
    controller.overrideUserInterfaceStyle = .light
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(descendants) }
    func button(_ id: String) throws -> UIButton {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == id } as? UIButton)
    }
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" } as? CandidateCanvasHostView)
    for _ in 0..<300 where strip.canvas == nil { try await Task.sleep(nanoseconds: 10_000_000) }
    let canvas = try XCTUnwrap(strip.canvas)
    XCTAssertTrue(canvas.panelLoaded)
    controller.view.layoutIfNeeded()
    XCTAssertGreaterThanOrEqual(strip.minimumContentHeight, 41, "大字号需要的高度不能让给皮肤留白")
    XCTAssertGreaterThanOrEqual(strip.candidateContentGuide.layoutFrame.height, 41)
    for character in "nihao" {
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(String(character).uppercased())" } as? UIButton)
      key.sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    XCTAssertTrue(strip.canvas === canvas, "逐键复用已解码资源")
    try button("expandCandidates").sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" } as? KeyboardCandidatePanelView)
    XCTAssertTrue(panel.canvas === canvas)
    defaults.set(false, forKey: CandidatePalette.followsDesktopKey)
    controller.viewWillAppear(false)
    for _ in 0..<100 where strip.canvas != nil { try await Task.sleep(nanoseconds: 10_000_000) }
    XCTAssertNil(strip.canvas)
    XCTAssertNil(panel.canvas)
    controller.viewWillDisappear(false)
  }

  /// 本机验收时暂存固定上游资源；CI 不下载、不复制这些图片进仓库。
  func testUpstreamCanvasesOnRealControllerWhenStaged() async throws {
    guard let root = ExternalCandidateSkin.defaultRoot else {
      throw XCTSkip("本机上游图片验收需要已签名的 App Group 容器")
    }
    guard ["green_leaf", "sakura-night"].allSatisfy({ FileManager.default.fileExists(atPath: root.appendingPathComponent($0).appendingPathComponent("skin.toml").path) }) else {
      throw XCTSkip("未暂存固定上游的两款 canvas 皮肤")
    }
    preserveSharedTheme()
    let follows = CandidatePalette.defaults.object(forKey: CandidatePalette.followsDesktopKey)
    let scheme = InputSchemePreference.scheme
    addTeardownBlock {
      CandidatePalette.defaults.set(follows, forKey: CandidatePalette.followsDesktopKey)
      InputSchemePreference.scheme = scheme
    }
    CandidatePalette.defaults.set(true, forKey: CandidatePalette.followsDesktopKey)
    let output = root.deletingLastPathComponent().appendingPathComponent("canvas-acceptance")
    try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(descendants) }
    for id in ["green_leaf", "sakura-night"] {
      for dark in [false, true] {
        for scheme in [ChineseInputScheme.quanpin, ChineseInputScheme.nineKey] {
          InputSchemePreference.scheme = scheme
          XCTAssertTrue(GlobalThemePreference.update {
            $0["global_theme"] = "custom"
            $0["custom_theme"] = ["candidate_skin": id, "candidate_skin_dark": id]
          })
          let controller = KeyboardViewController()
          controller.overrideUserInterfaceStyle = dark ? .dark : .light
          controller.loadViewIfNeeded()
          controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
          controller.viewWillAppear(false)
          let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" } as? CandidateCanvasHostView)
          for _ in 0..<300 where strip.canvas == nil { try await Task.sleep(nanoseconds: 10_000_000) }
          let canvas = try XCTUnwrap(strip.canvas)
          XCTAssertTrue(canvas.panelLoaded)
          XCTAssertFalse(canvas.decorations.isEmpty)
          func button(_ id: String) throws -> UIButton {
            try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == id } as? UIButton)
          }
          if scheme == .nineKey {
            for digit in "64426" { try button("nineKey\(digit)").sendActions(for: .primaryActionTriggered) }
          } else {
            for character in "nihao" {
              let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(String(character).uppercased())" } as? UIButton)
              key.sendActions(for: .primaryActionTriggered)
            }
          }
          for expanded in [false, true] {
            if expanded { try button("expandCandidates").sendActions(for: .primaryActionTriggered) }
            controller.view.layoutIfNeeded()
            if expanded {
              let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" } as? KeyboardCandidatePanelView)
              XCTAssertTrue(panel.canvas === canvas)
            }
            let format = UIGraphicsImageRendererFormat()
            format.scale = 2
            let png = UIGraphicsImageRenderer(size: controller.view.bounds.size, format: format).pngData { controller.view.layer.render(in: $0.cgContext) }
            let name = "\(id)-\(dark ? "dark" : "light")-\(scheme == .nineKey ? "nine" : "full")-\(expanded ? "expanded" : "strip").png"
            try png.write(to: output.appendingPathComponent(name))
          }
          controller.viewWillDisappear(false)
        }
      }
    }
  }

  func testFiniteGIFLoopsFastFramesAndDecodeBudget() throws {
    let original = try XCTUnwrap(Data(base64Encoded: "R0lGODlhDAAMAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQAEgAAACwAAAAADAAMAAAIFgABCBxIsKDBgwgTKlzIsKHDhxARBgQAIfkEARIAAQAsAAAAAAwADACBAAD/AAAAAAAAAAAACBYAAQgcSLCgwYMIEypcyLChw4cQEQYEADs="))
    let header = Data([0x21, 0xff, 0x0b] + Array("NETSCAPE2.0".utf8))
    let range = try XCTUnwrap(original.range(of: header))
    var once = original
    once.removeSubrange(range.lowerBound..<(range.lowerBound + 19))
    var budget = 8_000_000
    XCTAssertEqual(try XCTUnwrap(ExternalCandidateSkin.Canvas.Frames.decode(once, remaining: &budget)).loops, 1)
    var finite = original
    finite[range.lowerBound + 16] = 2
    budget = 8_000_000
    XCTAssertEqual(try XCTUnwrap(ExternalCandidateSkin.Canvas.Frames.decode(finite, remaining: &budget)).loops, 3)
    for index in 0..<(finite.count - 7) where finite[index] == 0x21 && finite[index + 1] == 0xf9 && finite[index + 2] == 4 {
      finite[index + 4] = 1
      finite[index + 5] = 0
    }
    budget = 8_000_000
    XCTAssertEqual(try XCTUnwrap(ExternalCandidateSkin.Canvas.Frames.decode(finite, remaining: &budget)).duration, 0.2, accuracy: 0.001)
    budget = 200
    XCTAssertNil(ExternalCandidateSkin.Canvas.Frames.decode(original, remaining: &budget))
    budget = 8_000_000
    XCTAssertNil(ExternalCandidateSkin.Canvas.Frames.decode(image([.red], width: 2049, height: 1), remaining: &budget))
  }

  func testPanelDetailsAndUserPickersShareTheResolvedPalette() throws {
    let root = try root()
    let manifest = root.appendingPathComponent("canvas-fixture/skin.toml")
    let text = try String(contentsOf: manifest, encoding: .utf8).replacingOccurrences(of: "[candidate.dark]", with: """
    preedit_text = '#102030'
    selected_translation = '#405060'
    selected_bar = '#708090'
    show_selected_bar = true
    [candidate.dark]
    """)
    try text.write(to: manifest, atomically: true, encoding: .utf8)
    let skinPreferences: [String: Any] = ["global_theme": "custom", "custom_theme": ["candidate_skin": "canvas-fixture"]]
    let skinResolved = try XCTUnwrap(CandidatePalette.resolveTheme(skinPreferences, systemDark: false, skinsRoot: root, canvasPanelDrawn: true))
    let skinPalette = CandidatePalette.palette(skinResolved)
    XCTAssertEqual(ThemeColor.hex(try XCTUnwrap(skinPalette.preeditText)), "#102030")
    XCTAssertEqual(ThemeColor.hex(try XCTUnwrap(skinPalette.selectedTranslation)), "#405060")
    XCTAssertEqual(ThemeColor.hex(try XCTUnwrap(skinPalette.selectedBar)), "#708090")
    XCTAssertTrue(skinPalette.showSelectedBar)

    let preferences: [String: Any] = ["global_theme": "custom", "custom_theme": ["candidate_skin": "canvas-fixture", "candidate_colors": ["text": "#112233"]]]
    let resolved = try XCTUnwrap(CandidatePalette.resolveTheme(preferences, systemDark: false, skinsRoot: root, canvasPanelDrawn: true))
    let palette = CandidatePalette.palette(resolved)
    XCTAssertEqual(ThemeColor.hex(try XCTUnwrap(palette.preeditText)), "#112233")
    XCTAssertEqual(ThemeColor.hex(try XCTUnwrap(palette.selectedTranslation)), "#405060")
    XCTAssertEqual(ThemeColor.hex(try XCTUnwrap(palette.selectedBar)), "#708090")
    XCTAssertEqual(ThemeColor.hex(palette.text), "#112233")
    XCTAssertTrue(palette.showSelectedBar)
  }

}
