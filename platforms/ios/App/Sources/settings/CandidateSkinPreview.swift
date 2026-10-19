import SwiftUI
import UIKit

/// 外部皮肤选择器使用键盘的画布与样本文字；图片解码不占用设置页主线程。
struct CandidateSkinPreview: UIViewRepresentable {
  let preferences: [String: Any]
  let systemDark: Bool
  let resourceRevision: UUID
  var skinsRoot: URL? = ExternalCandidateSkin.defaultRoot

  func makeUIView(context: Context) -> CandidateSkinPreviewRow { CandidateSkinPreviewRow() }
  func makeCoordinator() -> Coordinator { Coordinator() }

  func updateUIView(_ view: CandidateSkinPreviewRow, context: Context) {
    guard let request = Request(preferences: preferences, systemDark: systemDark,
                                resourceRevision: resourceRevision, root: skinsRoot) else {
      context.coordinator.invalidate(view)
      return
    }
    context.coordinator.update(view, request: request)
  }

  static func dismantleUIView(_ view: CandidateSkinPreviewRow, coordinator: Coordinator) {
    coordinator.invalidate(view)
  }

  struct Request: Equatable {
    let document: Data
    let systemDark: Bool
    let resourceRevision: UUID
    let root: URL?

    init?(preferences: [String: Any], systemDark: Bool, resourceRevision: UUID, root: URL?) {
      guard let document = try? JSONSerialization.data(withJSONObject: preferences, options: .sortedKeys) else { return nil }
      self.document = document
      self.systemDark = systemDark
      self.resourceRevision = resourceRevision
      self.root = root
    }
  }

  struct Snapshot {
    let palette: CandidatePalette
    let canvas: ExternalCandidateSkin.Canvas?
  }

  /// 请求身份包含资源代次，同名包替换或删除后也会重新加载；旧回调不能覆盖新选择。
  @MainActor final class Coordinator {
    private static let queue = DispatchQueue(label: "app.msime.settings-canvas", qos: .userInitiated)
    private let queue: DispatchQueue
    private let load: (Request) -> Snapshot
    private var current: Request?
    private var generation: UInt64 = 0

    init(queue: DispatchQueue? = nil, load: ((Request) -> Snapshot)? = nil) {
      self.queue = queue ?? Self.queue
      self.load = load ?? Self.loadSnapshot
    }

    func update(_ view: CandidateSkinPreviewRow, request: Request) {
      guard current != request else { return }
      current = request
      generation &+= 1
      let expected = generation
      view.apply(Snapshot(palette: CandidatePalette.palette(.system), canvas: nil))
      let load = load
      queue.async { [weak self, weak view] in
        let snapshot = autoreleasepool { load(request) }
        DispatchQueue.main.async { [weak self, weak view] in
          guard let self, let view, generation == expected else { return }
          view.apply(snapshot)
        }
      }
    }

    func invalidate(_ view: CandidateSkinPreviewRow) {
      generation &+= 1
      current = nil
      view.apply(Snapshot(palette: CandidatePalette.palette(.system), canvas: nil))
    }

    nonisolated private static func loadSnapshot(_ request: Request) -> Snapshot {
      let preferences = (try? JSONSerialization.jsonObject(with: request.document)) as? [String: Any] ?? [:]
      let fallback = CandidatePalette.resolveTheme(preferences, systemDark: request.systemDark, skinsRoot: request.root) ?? .system
      let dark = CandidatePalette.isDark(candidateTheme: preferences["candidate_theme"] as? String,
                                         appMode: preferences["theme"] as? String, systemDark: request.systemDark)
      let canvas = fallback.candidateSkin.flatMap { ExternalCandidateSkin.canvas($0, dark: dark, root: request.root) }
      let resolved = canvas?.panelLoaded == true
        ? CandidatePalette.resolveTheme(preferences, systemDark: request.systemDark, skinsRoot: request.root, canvasPanelDrawn: true) ?? fallback
        : fallback
      return Snapshot(palette: CandidatePalette.palette(resolved), canvas: canvas)
    }
  }
}

/// 内容布局指南让小尺寸预览与键盘一样缩小过大的留白，挂件仍保留前后层级。
final class CandidateSkinPreviewRow: CandidateCanvasHostView {
  private let row = UIStackView()
  private var items: [(UIView, UILabel, UILabel)] = []

  init() {
    super.init(frame: .zero)
    minimumContentHeight = max(40, UIFont.preferredFont(forTextStyle: .body).lineHeight + 12)
    minimumContentWidth = 120
    row.axis = .horizontal
    row.spacing = 6
    row.distribution = .fillEqually
    row.translatesAutoresizingMaskIntoConstraints = false
    addSubview(row)
    NSLayoutConstraint.activate([
      row.leadingAnchor.constraint(equalTo: candidateContentGuide.leadingAnchor, constant: 8),
      row.trailingAnchor.constraint(equalTo: candidateContentGuide.trailingAnchor, constant: -8),
      row.topAnchor.constraint(equalTo: candidateContentGuide.topAnchor, constant: 6),
      row.bottomAnchor.constraint(equalTo: candidateContentGuide.bottomAnchor, constant: -6),
    ])
    for (index, word) in ["水杉", "输入", "输入法"].enumerated() {
      let number = UILabel()
      number.text = "\(index + 1)"
      number.setContentHuggingPriority(.required, for: .horizontal)
      number.setContentCompressionResistancePriority(.required, for: .horizontal)
      number.font = .preferredFont(forTextStyle: .caption1)
      number.adjustsFontForContentSizeCategory = true
      let text = UILabel()
      text.text = word
      text.font = .preferredFont(forTextStyle: .body)
      text.adjustsFontForContentSizeCategory = true
      text.adjustsFontSizeToFitWidth = true
      text.minimumScaleFactor = 0.8
      let stack = UIStackView(arrangedSubviews: [number, text])
      stack.spacing = 3
      stack.alignment = .center
      stack.translatesAutoresizingMaskIntoConstraints = false
      let item = UIView()
      item.addSubview(stack)
      NSLayoutConstraint.activate([
        stack.leadingAnchor.constraint(equalTo: item.leadingAnchor, constant: 5),
        stack.trailingAnchor.constraint(equalTo: item.trailingAnchor, constant: -5),
        stack.topAnchor.constraint(equalTo: item.topAnchor),
        stack.bottomAnchor.constraint(equalTo: item.bottomAnchor),
      ])
      row.addArrangedSubview(item)
      items.append((item, number, text))
    }
    isAccessibilityElement = true
    accessibilityLabel = "候选预览：水杉，输入，输入法"
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  func apply(_ snapshot: CandidateSkinPreview.Snapshot) {
    canvas = snapshot.canvas
    let palette = snapshot.palette
    backgroundColor = canvas?.panelLoaded == true ? .clear : palette.surface
    layer.cornerRadius = canvas == nil ? 12 : 0
    for (index, item) in items.enumerated() {
      item.1.textColor = palette.number
      item.2.textColor = index == 0 && canvas != nil ? palette.selectedText ?? palette.text : palette.text
      item.0.backgroundColor = index == 0 ? (canvas == nil ? palette.hover : palette.selected) : .clear
      item.0.layer.cornerRadius = canvas?.itemCornerRadius ?? 9
      item.0.layer.borderWidth = canvas == nil ? 1 : 0
      item.0.layer.borderColor = palette.border.cgColor
    }
  }
}
