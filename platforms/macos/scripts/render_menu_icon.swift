#!/usr/bin/env xcrun swift

// 渲染输入菜单用的模板 TIFF：MSIMEClientInputMethodMenuIcon.tiff 是 bundle 自己引用的标志，取自 MSIMEClientInputMethodMenuIcon.svg 那条笔画；每个输入模式的图标则只有一个大字——中、双、五、粤、注、日、한、越、ཀ、笔或英——在图块四周留出空白，不带标志。菜单栏和 Ctrl+空格 切换条按 16 点绘制这些图标，以前「标志加右下角角标」的画法里角标只剩几个像素，认不出是哪个模式；苹果自家和其他输入法的输入源都是一个大字，一眼就能分清，这里照同样的做法。
//
// The input menu draws this through HIToolbox rather than through NSImage, and that path reads the TIFF's
// pages, not the DPI metadata of a single one: a lone 2x page is taken for a 32-point image, which the
// 16-point menu slot then crops to whatever sits in its middle - for this stroke, the thick middle bar,
// which arrives as a filled square. Apple's own input methods (AinuIM, TamilIM) ship 16x16 at 72 dpi and
// 32x32 at 144 dpi in one file, so that is what this writes.
//
// Usage: xcrun swift platforms/macos/scripts/render_menu_icon.swift [output-directory]
// Leaves the TIFFs beside the SVGs unless another directory is given.

import AppKit
import Foundation

// 通用图标是水杉标志 C 的单色版：一整块圆角方形，折线从中间镂空。各平台的标志都是 packages/ui/src/assets/msime.svg 那块深色圆角方形加绿边和白折线；菜单栏只认模板图（只看 alpha，颜色由系统按浅色、深色和高亮填），所以这里用同一个外形，把白折线换成镂空。几何取自 msime.svg 的 110 单位画布：圆角方形的外沿从 2.5 到 107.5、圆角 24.5（5 宽的边压在 5 到 105、圆角 22 的路径上），折线是母版 apps/desktop/app-icon.svg 那一笔以外框中心为原点缩到 80%、线宽 8 不变、round 端帽，坐标 y 向下，与 SVG 一致。改了 msime.svg 的折线就要同步这里和旁边的 MSIMEClientInputMethodMenuIcon.svg，再跑一次本脚本。
let tileOrigin: CGFloat = 2.5
let tileSide: CGFloat = 105
let tileCorner: CGFloat = 24.5

func metasequoiaStroke() -> CGPath {
    let path = CGMutablePath()
    path.move(to: CGPoint(x: 70.7787, y: 22.2))
    path.addLine(to: CGPoint(x: 39.1201, y: 34.3382))
    path.addLine(to: CGPoint(x: 70.7787, y: 43.4418))
    path.addLine(to: CGPoint(x: 39.1201, y: 58.6144))
    path.addCurve(to: CGPoint(x: 37.4, y: 87),
                  control1: CGPoint(x: 69.0496, y: 63.672),
                  control2: CGPoint(x: 97.1824, y: 67.8192))
    return path.copy(strokingWithWidth: 8, lineCap: .round, lineJoin: .miter, miterLimit: 4)
}

// One thirty-second of the tile stays clear on every side so the rounded square does not sit on the edge.
let edgeClearanceDivisor: CGFloat = 32

// 画一张空白的模板位图交给 draw，再写成 PNG。整张图是纯黑加 alpha：菜单把它当模板着色，形状只由 alpha 携带，填了底色就会变成一整块实心方块。
func render(pixels: Int, to url: URL, draw: (CGContext, CGFloat) -> Void) throws {
    let side = CGFloat(pixels)
    guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
                                     bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                     isPlanar: false, colorSpaceName: .deviceRGB,
                                     bytesPerRow: 0, bitsPerPixel: 0),
          let context = NSGraphicsContext(bitmapImageRep: rep) else {
        throw CocoaError(.fileWriteUnknown)
    }
    NSGraphicsContext.saveGraphicsState()
    defer { NSGraphicsContext.restoreGraphicsState() }
    NSGraphicsContext.current = context
    let cg = context.cgContext
    cg.clear(CGRect(x: 0, y: 0, width: side, height: side))
    cg.setFillColor(NSColor.black.cgColor)
    draw(cg, side)

    guard let png = rep.representation(using: .png, properties: [:]) else {
        throw CocoaError(.fileWriteUnknown)
    }
    try png.write(to: url)
}

func drawLogo(_ stroked: CGPath, in cg: CGContext, side: CGFloat) {
    let inset = side / edgeClearanceDivisor
    let scale = (side - inset * 2) / tileSide
    // The bitmap's origin is bottom-left and the SVG's is top-left: map the 110-unit canvas so the tile's outer edge fills the area inside the clearance, flipped.
    var transform = CGAffineTransform(translationX: inset, y: side - inset)
        .scaledBy(x: scale, y: -scale)
        .translatedBy(x: -tileOrigin, y: -tileOrigin)
    let tile = CGPath(roundedRect: CGRect(x: tileOrigin, y: tileOrigin, width: tileSide, height: tileSide),
                      cornerWidth: tileCorner, cornerHeight: tileCorner, transform: &transform)
    cg.addPath(tile)
    cg.fillPath()
    // 折线处清成透明：模板图里镂空的地方露出菜单栏本身的底色，和彩色标志上白折线的位置一致。
    guard let stroke = stroked.copy(using: &transform) else { return }
    cg.setBlendMode(.clear)
    cg.addPath(stroke)
    cg.fillPath()
    cg.setBlendMode(.normal)
}

// 模式字的墨迹长边占图块的 3/4，16 像素页约 12 像素；四周留白让系统切换条里的字不会比相邻输入源显得更大。
// 曾对照过三种画法（16 / 22 / 32 像素、浅色与深色菜单栏）：只有字、字外加圆角描边、实心圆角块里镂空字。描边和底块都要占掉外圈，16 像素页上字只剩 10 到 11 像素，越、粤这类笔画多的字糊成一团；只有字的那种最大也最清楚，所以不加框。
let modeGlyphFraction: CGFloat = 12.0 / 16

func modeFont(for character: String) -> CTFont {
    // PingFang 随所有受支持的 macOS 提供，Hiragino Sans GB 是系统自己给简体中文用的后备字体；两者都没有韩文和藏文，한 取系统的韩文字体 Apple SD Gothic Neo，字重相同；ཀ 取系统自带的藏文字体 Kailasa 的粗体。取第一个包含全部字形的字体。字号无所谓，绘制时按墨迹框缩放。
    var unichars = Array(character.utf16)
    var glyphs = [CGGlyph](repeating: 0, count: unichars.count)
    for name in ["PingFangSC-Semibold", "HiraginoSansGB-W6", "AppleSDGothicNeo-SemiBold", "Kailasa-Bold"] {
        let font = CTFontCreateWithName(name as CFString, 100, nil)
        if (CTFontCopyPostScriptName(font) as String) == name,
           CTFontGetGlyphsForCharacters(font, &unichars, &glyphs, unichars.count) { return font }
    }
    fatalError("no font for the input mode icon \(character)")
}

func drawModeGlyph(_ character: String, in cg: CGContext, side: CGFloat) {
    let font = modeFont(for: character)
    var unichars = Array(character.utf16)
    var glyphs = [CGGlyph](repeating: 0, count: unichars.count)
    guard CTFontGetGlyphsForCharacters(font, &unichars, &glyphs, unichars.count),
          let outline = CTFontCreatePathForGlyph(font, glyphs[0], nil) else {
        fatalError("\(character) has no outline in \(CTFontCopyPostScriptName(font))")
    }
    // 按字形自身的墨迹框缩放并居中，而不是按字号和步进框：한 的墨迹在 Apple SD Gothic Neo 的字身里比汉字在 PingFang 里矮一截，按同一字号画会显得小一号；各自把墨迹框的长边撑到同一尺寸，十个字的视觉大小才一致，也都落在图块正中。
    let bounds = outline.boundingBox
    let scale = side * modeGlyphFraction / max(bounds.width, bounds.height)
    var place = CGAffineTransform(translationX: side / 2, y: side / 2)
        .scaledBy(x: scale, y: scale)
        .translatedBy(x: -bounds.midX, y: -bounds.midY)
    guard let glyph = outline.copy(using: &place) else { return }
    cg.addPath(glyph)
    cg.fillPath()
}

let resources = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()   // scripts
    .deletingLastPathComponent()   // macos
    .appendingPathComponent("resources")
let destination = CommandLine.arguments.count > 1
    ? URL(fileURLWithPath: CommandLine.arguments[1])
    : resources

let staging = URL(fileURLWithPath: NSTemporaryDirectory())
    .appendingPathComponent("msime-client-menu-icon-\(ProcessInfo.processInfo.processIdentifier)")
try FileManager.default.createDirectory(at: staging, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: staging) }

func writeIcon(named name: String, draw: (CGContext, CGFloat) -> Void) throws {
    // tiffutil pairs the pages by the @2x suffix, so the staged names carry it.
    let onex = staging.appendingPathComponent("\(name).png")
    let twox = staging.appendingPathComponent("\(name)@2x.png")
    try render(pixels: 16, to: onex, draw: draw)
    try render(pixels: 32, to: twox, draw: draw)

    let output = destination.appendingPathComponent("\(name).tiff")
    let tiffutil = Process()
    tiffutil.executableURL = URL(fileURLWithPath: "/usr/bin/tiffutil")
    tiffutil.arguments = ["-cathidpicheck", onex.path, twox.path, "-out", output.path]
    try tiffutil.run()
    tiffutil.waitUntilExit()
    guard tiffutil.terminationStatus == 0 else { exit(tiffutil.terminationStatus) }
    print("Wrote \(output.path)")
}

let logo = metasequoiaStroke()
try writeIcon(named: "MSIMEClientInputMethodMenuIcon") { drawLogo(logo, in: $0, side: $1) }
// 各输入模式的图标，Info.plist.in 里每个模式各引用一张。
for (character, mode) in [("中", "Chinese"), ("双", "Shuangpin"), ("五", "Wubi"), ("粤", "Cantonese"),
                          ("注", "Zhuyin"), ("日", "Japanese"), ("한", "Korean"), ("越", "Vietnamese"),
                          ("ཀ", "Tibetan"), ("笔", "Stroke"), ("英", "English")] {
    try writeIcon(named: "MSIMEClientInputMethodMenuIcon\(mode)") { drawModeGlyph(character, in: $0, side: $1) }
}
