import AppKit
import SwiftUI
import Vision

@MainActor final class MacHandwritingAppearance: ObservableObject {
  static let shared = MacHandwritingAppearance()
  @Published private(set) var colorScheme: ColorScheme?

  func apply(_ preferences: NSDictionary) {
    // macOS 的手写识别板一律跟随颜色模式，不读它单独的明暗设置。
    let next: ColorScheme?
    switch preferences["theme"] as? String {
    case "light": next = .light
    case "system": next = nil
    case "dark": next = .dark
    default: next = nil
    }
    if colorScheme != next { colorScheme = next }
  }
}

enum MacHandwritingPalette {
  static func color(_ rgb: UInt32) -> Color {
    Color(.sRGB, red: Double((rgb >> 16) & 255) / 255,
      green: Double((rgb >> 8) & 255) / 255, blue: Double(rgb & 255) / 255)
  }
  static func background(light: Bool) -> Color { color(light ? 0xF7F7FA : 0x202027) }
  static func ink(light: Bool) -> Color { color(light ? 0x202027 : 0xF5F5F7) }
  static func panel(light: Bool) -> Color { color(light ? 0xFFFFFF : 0x292A2F) }
  static func canvas(light: Bool) -> Color { color(light ? 0xFBFCFB : 0x17191C) }
  static func border(light: Bool) -> Color { color(light ? 0xDDE3DF : 0x3A3D42) }
  static func accent(light: Bool) -> Color { color(light ? 0x177A47 : 0x53D28A) }
  static func accentSoft(light: Bool) -> Color { color(light ? 0xE7F5EC : 0x193D2A) }
  static func secondary(light: Bool) -> Color { color(light ? 0x68736C : 0xA7AEA9) }
}

enum MacHandwritingProvider {
  private static func localImage(for strokes: [MacInkStroke], size: Int = 420) -> CGImage? {
    guard !strokes.isEmpty,
          let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
          let context = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8,
                                  bytesPerRow: 0, space: colorSpace,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
    context.setFillColor(NSColor.white.cgColor)
    context.fill(CGRect(x: 0, y: 0, width: size, height: size))
    context.setStrokeColor(NSColor.black.cgColor)
    context.setLineWidth(7)
    context.setLineCap(.round)
    context.setLineJoin(.round)
    let coordinateScale = CGFloat(size) / 250
    for stroke in strokes where stroke.points.count > 1 {
      context.beginPath()
      let first = stroke.points[0]
      context.move(to: CGPoint(x: first.x * coordinateScale, y: CGFloat(size) - first.y * coordinateScale))
      for point in stroke.points.dropFirst() {
        context.addLine(to: CGPoint(x: point.x * coordinateScale, y: CGFloat(size) - point.y * coordinateScale))
      }
      context.strokePath()
    }
    return context.makeImage()
  }

  private static func isCJK(_ value: String) -> Bool {
    value.unicodeScalars.contains { scalar in
      (0x3400...0x4DBF).contains(scalar.value) || (0x4E00...0x9FFF).contains(scalar.value) ||
        (0xF900...0xFAFF).contains(scalar.value)
    }
  }

  static func recognizeLocal(_ strokes: [MacInkStroke]) throws -> [String] {
    guard let image = localImage(for: strokes) else { throw NSError(domain: "MSIMEHandwriting", code: 400) }
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    request.recognitionLanguages = ["zh-Hans", "en-US"]
    let handler = VNImageRequestHandler(cgImage: image, options: [:])
    try handler.perform([request])
    let values = (request.results ?? []).flatMap { observation in
      observation.topCandidates(5).map(\.string)
    }
    var candidates: [String] = []
    for value in values where !value.isEmpty && !candidates.contains(value) {
      if candidates.count < 12 { candidates.append(value) }
    }
    let chinese = candidates.filter(isCJK)
    let others = candidates.filter { !isCJK($0) }
    return chinese + others
  }

  static func recognize(_ strokes: [MacInkStroke], language: String = "zh-CN", socketPath: String) throws -> [String] {
    if socketPath.isEmpty { return try recognizeLocal(strokes) }
    let payload = strokes.map { $0.points.map { ["x": Float($0.x), "y": Float($0.y)] } }
    let request: NSDictionary = ["language": language, "strokes": payload, "socket_path": socketPath]
    guard let type = NSClassFromString("MSIMEClientSession") as? NSObject.Type,
          let result = type.perform(NSSelectorFromString("handwritingProviderRequest:"), with: request)?.takeUnretainedValue() as? NSDictionary else { throw NSError(domain: "MSIMEHandwriting", code: 503) }
    if let error = result["error"] as? NSError { throw error }
    return (result["candidates"] as? [String]) ?? []
  }
}

struct MacInkStroke: Identifiable {
  let id = UUID()
  var points: [CGPoint]
}

private struct MacInkCanvas: NSViewRepresentable {
  @Binding var strokes: [MacInkStroke]
  var dark: Bool
  func makeNSView(context: Context) -> CanvasView { let view = CanvasView(); view.dark = dark; view.onChange = { strokes = $0 }; return view }
  func updateNSView(_ view: CanvasView, context: Context) { view.dark = dark; view.strokes = strokes; view.needsDisplay = true }
  final class CanvasView: NSView {
    var strokes: [MacInkStroke] = []
    var dark = false
    var onChange: (([MacInkStroke]) -> Void)?
    private var active: [CGPoint] = []
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
      (dark ? NSColor(calibratedRed: 0.09, green: 0.10, blue: 0.11, alpha: 1) : NSColor(calibratedRed: 0.985, green: 0.99, blue: 0.985, alpha: 1)).setFill()
      bounds.fill()
      let guide = NSBezierPath()
      guide.lineWidth = 1
      guide.setLineDash([5, 7], count: 2, phase: 0)
      guide.move(to: NSPoint(x: bounds.midX, y: 18)); guide.line(to: NSPoint(x: bounds.midX, y: bounds.height - 18))
      guide.move(to: NSPoint(x: 18, y: bounds.midY)); guide.line(to: NSPoint(x: bounds.width - 18, y: bounds.midY))
      (dark ? NSColor(calibratedWhite: 1, alpha: 0.09) : NSColor(calibratedWhite: 0, alpha: 0.075)).setStroke()
      guide.stroke()
      if strokes.isEmpty && active.isEmpty {
        let title: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 17, weight: .medium), .foregroundColor: NSColor.secondaryLabelColor]
        let hint: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 12), .foregroundColor: NSColor.tertiaryLabelColor]
        let primary = NSString(string: "在这里写字")
        let secondary = NSString(string: "支持鼠标与触控板")
        let primarySize = primary.size(withAttributes: title)
        let secondarySize = secondary.size(withAttributes: hint)
        primary.draw(at: NSPoint(x: (bounds.width - primarySize.width) / 2, y: bounds.midY - 24), withAttributes: title)
        secondary.draw(at: NSPoint(x: (bounds.width - secondarySize.width) / 2, y: bounds.midY + 5), withAttributes: hint)
      }
      (dark ? NSColor(calibratedRed: 0.33, green: 0.89, blue: 0.59, alpha: 1) : NSColor(calibratedRed: 0.08, green: 0.43, blue: 0.25, alpha: 1)).setStroke()
      for stroke in strokes + (active.isEmpty ? [] : [MacInkStroke(points: active)]) {
        guard stroke.points.count > 1 else { continue }
        let path = NSBezierPath(); path.lineWidth = 4; path.lineCapStyle = .round; path.lineJoinStyle = .round
        path.move(to: stroke.points[0]); for point in stroke.points.dropFirst() { path.line(to: point) }; path.stroke()
      }
    }
    override func mouseDown(with event: NSEvent) { active = [convert(event.locationInWindow, from: nil)]; needsDisplay = true }
    override func mouseDragged(with event: NSEvent) { active.append(convert(event.locationInWindow, from: nil)); needsDisplay = true }
    override func mouseUp(with event: NSEvent) { active.append(convert(event.locationInWindow, from: nil)); if active.count > 1 { strokes.append(MacInkStroke(points: active)); onChange?(strokes) }; active = []; needsDisplay = true }
  }
}

struct MacHandwritingCanvasView: View {
  @Environment(\.colorScheme) private var colorScheme
  @Binding var strokes: [MacInkStroke]
  var onSubmit: ([MacInkStroke]) -> Void
  var candidates: [String] = []
  var candidateAction = "复制"
  var paletteLight: Bool?
  var snapshotCanvas = false
  var onCandidate: (String) -> Void = { text in NotificationCenter.default.post(name: .msimeHandwritingCandidateSelected, object: nil, userInfo: ["text": text]) }
  var body: some View {
    let light = paletteLight ?? (colorScheme == .light)
    HStack(alignment: .top, spacing: 16) {
      VStack(spacing: 12) {
        Group {
          if snapshotCanvas {
            MacHandwritingCanvasSnapshot(light: light, hasInk: !candidates.isEmpty)
          } else {
            MacInkCanvas(strokes: $strokes, dark: !light)
          }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(MacHandwritingPalette.canvas(light: light))
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(MacHandwritingPalette.border(light: light), lineWidth: 1))
        .shadow(color: .black.opacity(light ? 0.06 : 0.18), radius: 12, y: 5)
        HStack(spacing: 8) {
          MacHandwritingActionButton(title: "撤销一笔", symbol: "arrow.uturn.backward", light: light) {
            _ = strokes.popLast(); onSubmit(strokes)
          }.disabled(strokes.isEmpty)
          MacHandwritingActionButton(title: "清空", symbol: "trash", light: light, destructive: true) {
            strokes.removeAll(); onSubmit([])
          }.disabled(strokes.isEmpty)
          Spacer()
          Text(strokes.isEmpty ? "等待书写" : "\(strokes.count) 笔")
            .font(.caption.weight(.medium)).foregroundStyle(MacHandwritingPalette.secondary(light: light))
        }
      }
      .frame(minWidth: 330, maxWidth: .infinity)

      VStack(alignment: .leading, spacing: 12) {
        HStack {
          Text("候选字").font(.headline)
          Spacer()
          if !candidates.isEmpty {
            Text("\(candidates.count) 个")
              .font(.caption.weight(.semibold)).foregroundStyle(MacHandwritingPalette.accent(light: light))
              .padding(.horizontal, 8).padding(.vertical, 4)
              .background(MacHandwritingPalette.accentSoft(light: light), in: Capsule())
          }
        }
        if candidates.isEmpty {
          VStack(spacing: 10) {
            Image(systemName: "character.cursor.ibeam").font(.system(size: 28, weight: .light))
            Text("候选会显示在这里").font(.subheadline.weight(.medium))
            Text("每写完一笔都会自动更新").font(.caption).foregroundStyle(MacHandwritingPalette.secondary(light: light))
          }
          .foregroundStyle(MacHandwritingPalette.secondary(light: light))
          .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
          Grid(horizontalSpacing: 8, verticalSpacing: 8) {
            ForEach(0..<((candidates.count + 2) / 3), id: \.self) { row in
              GridRow {
                ForEach(0..<3, id: \.self) { column in
                  let index = row * 3 + column
                  if index < candidates.count {
                    let candidate = candidates[index]
                    MacHandwritingCandidateButton(candidate: candidate, light: light) { onCandidate(candidate) }
                  } else {
                    Color.clear.frame(minHeight: 54)
                  }
                }
              }
            }
          }.padding(.vertical, 2).frame(maxHeight: .infinity, alignment: .top)
        }
        Text(candidates.isEmpty ? "先在左侧写一个汉字" : "点击候选即可\(candidateAction)")
          .font(.caption).foregroundStyle(MacHandwritingPalette.secondary(light: light))
      }
      .padding(16)
      .frame(width: 250)
      .frame(maxHeight: .infinity, alignment: .top)
      .background(MacHandwritingPalette.panel(light: light), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
      .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(MacHandwritingPalette.border(light: light), lineWidth: 1))
    }
  }
}

private struct MacHandwritingCanvasSnapshot: View {
  let light: Bool
  let hasInk: Bool
  var body: some View {
    GeometryReader { geometry in
      ZStack {
        MacHandwritingPalette.canvas(light: light)
        Path { path in
          path.move(to: CGPoint(x: geometry.size.width / 2, y: 18))
          path.addLine(to: CGPoint(x: geometry.size.width / 2, y: geometry.size.height - 18))
          path.move(to: CGPoint(x: 18, y: geometry.size.height / 2))
          path.addLine(to: CGPoint(x: geometry.size.width - 18, y: geometry.size.height / 2))
        }
        .stroke(MacHandwritingPalette.ink(light: light).opacity(0.09), style: StrokeStyle(lineWidth: 1, dash: [5, 7]))
        if hasInk {
          Path { path in
            let center = CGPoint(x: geometry.size.width / 2, y: geometry.size.height / 2)
            path.move(to: CGPoint(x: center.x - 45, y: center.y - 72)); path.addLine(to: CGPoint(x: center.x + 36, y: center.y - 83))
            path.move(to: CGPoint(x: center.x, y: center.y - 103)); path.addLine(to: CGPoint(x: center.x + 3, y: center.y + 104))
            path.move(to: CGPoint(x: center.x - 92, y: center.y - 17)); path.addLine(to: CGPoint(x: center.x - 37, y: center.y + 12)); path.addLine(to: CGPoint(x: center.x - 78, y: center.y + 76))
            path.move(to: CGPoint(x: center.x + 77, y: center.y - 47)); path.addLine(to: CGPoint(x: center.x + 28, y: center.y + 4)); path.addLine(to: CGPoint(x: center.x + 91, y: center.y + 74))
          }.stroke(MacHandwritingPalette.accent(light: light), style: StrokeStyle(lineWidth: 4, lineCap: .round, lineJoin: .round))
        } else {
          VStack(spacing: 5) {
            Text("在这里写字").font(.system(size: 17, weight: .medium))
            Text("支持鼠标与触控板").font(.system(size: 12))
          }.foregroundStyle(MacHandwritingPalette.secondary(light: light))
        }
      }
    }
  }
}

private struct MacHandwritingActionButton: View {
  let title: String
  let symbol: String
  let light: Bool
  var destructive = false
  let action: () -> Void
  var body: some View {
    Button(action: action) {
      Label(title, systemImage: symbol).font(.system(size: 12, weight: .medium))
        .padding(.horizontal, 11).padding(.vertical, 7)
        .background(MacHandwritingPalette.panel(light: light), in: Capsule())
        .overlay(Capsule().stroke(MacHandwritingPalette.border(light: light), lineWidth: 1))
    }
    .buttonStyle(.plain)
    .foregroundStyle(destructive ? Color.red : MacHandwritingPalette.ink(light: light))
  }
}

private struct MacHandwritingCandidateButton: View {
  let candidate: String
  let light: Bool
  let action: () -> Void
  @State private var hovered = false
  var body: some View {
    Button(action: action) {
      Text(candidate).font(.system(size: 25, weight: .medium)).lineLimit(1).minimumScaleFactor(0.65)
        .frame(maxWidth: .infinity, minHeight: 54)
        .background(hovered ? MacHandwritingPalette.accentSoft(light: light) : MacHandwritingPalette.canvas(light: light),
                    in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous)
          .stroke(hovered ? MacHandwritingPalette.accent(light: light) : MacHandwritingPalette.border(light: light), lineWidth: 1))
    }
    .buttonStyle(.plain)
    .foregroundStyle(MacHandwritingPalette.ink(light: light))
    .onHover { hovered = $0 }
    .accessibilityLabel("候选 \(candidate)")
  }
}

extension Notification.Name { static let msimeHandwritingCandidateSelected = Notification.Name("MSIMEHandwritingCandidateSelected") }

/// Own the window's presentation state separately from the reusable ink canvas.
struct MacHandwritingToolView: View {
  @ObservedObject var appearance = MacHandwritingAppearance.shared
  @Environment(\.colorScheme) private var systemColorScheme
  var onCandidate: ((String) -> Bool)?
  var snapshotCanvas = false
  var previewCandidates: [String] = []
  @State private var strokes: [MacInkStroke] = []
  @State private var candidates: [String] = []
  @State private var socketPath = ""
  @State private var message: String?
  @State private var busy = false
  @State private var pending: Task<Void, Never>?
  @State private var showsAdvancedService = false
  var body: some View {
    let light = (appearance.colorScheme ?? systemColorScheme) == .light
    let shownCandidates = previewCandidates.isEmpty ? candidates : previewCandidates
    VStack(alignment: .leading, spacing: 16) {
      HStack(spacing: 12) {
        Image(systemName: "hand.draw.fill")
          .font(.system(size: 20, weight: .semibold)).foregroundStyle(MacHandwritingPalette.accent(light: light))
          .frame(width: 42, height: 42).background(MacHandwritingPalette.accentSoft(light: light), in: RoundedRectangle(cornerRadius: 13, style: .continuous))
        VStack(alignment: .leading, spacing: 2) {
          Text("手写输入").font(.title3.weight(.semibold))
          Text("写一个汉字，然后从候选中选择").font(.subheadline).foregroundStyle(MacHandwritingPalette.secondary(light: light))
        }
        Spacer()
        if busy { ProgressView().controlSize(.small).help("正在识别") }
      }
      MacHandwritingCanvasView(strokes: $strokes, onSubmit: recognize, candidates: shownCandidates,
        candidateAction: onCandidate == nil ? "复制" : "输入", paletteLight: light,
        snapshotCanvas: snapshotCanvas, onCandidate: { text in
        if let onCandidate {
          if !onCandidate(text) { message = "原输入位置已失效，请返回编辑器后重新打开手写板。" }
        } else {
          NSPasteboard.general.clearContents()
          message = NSPasteboard.general.setString(text, forType: .string) ? "已复制" : "复制失败，请重试。"
        }
      })
        .disabled(busy)
      HStack(spacing: 8) {
        if let message {
          Image(systemName: message == "已复制" ? "checkmark.circle.fill" : "info.circle")
          Text(message).lineLimit(1)
        } else {
          Image(systemName: "sparkles")
          Text(busy ? "正在识别…" : "笔迹只用于本次识别")
        }
        Spacer()
        VStack(alignment: .leading, spacing: 6) {
          Button {
            withAnimation(.easeInOut(duration: 0.16)) { showsAdvancedService.toggle() }
          } label: {
            Label("高级识别服务", systemImage: showsAdvancedService ? "chevron.down" : "chevron.right")
              .foregroundStyle(MacHandwritingPalette.secondary(light: light))
          }.buttonStyle(.plain)
          if showsAdvancedService {
            TextField("Provider socket 路径", text: $socketPath).textFieldStyle(.roundedBorder).disabled(busy)
              .transition(.opacity.combined(with: .move(edge: .top)))
          }
        }.frame(width: 210)
      }
      .font(.caption).foregroundStyle(MacHandwritingPalette.secondary(light: light))
    }.padding(22).frame(width: 720, height: 510)
    .background(MacHandwritingPalette.background(light: light))
    .foregroundStyle(MacHandwritingPalette.ink(light: light))
    .preferredColorScheme(appearance.colorScheme)
    .onChange(of: strokes.count) { _ in candidates = []; if !strokes.isEmpty { recognize(strokes) } }
    .onChange(of: socketPath) { _ in candidates = []; message = nil }
    .onDisappear { pending?.cancel(); pending = nil; candidates = []; strokes = [] }
  }
  private func recognize(_ ink: [MacInkStroke]) {
    guard !busy else { return }
    busy = true; candidates = []; message = nil
    let path = socketPath
    pending = Task { @MainActor in
      defer { busy = false }
      do {
        let work = Task.detached(priority: .userInitiated) {
          try MacHandwritingProvider.recognize(ink, socketPath: path)
        }
        let result = try await withTaskCancellationHandler(operation: { try await work.value }, onCancel: { work.cancel() })
        try Task.checkCancellation()
        candidates = result
        if result.isEmpty { message = "没有识别到候选，请重新书写。" }
      } catch {
        if !Task.isCancelled { message = error.localizedDescription }
      }
    }
  }
}
