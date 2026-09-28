#!/usr/bin/env xcrun swift

// Renders the input menu's template TIFFs from the stroke in MSIMEClientInputMethodMenuIcon.svg: MSIMEClientInputMethodMenuIcon.tiff is the bare logo the bundle names, and each input mode gets the logo with a corner badge carrying its own character, 中, 日 or 英. With the bare logo on all three modes the menu bar and the system's Ctrl+Space switcher showed three identical icons, and nothing told the modes apart.
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

// 标志本身，逐点取自产品图稿 apps/desktop/app-icon.svg 里那条 stroke="white" 的路径：110 单位画布、
// stroke-width 8、round cap、默认的 miter 接角。坐标 y 向下，与 SVG 一致。
//
// 这里原本是一条照着图稿描出来的近似折线（等宽 4.5、round 接角），形状对不上：最后一笔在图稿里是从右上
// 长扫到左下收尾，描出来那条却收在右下。菜单栏上的记号必须和应用图标是同一个，所以直接用图稿的路径，
// 旁边的 SVG 与它保持一致，改动其一就要同步另一个。
func metasequoiaStroke() -> CGPath {
    let path = CGMutablePath()
    path.move(to: CGPoint(x: 74.7234, y: 14))
    path.addLine(to: CGPoint(x: 35.1501, y: 29.1727))
    path.addLine(to: CGPoint(x: 74.7234, y: 40.5522))
    path.addLine(to: CGPoint(x: 35.1501, y: 59.518))
    path.addCurve(to: CGPoint(x: 33, y: 95),
                  control1: CGPoint(x: 72.562, y: 65.84),
                  control2: CGPoint(x: 107.728, y: 71.024))
    return path.copy(strokingWithWidth: 8, lineCap: .round, lineJoin: .miter, miterLimit: 4)
}

// One thirty-second of the tile stays clear on every side so the round caps do not sit on the edge.
let edgeClearanceDivisor: CGFloat = 32

func render(_ stroked: CGPath, badge: String?, pixels: Int, to url: URL) throws {
    let bounds = stroked.boundingBox
    let side = CGFloat(pixels)
    let inset = side / edgeClearanceDivisor
    // A badged icon draws the logo into the top-left share of the tile, so the badge takes the bottom-right corner without covering the stroke.
    let area = badge == nil ? side : side * badgedLogoFraction
    let target = area - inset * 2
    let scale = min(target / bounds.width, target / bounds.height)

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

    // The bitmap's origin is bottom-left and the SVG's is top-left, so the drawing is flipped back after
    // being scaled to fit and centred.
    var transform = CGAffineTransform.identity
    transform = transform.translatedBy(x: (area - bounds.width * scale) / 2,
                                       y: side - area + (area - bounds.height * scale) / 2)
    transform = transform.scaledBy(x: scale, y: -scale)
    transform = transform.translatedBy(x: -bounds.minX, y: -bounds.maxY)
    cg.concatenate(transform)
    cg.addPath(stroked)
    // Black with a live alpha channel: the menu tints this as a template, so the shape is carried by the
    // alpha and a filled background would arrive as a solid block.
    cg.setFillColor(NSColor.black.cgColor)
    cg.fillPath()
    if let badge { drawBadge(badge, in: cg, side: side) }

    guard let png = rep.representation(using: .png, properties: [:]) else {
        throw CocoaError(.fileWriteUnknown)
    }
    try png.write(to: url)
}

// 角标：右下角一块圆角方块，模式的字从方块里镂空出来。整张图仍是纯黑加 alpha 的模板图，所以角标跟着菜单一起着色，不带自己的颜色。方块四周再清出一圈空白，标志的笔画经过角落时不会和方块粘成一团。
// The badge takes a little over half the tile: at the 16-pixel page that leaves the character about seven pixels tall, the smallest at which 中, 日 and 英 still read apart.
let badgeSideFraction: CGFloat = 0.5
let badgedLogoFraction: CGFloat = 0.7
let badgeHaloFraction: CGFloat = 1.0 / 32
let badgeCornerFraction: CGFloat = 0.22
let badgeGlyphFraction: CGFloat = 0.78

func badgeFont(size: CGFloat) -> CTFont {
    // PingFang ships with every supported macOS; Hiragino Sans GB is the fallback the system itself uses for Simplified Chinese.
    for name in ["PingFangSC-Semibold", "HiraginoSansGB-W6"] {
        let font = CTFontCreateWithName(name as CFString, size, nil)
        if (CTFontCopyPostScriptName(font) as String) == name { return font }
    }
    fatalError("no CJK font for the menu icon badge")
}

func drawBadge(_ character: String, in cg: CGContext, side: CGFloat) {
    let badgeSide = (side * badgeSideFraction).rounded()
    let badge = CGRect(x: side - badgeSide, y: 0, width: badgeSide, height: badgeSide)
    let halo = side * badgeHaloFraction
    cg.saveGState()
    defer { cg.restoreGState() }
    cg.concatenate(cg.ctm.inverted())
    cg.clear(badge.insetBy(dx: -halo, dy: -halo))
    let corner = badgeSide * badgeCornerFraction
    cg.addPath(CGPath(roundedRect: badge, cornerWidth: corner, cornerHeight: corner, transform: nil))
    cg.setFillColor(NSColor.black.cgColor)
    cg.fillPath()

    let font = badgeFont(size: badgeSide * badgeGlyphFraction)
    var unichars = Array(character.utf16)
    var glyphs = [CGGlyph](repeating: 0, count: unichars.count)
    guard CTFontGetGlyphsForCharacters(font, &unichars, &glyphs, unichars.count),
          let outline = CTFontCreatePathForGlyph(font, glyphs[0], nil) else {
        fatalError("\(character) has no outline in \(CTFontCopyPostScriptName(font))")
    }
    // Centre the outline's own bounds rather than the advance box, so each character sits optically in the middle of the badge.
    let bounds = outline.boundingBox
    var place = CGAffineTransform(translationX: badge.midX - bounds.midX, y: badge.midY - bounds.midY)
    guard let glyph = outline.copy(using: &place) else { return }
    cg.addPath(glyph)
    cg.setBlendMode(.clear)
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

func writeIcon(_ shape: CGPath, badge: String? = nil, named name: String) throws {
    // tiffutil pairs the pages by the @2x suffix, so the staged names carry it.
    let onex = staging.appendingPathComponent("\(name).png")
    let twox = staging.appendingPathComponent("\(name)@2x.png")
    try render(shape, badge: badge, pixels: 16, to: onex)
    try render(shape, badge: badge, pixels: 32, to: twox)

    let output = destination.appendingPathComponent("\(name).tiff")
    let tiffutil = Process()
    tiffutil.executableURL = URL(fileURLWithPath: "/usr/bin/tiffutil")
    tiffutil.arguments = ["-cathidpicheck", onex.path, twox.path, "-out", output.path]
    try tiffutil.run()
    tiffutil.waitUntilExit()
    guard tiffutil.terminationStatus == 0 else { exit(tiffutil.terminationStatus) }
    print("Wrote \(output.path)")
}

try writeIcon(metasequoiaStroke(), named: "MSIMEClientInputMethodMenuIcon")
// The input modes' icons, named in Info.plist.in beside each mode.
try writeIcon(metasequoiaStroke(), badge: "中", named: "MSIMEClientInputMethodMenuIconChinese")
try writeIcon(metasequoiaStroke(), badge: "日", named: "MSIMEClientInputMethodMenuIconJapanese")
try writeIcon(metasequoiaStroke(), badge: "英", named: "MSIMEClientInputMethodMenuIconEnglish")
