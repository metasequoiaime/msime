import UIKit
@preconcurrency import MLKitDigitalInkRecognition
@preconcurrency import MLKitCommon

private final class HandwritingDownloadFailure: @unchecked Sendable {
  private let lock = NSLock()
  private var storedError: Error?
  func record(_ error: Error) { lock.lock(); defer { lock.unlock() }; storedError = error }
  var error: Error? { lock.lock(); defer { lock.unlock() }; return storedError }
}

@MainActor
final class HandwritingRecognizer {
  init() {
    // ML Kit creates its download session while checking model availability, before download().
    _ = HandwritingDownloadSession.configureSharedContainer(InputSchemePreference.appGroupIdentifier)
  }
  private lazy var model = DigitalInkRecognitionModel(modelIdentifier:
    .zhHaniCn)
  private var engine: DigitalInkRecognizer?
  func release() { engine = nil }
  var isReady: Bool { ModelManager.modelManager().isModelDownloaded(model) }

  func download(onProgress: (Double) -> Void) async throws {
    if isReady { return }
    guard HandwritingDownloadSession.configureSharedContainer(InputSchemePreference.appGroupIdentifier) else {
      throw NSError(domain: "MSIMEHandwriting", code: 4,
        userInfo: [NSLocalizedDescriptionKey: "无法访问手写模型共享目录，请检查完全访问权限"])
    }
    let failure = HandwritingDownloadFailure()
    let languageTag = model.modelIdentifier.languageTag
    let observer = NotificationCenter.default.addObserver(forName: .mlkitModelDownloadDidFail,
      object: nil, queue: nil) { notification in
      guard let remote = notification.userInfo?[ModelDownloadUserInfoKey.remoteModel.rawValue] as? DigitalInkRecognitionModel,
        remote.modelIdentifier.languageTag == languageTag else { return }
      failure.record(notification.userInfo?[ModelDownloadUserInfoKey.error.rawValue] as? Error
        ?? NSError(domain: "MSIMEHandwriting", code: 3))
    }
    defer { NotificationCenter.default.removeObserver(observer) }
    let progress = ModelManager.modelManager().download(model, conditions:
      ModelDownloadConditions(allowsCellularAccess: true, allowsBackgroundDownloading: false))
    defer { if !isReady { progress.cancel() } }
    for _ in 0..<600 {
      try Task.checkCancellation()
      if isReady { return }
      if let error = failure.error { throw error }
      onProgress(progress.fractionCompleted)
      if progress.isCancelled { throw CancellationError() }
      try await Task.sleep(nanoseconds: 300_000_000)
    }
    throw NSError(domain: "MSIMEHandwriting", code: 2,
      userInfo: [NSLocalizedDescriptionKey: "模型下载未完成，请检查网络后重试"])
  }

  func recognize(_ strokes: [[CGPoint]], width: Double, height: Double) async throws -> [String] {
    try Task.checkCancellation()
    guard isReady else {
      throw NSError(domain: "MSIMEHandwriting", code: 1,
        userInfo: [NSLocalizedDescriptionKey: "请先下载中文手写模型"])
    }
    let ink = Ink(strokes: strokes.map { Stroke(points: $0.map { StrokePoint(x: Float($0.x), y: Float($0.y)) }) })
    let context = DigitalInkRecognitionContext(preContext: "", writingArea:
      WritingArea(width: Float(width), height: Float(height)))
    if engine == nil { engine = DigitalInkRecognizer.digitalInkRecognizer(options: DigitalInkRecognizerOptions(model: model)) }
    let recognizer = engine!
    let words: [String] = try await withCheckedThrowingContinuation { continuation in
      recognizer.recognize(ink: ink, context: context) { result, error in
        if let error { continuation.resume(throwing: error); return }
        var seen = Set<String>()
        continuation.resume(returning: (result?.candidates ?? []).map { $0.text.trimmingCharacters(in: .whitespacesAndNewlines) }
          .filter { !$0.isEmpty && seen.insert($0).inserted }.prefix(12).map { $0 })
      }
    }
    try Task.checkCancellation()
    return words
  }
}

final class HandwritingCanvas: UIView {
  private(set) var strokes: [[CGPoint]] = []
  /// The visible card is only the drawing guide; ink remains accepted across the whole panel.
  var cardRect: CGRect = .zero { didSet { if cardRect != oldValue { setNeedsDisplay() } } }
  var onChange: (() -> Void)?
  var onStrokeBegan: (() -> Void)?
  var acceptsInk = true
  private var drawing = false
  private var previousSize = CGSize.zero
  override func layoutSubviews() {
    super.layoutSubviews()
    if previousSize != .zero && previousSize != bounds.size && hasInk { clear() }
    previousSize = bounds.size
  }
  var hasInk: Bool { !strokes.isEmpty }
  override init(frame: CGRect) {
    super.init(frame: frame)
    accessibilityIdentifier = "handwritingCanvas"
    accessibilityLabel = "手写区域"
    accessibilityHint = "用手指书写，停笔后选择上方候选文字"
    isMultipleTouchEnabled = false
    isOpaque = false
    // The rounded surface is drawn inside cardRect because this view now covers the full panel.
    clipsToBounds = true
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
  func clear() { strokes = []; drawing = false; setNeedsDisplay(); onChange?() }
  func undo() { if !strokes.isEmpty { strokes.removeLast() }; drawing = false; setNeedsDisplay(); onChange?() }
  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
    guard acceptsInk, strokes.count < 64, let touch = touches.first else { return }
    drawing = true
    strokes.append([touch.location(in: self)])
    onStrokeBegan?()
    setNeedsDisplay()
  }
  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
    guard drawing, let touch = touches.first, !strokes.isEmpty else { return }
    for point in (event?.coalescedTouches(for: touch) ?? [touch]).map({ $0.location(in: self) }) {
      guard strokes[strokes.count - 1].count < 512 else { break }
      let bounded = CGPoint(
        x: SharedNumber.clamped(point.x, to: 0...bounds.width),
        y: SharedNumber.clamped(point.y, to: 0...bounds.height))
      if let last = strokes.last?.last, hypot(last.x - bounded.x, last.y - bounded.y) < 1 { continue }
      strokes[strokes.count - 1].append(bounded)
    }
    setNeedsDisplay()
  }
  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
    guard drawing else { return }; touchesMoved(touches, with: event); drawing = false; onChange?()
  }
  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { if drawing { undo() } }
  private func drawInk(_ context: CGContext, color: UIColor, width: CGFloat) {
    context.setStrokeColor(color.cgColor); context.setFillColor(color.cgColor)
    context.setLineWidth(width); context.setLineCap(.round); context.setLineJoin(.round)
    for stroke in strokes {
      guard let first = stroke.first else { continue }
      if stroke.count == 1 { context.fillEllipse(in: CGRect(x: first.x - width / 2, y: first.y - width / 2, width: width, height: width)); continue }
      context.beginPath(); context.move(to: first)
      for point in stroke.dropFirst() { context.addLine(to: point) }
      context.strokePath()
    }
  }
  override func draw(_ rect: CGRect) {
    guard let context = UIGraphicsGetCurrentContext() else { return }
    let skin = KeyboardTheme.current
    let card = cardRect.isEmpty ? bounds : cardRect
    UIBezierPath(roundedRect: card, cornerRadius: 10).addClip()
    skin.keyBackground.setFill(); context.fill(card)
    context.setStrokeColor(skin.accent.withAlphaComponent(0.12).cgColor)
    context.setLineDash(phase: 0, lengths: [4, 4]); context.setLineWidth(1)
    context.move(to: CGPoint(x: card.midX, y: card.minY)); context.addLine(to: CGPoint(x: card.midX, y: card.maxY))
    context.move(to: CGPoint(x: card.minX, y: card.midY)); context.addLine(to: CGPoint(x: card.maxX, y: card.midY)); context.strokePath()
    context.setLineDash(phase: 0, lengths: [])
    context.resetClip()
    drawInk(context, color: skin.keyForeground, width: 3)
    // The status label is centred over the same card and carries the empty-state and live messages.
  }
  func setTestStrokes(_ values: [[CGPoint]]) { strokes = values; setNeedsDisplay(); onChange?() }
}

final class HandwritingInputView: UIView {
  let canvas = HandwritingCanvas()
  /// Transparent geometry guide for the visible writing card.
  private let cardGuide = UIView()
  /// Recognition results are rendered by the controller's shared candidate strip.
  var onResults: (([String]) -> Void)?
  private let status = UILabel()
  private var recognizerStorage: HandwritingRecognizer?
  private var recognizer: HandwritingRecognizer {
    if recognizerStorage == nil { recognizerStorage = HandwritingRecognizer() }
    return recognizerStorage!
  }
  private let modelButton = UIButton(type: .system)
  private var downloadGate = HandwritingDownloadGate()
  private var statusCentred: NSLayoutConstraint?
  private var statusBelowModelButton: NSLayoutConstraint?
  private var downloadTask: Task<Void, Never>?
  var canDownload: () -> Bool = { false }
  func activate() {
    downloadGate.setActive(true)
    let ready = recognizer.isReady
    modelButton.isEnabled = true
    canvas.acceptsInk = ready
    modelButton.isHidden = ready
    placeStatus()
    if !ready && downloadTask == nil {
      modelButton.setTitle("下载中文手写模型", for: .normal)
      showStatus(canDownload() ? "首次下载后可离线手写" : "首次下载需在系统设置允许完全访问")
    }
  }
  func deactivate() {
    downloadGate.setActive(false)
    downloadTask?.cancel()
    downloadTask = nil
    clear(); recognizerStorage?.release()
  }
  private func toggleDownload() {
    if let downloadTask {
      downloadTask.cancel(); modelButton.isEnabled = false
      showStatus("正在取消下载…"); return
    }
    guard canDownload() else { showStatus("请在系统键盘设置中允许完全访问，再点下载"); return }
    modelButton.setTitle("取消下载", for: .normal)
    let token = downloadGate.beginDownload()
    downloadTask = Task { [weak self] in
      guard let self else { return }
      do {
        try await self.recognizer.download { [weak self] fraction in
          guard let self, self.downloadGate.accepts(token) else { return }
          self.showStatus(fraction > 0 ? "模型下载中 \(Int(fraction * 100))%" : "正在连接模型服务…")
        }
        try Task.checkCancellation()
        guard self.downloadGate.accepts(token) else { return }
        self.downloadTask = nil; self.activate(); self.showStatus("在此手写，停笔后选字")
      } catch is CancellationError {
        guard self.downloadGate.accepts(token) else { return }
        self.downloadTask = nil; self.activate()
      } catch {
        guard self.downloadGate.accepts(token) else { return }
        self.downloadTask = nil; self.activate(); self.showStatus("下载失败，请检查网络后重试")
      }
    }
  }
  private var task: Task<Void, Never>?
  private var revision = UUID()
  private(set) var results: [String] = []
  var onInsert: ((String) -> Void)?
  var onDelete: (() -> Void)?
  var hasInk: Bool { canvas.hasInk }
  override init(frame: CGRect) {
    super.init(frame: frame)
    accessibilityIdentifier = "handwritingInput"
    // Recognition results belong to the shared candidate strip; keeping a private strip here
    // duplicated candidates and reduced the writing canvas.
    let column = UIStackView(); column.axis = .vertical; column.spacing = 4
    status.font = .systemFont(ofSize: 13); status.text = "在此手写，停笔后选字"
    status.accessibilityIdentifier = "handwritingStatus"
    status.textAlignment = .center
    status.numberOfLines = 2
    status.translatesAutoresizingMaskIntoConstraints = false
    let row = UIStackView(); row.spacing = KeyboardLayoutPreference.keySpacing
    cardGuide.isUserInteractionEnabled = false
    cardGuide.backgroundColor = .clear
    row.addArrangedSubview(cardGuide)
    cardGuide.addSubview(status)
    NSLayoutConstraint.activate([
      status.centerXAnchor.constraint(equalTo: cardGuide.centerXAnchor),
      status.leadingAnchor.constraint(greaterThanOrEqualTo: cardGuide.leadingAnchor, constant: 12),
      status.trailingAnchor.constraint(lessThanOrEqualTo: cardGuide.trailingAnchor, constant: -12),
    ])
    let tools = UIStackView(); tools.axis = .vertical; tools.spacing = 4; tools.distribution = .fillEqually
    for (title, id, action) in [
      ("撤销", "handwritingUndo", { [weak self] in self?.canvas.undo() }),
      ("清空", "handwritingClear", { [weak self] in self?.clear() }),
      ("⌫", "handwritingDelete", { [weak self] in if self?.hasInk == true { self?.canvas.undo() } else { self?.onDelete?() } }),
    ] {
      let button = KeyboardKeyButton(type: .system); button.setTitle(title, for: .normal)
      button.accessibilityIdentifier = id; button.accessibilityLabel = title == "⌫" ? "删除" : title
      button.addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
      tools.addArrangedSubview(button)
    }
    tools.widthAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
    row.addArrangedSubview(tools); column.addArrangedSubview(row)
    addSubview(column); column.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([column.leadingAnchor.constraint(equalTo: leadingAnchor), column.trailingAnchor.constraint(equalTo: trailingAnchor), column.topAnchor.constraint(equalTo: topAnchor), column.bottomAnchor.constraint(equalTo: bottomAnchor)])
    insertSubview(canvas, at: 0)
    canvas.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      canvas.leadingAnchor.constraint(equalTo: leadingAnchor),
      canvas.trailingAnchor.constraint(equalTo: trailingAnchor),
      canvas.topAnchor.constraint(equalTo: topAnchor),
      canvas.bottomAnchor.constraint(equalTo: bottomAnchor),
    ])
    tools.widthAnchor.constraint(lessThanOrEqualToConstant: 72).isActive = true
    let share = tools.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.17)
    share.priority = .defaultHigh
    share.isActive = true
    cardGuide.addSubview(modelButton)
    modelButton.translatesAutoresizingMaskIntoConstraints = false
    modelButton.backgroundColor = .secondarySystemBackground
    modelButton.layer.cornerRadius = 12
    modelButton.titleLabel?.font = .systemFont(ofSize: 15, weight: .medium)
    modelButton.accessibilityIdentifier = "handwritingDownloadModel"
    modelButton.accessibilityHint = "联网下载 Google 中文手写模型，完成后可离线识别"
    modelButton.isHidden = true
    modelButton.addAction(UIAction { [weak self] _ in self?.toggleDownload() }, for: .primaryActionTriggered)
    NSLayoutConstraint.activate([
      modelButton.centerXAnchor.constraint(equalTo: cardGuide.centerXAnchor),
      modelButton.centerYAnchor.constraint(equalTo: cardGuide.centerYAnchor),
      modelButton.widthAnchor.constraint(equalTo: cardGuide.widthAnchor, multiplier: 0.85),
      modelButton.heightAnchor.constraint(equalToConstant: 44),
    ])
    canvas.onStrokeBegan = { [weak self] in self?.invalidate(); self?.showStatus("书写中…") }
    canvas.onChange = { [weak self] in self?.scheduleRecognition() }
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
  private func invalidate() {
    revision = UUID(); task?.cancel(); task = nil; results = []
    onResults?([])
  }
  func clear() { invalidate(); canvas.clear() }
  override func layoutSubviews() {
    super.layoutSubviews()
    canvas.cardRect = cardGuide.convert(cardGuide.bounds, to: canvas)
  }
  override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
    guard let hit = super.hitTest(point, with: event) else { return nil }
    return hit is UIControl ? hit : canvas
  }
  private func placeStatus() {
    guard status.superview != nil else { return }
    if statusCentred == nil {
      statusCentred = status.centerYAnchor.constraint(equalTo: cardGuide.centerYAnchor)
    }
    if statusBelowModelButton == nil, modelButton.superview != nil {
      statusBelowModelButton = status.topAnchor.constraint(equalTo: modelButton.bottomAnchor, constant: 10)
    }
    let sharesTheCanvas = !modelButton.isHidden && statusBelowModelButton != nil
    statusBelowModelButton?.isActive = sharesTheCanvas
    statusCentred?.isActive = !sharesTheCanvas
  }
  private func showStatus(_ text: String) {
    status.text = text; status.textColor = KeyboardTheme.current.keyForeground
    status.isHidden = text.isEmpty
    placeStatus()
  }
  private func scheduleRecognition() {
    invalidate()
    guard hasInk else { showStatus("在此手写，停笔后选字"); if !modelButton.isHidden { activate() }; return }
    showStatus("停笔后识别…")
    let current = revision
    task = Task { [weak self] in
      do {
        try await Task.sleep(nanoseconds: 550_000_000)
        guard let self, self.revision == current, self.canvas.hasInk else { return }
        self.showStatus("正在识别…")
        let words = try await self.recognizer.recognize(self.canvas.strokes, width: self.canvas.bounds.width, height: self.canvas.bounds.height)
        guard !Task.isCancelled, self.revision == current else { return }
        self.results = words
        self.showStatus(words.isEmpty ? "未识别，请撤销或重新书写" : "")
        self.onResults?(words)
      } catch is CancellationError { } catch {
        guard let self, self.revision == current else { return }
        self.showStatus("识别失败，请重写后重试")
      }
    }
  }
  @discardableResult func use(at index: Int) -> Bool {
    guard results.indices.contains(index) else { return false }
    let word = results[index]
    clear(); onInsert?(word); return true
  }
  @discardableResult func commitFirst() -> Bool {
    guard let word = results.first else { return false }; clear(); onInsert?(word); return true
  }
}
