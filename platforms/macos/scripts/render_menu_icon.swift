#!/usr/bin/env xcrun swift

// Renders the input menu's template TIFF, MSIMEClientInputMethodMenuIcon.tiff, from the stroke in MSIMEClientInputMethodMenuIcon.svg. The bundle and all three input modes use it.
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

// The path from the SVG, in that file's user units, y pointing down. Keep the two in step: the SVG is what
// a designer edits, this is what ships, and the viewBox there is the square this computes.
func metasequoiaStroke() -> CGPath {
    let path = CGMutablePath()
    path.move(to: CGPoint(x: 25, y: 6.5))
    path.addLine(to: CGPoint(x: 7.5, y: 14))
    path.addLine(to: CGPoint(x: 24.5, y: 18.25))
    path.addLine(to: CGPoint(x: 7.5, y: 25.75))
    path.addCurve(to: CGPoint(x: 25, y: 28.5),
                  control1: CGPoint(x: 12, y: 29.25),
                  control2: CGPoint(x: 18, y: 31))
    return path.copy(strokingWithWidth: 4.5, lineCap: .round, lineJoin: .round, miterLimit: 10)
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
