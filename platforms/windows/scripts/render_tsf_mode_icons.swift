#!/usr/bin/env xcrun swift

// 渲染 Windows 任务栏输入指示器的方案图标：tsf/assets 下的 shuangpin、wubi、cantonese、zhuyin、vietnamese、tibetan、stroke 各一对 -light.ico 和 -dark.ico，分别是 双、五、粤、注、越、藏、笔 一个大字铺满图块。字形取自思源黑体 SC Regular（Source Han Sans，SIL Open Font License 1.1）的轮廓，按字形自身的墨迹框缩放（与 macOS 输入菜单的 platforms/macos/scripts/render_menu_icon.swift 同一种摆法），长边撑到图块的 15/16 并居中。不用 macOS 菜单图标的 PingFang SC：那是 Apple 随系统授权的字体，它的字形不能放进 Windows 安装包分发，而 OFL 字体渲染出的图像可以随任何产品分发。字重用 Regular 而不是 macOS 的 Semibold：任务栏上并排的 中、英、日 是上游的原图，笔画是细的，同一个指示器换方案时字不应忽粗忽细。藏文画 藏 而不是 macOS 的 ཀ，与 Windows 托盘和悬浮工具栏一致。
//
// 每个 ICO 带十个尺寸（16 到 48 每隔 4，再加 64），与 jp 和 kr 两对相同；GetIcon 按显示器 DPI 把 16 换算成像素，再由 LoadImage 取最接近的一个尺寸（tsf/LanguageBar/LanguageBar.cpp）。每个尺寸是 32 位 BMP 条目加 AND 掩码，与上游的 cn、en、jp 图标同一种存法。浅色任务栏用黑字，深色任务栏用白字，颜色不预乘，形状只由 alpha 携带。
//
// 用法：xcrun swift platforms/windows/scripts/render_tsf_mode_icons.swift <SourceHanSansSC-Regular.otf> [输出目录]
// 字体文件取自 https://github.com/adobe-fonts/source-han-sans/releases/download/2.005R/09_SourceHanSansSC.zip 里的 OTF/SimplifiedChinese/SourceHanSansSC-Regular.otf，脚本按下面固定的 SHA-256 核对，换了版本的字体会被拒绝，免得同一份图标悄悄换了字形。不给目录时写到 platforms/windows/tsf/assets。改了字或字重之后重新运行，把十四个文件一起提交。

import CoreGraphics
import CoreText
import CryptoKit
import Foundation

let sizes = [16, 20, 24, 28, 32, 36, 40, 44, 48, 64]
let modeGlyphFraction: CGFloat = 15.0 / 16
let fontName = "SourceHanSansSC-Regular"
let fontSHA256 = "f1d8611151880c6c336aabeac4640ef434fa13cbfbf1ffe82d0a71b2a5637256"

guard CommandLine.arguments.count > 1 else {
    fatalError("usage: render_tsf_mode_icons.swift <SourceHanSansSC-Regular.otf> [output directory]")
}
let fontURL = URL(fileURLWithPath: CommandLine.arguments[1])
let fontBytes = try Data(contentsOf: fontURL)
let fontDigest = SHA256.hash(data: fontBytes).map { String(format: "%02x", $0) }.joined()
guard fontDigest == fontSHA256 else {
    fatalError("\(fontURL.path) is not Source Han Sans SC 2.005 Regular (SHA-256 \(fontDigest))")
}
guard let fontDescriptors = CTFontManagerCreateFontDescriptorsFromData(fontBytes as CFData) as? [CTFontDescriptor],
      let fontDescriptor = fontDescriptors.first else {
    fatalError("cannot read \(fontURL.path)")
}

let modes: [(character: String, name: String)] = [
    ("双", "shuangpin"), ("五", "wubi"), ("粤", "cantonese"), ("注", "zhuyin"),
    ("越", "vietnamese"), ("藏", "tibetan"), ("笔", "stroke"),
]

func glyphOutline(_ character: String) -> CGPath {
    let font = CTFontCreateWithFontDescriptor(fontDescriptor, 100, nil)
    guard (CTFontCopyPostScriptName(font) as String) == fontName else {
        fatalError("\(fontURL.path) is not \(fontName)")
    }
    var unichars = Array(character.utf16)
    var glyphs = [CGGlyph](repeating: 0, count: unichars.count)
    guard CTFontGetGlyphsForCharacters(font, &unichars, &glyphs, unichars.count),
          let outline = CTFontCreatePathForGlyph(font, glyphs[0], nil) else {
        fatalError("\(character) has no outline in \(fontName)")
    }
    return outline
}

// 一个尺寸的 alpha 覆盖率，行从上到下。按墨迹框缩放并居中，与 render_menu_icon.swift 的 drawModeGlyph 相同。
func coverage(of outline: CGPath, pixels: Int) -> [UInt8] {
    var alpha = [UInt8](repeating: 0, count: pixels * pixels)
    let side = CGFloat(pixels)
    alpha.withUnsafeMutableBytes { buffer in
        guard let context = CGContext(data: buffer.baseAddress, width: pixels, height: pixels,
                                      bitsPerComponent: 8, bytesPerRow: pixels, space: CGColorSpaceCreateDeviceGray(),
                                      bitmapInfo: CGImageAlphaInfo.alphaOnly.rawValue) else {
            fatalError("cannot create a \(pixels)px bitmap")
        }
        context.setShouldAntialias(true)
        let bounds = outline.boundingBox
        let scale = side * modeGlyphFraction / max(bounds.width, bounds.height)
        var place = CGAffineTransform(translationX: side / 2, y: side / 2)
            .scaledBy(x: scale, y: scale)
            .translatedBy(x: -bounds.midX, y: -bounds.midY)
        guard let glyph = outline.copy(using: &place) else { fatalError("cannot place the glyph") }
        context.addPath(glyph)
        context.setFillColor(CGColor(gray: 0, alpha: 1))
        context.fillPath()
    }
    // CGContext 的位图第一行是图块的顶行。
    return alpha
}

func appendLE16(_ value: Int, to data: inout Data) {
    data.append(UInt8(value & 0xFF))
    data.append(UInt8((value >> 8) & 0xFF))
}

func appendLE32(_ value: Int, to data: inout Data) {
    for shift in stride(from: 0, to: 32, by: 8) { data.append(UInt8((value >> shift) & 0xFF)) }
}

// 一个 ICO 里的 BMP 条目：BITMAPINFOHEADER（高度写两倍，含掩码）、自下而上的 BGRA 像素、每行补齐到 4 字节的 1 位 AND 掩码（alpha 为 0 的像素置 1）。
func bitmapEntry(alpha: [UInt8], pixels: Int, white: Bool) -> Data {
    var data = Data()
    appendLE32(40, to: &data)
    appendLE32(pixels, to: &data)
    appendLE32(pixels * 2, to: &data)
    appendLE16(1, to: &data)
    appendLE16(32, to: &data)
    appendLE32(0, to: &data)
    let maskStride = ((pixels + 31) / 32) * 4
    appendLE32(pixels * pixels * 4 + maskStride * pixels, to: &data)
    for _ in 0..<4 { appendLE32(0, to: &data) }
    let shade: UInt8 = white ? 255 : 0
    for row in (0..<pixels).reversed() {
        for column in 0..<pixels {
            data.append(contentsOf: [shade, shade, shade, alpha[row * pixels + column]])
        }
    }
    for row in (0..<pixels).reversed() {
        var line = [UInt8](repeating: 0, count: maskStride)
        for column in 0..<pixels where alpha[row * pixels + column] == 0 {
            line[column / 8] |= UInt8(0x80 >> (column % 8))
        }
        data.append(contentsOf: line)
    }
    return data
}

func icoFile(outline: CGPath, white: Bool) -> Data {
    let entries = sizes.map { bitmapEntry(alpha: coverage(of: outline, pixels: $0), pixels: $0, white: white) }
    var data = Data()
    appendLE16(0, to: &data)
    appendLE16(1, to: &data)
    appendLE16(sizes.count, to: &data)
    var offset = 6 + 16 * sizes.count
    for (pixels, entry) in zip(sizes, entries) {
        data.append(UInt8(pixels & 0xFF))
        data.append(UInt8(pixels & 0xFF))
        data.append(0)
        data.append(0)
        appendLE16(1, to: &data)
        appendLE16(32, to: &data)
        appendLE32(entry.count, to: &data)
        appendLE32(offset, to: &data)
        offset += entry.count
    }
    for entry in entries { data.append(entry) }
    return data
}

let assets = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()   // scripts
    .deletingLastPathComponent()   // windows
    .appendingPathComponent("tsf/assets")
let destination = CommandLine.arguments.count > 2 ? URL(fileURLWithPath: CommandLine.arguments[2]) : assets

for mode in modes {
    let outline = glyphOutline(mode.character)
    for (theme, white) in [("light", false), ("dark", true)] {
        let url = destination.appendingPathComponent("\(mode.name)-\(theme).ico")
        try icoFile(outline: outline, white: white).write(to: url)
        print("Wrote \(url.path)")
    }
}
