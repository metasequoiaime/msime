#!/usr/bin/env xcrun swift

// Renders the input menu's template TIFFs: MSIMEClientInputMethodMenuIcon.tiff from the stroke in MSIMEClientInputMethodMenuIcon.svg, and the two input-mode icons MSIMEClientInputMethodMenuIconChinese.tiff (中) and MSIMEClientInputMethodMenuIconEnglish.tiff (英) from the glyphs their SVGs name.
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
import CoreText
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

// The input-mode icons are one glyph each, taken from the same face their SVGs name so the SVG preview and the shipped TIFF agree. PingFang SC ships with every macOS the bundle supports; a missing face fails the script rather than rendering a substitute.
let modeGlyphFontName = "PingFangSC-Semibold"

func glyphOutline(_ character: String) throws -> CGPath {
    let font = CTFontCreateWithName(modeGlyphFontName as CFString, 64, nil)
    guard (CTFontCopyPostScriptName(font) as String) == modeGlyphFontName else {
        throw CocoaError(.fileReadNoSuchFile, userInfo: [NSLocalizedDescriptionKey: "\(modeGlyphFontName) is not installed"])
    }
    var characters = Array(character.utf16)
    var glyphs = [CGGlyph](repeating: 0, count: characters.count)
    guard CTFontGetGlyphsForCharacters(font, &characters, &glyphs, characters.count), glyphs.count == 1,
          let outline = CTFontCreatePathForGlyph(font, glyphs[0], nil) else {
        throw CocoaError(.fileReadCorruptFile, userInfo: [NSLocalizedDescriptionKey: "\(modeGlyphFontName) has no glyph for \(character)"])
    }
    // Font outlines point y up; the renderer below expects the SVG's y-down space, so flip once here.
    var flip = CGAffineTransform(scaleX: 1, y: -1)
    return outline.copy(using: &flip) ?? outline
}

// One thirty-second of the tile stays clear on every side so the round caps do not sit on the edge.
let edgeClearanceDivisor: CGFloat = 32

func render(_ stroked: CGPath, pixels: Int, to url: URL) throws {
    let bounds = stroked.boundingBox
    let side = CGFloat(pixels)
    let inset = side / edgeClearanceDivisor
    let target = side - inset * 2
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
    transform = transform.translatedBy(x: (side - bounds.width * scale) / 2,
                                       y: (side - bounds.height * scale) / 2)
    transform = transform.scaledBy(x: scale, y: -scale)
    transform = transform.translatedBy(x: -bounds.minX, y: -bounds.maxY)
    cg.concatenate(transform)
    cg.addPath(stroked)
    // Black with a live alpha channel: the menu tints this as a template, so the shape is carried by the
    // alpha and a filled background would arrive as a solid block.
    cg.setFillColor(NSColor.black.cgColor)
    cg.fillPath()

    guard let png = rep.representation(using: .png, properties: [:]) else {
        throw CocoaError(.fileWriteUnknown)
    }
    try png.write(to: url)
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

func writeIcon(_ shape: CGPath, named name: String) throws {
    // tiffutil pairs the pages by the @2x suffix, so the staged names carry it.
    let onex = staging.appendingPathComponent("\(name).png")
    let twox = staging.appendingPathComponent("\(name)@2x.png")
    try render(shape, pixels: 16, to: onex)
    try render(shape, pixels: 32, to: twox)

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
try writeIcon(try glyphOutline("中"), named: "MSIMEClientInputMethodMenuIconChinese")
try writeIcon(try glyphOutline("英"), named: "MSIMEClientInputMethodMenuIconEnglish")
