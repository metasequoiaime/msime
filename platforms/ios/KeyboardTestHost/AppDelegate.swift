import UIKit

// A separate host lets keyboard integration tests run without loading the production app.
// The current iOS SDK requires a scene-based lifecycle before XCTest can attach.
@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
  func application(_ application: UIApplication,
                   configurationForConnecting session: UISceneSession,
                   options: UIScene.ConnectionOptions) -> UISceneConfiguration {
    let configuration = UISceneConfiguration(name: nil, sessionRole: session.role)
    configuration.delegateClass = SceneDelegate.self
    return configuration
  }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
  var window: UIWindow?

  func scene(_ scene: UIScene, willConnectTo session: UISceneSession,
             options: UIScene.ConnectionOptions) {
    guard let windowScene = scene as? UIWindowScene else { return }
    let window = UIWindow(windowScene: windowScene)
    let arguments = ProcessInfo.processInfo.arguments
    window.rootViewController = arguments.contains("--system-keyboard-editor") ? SystemKeyboardEditorViewController()
      : arguments.contains("--selection-only-insert-editor") ? SelectionOnlyInsertEditorViewController()
      : UIViewController()
    window.makeKeyAndVisible()
    self.window = window
  }
}

/// 独立编辑器只使用系统键盘，不在进程内构造产品的键盘控制器。
private final class SystemKeyboardEditorViewController: UIViewController {
  private let field = UITextField()

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .systemBackground
    field.accessibilityIdentifier = "systemKeyboardEditor"
    field.placeholder = "输入合成测试文字"
    field.borderStyle = .roundedRect
    field.autocapitalizationType = .none
    field.autocorrectionType = .no
    field.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(field)
    NSLayoutConstraint.activate([
      field.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 24),
      field.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 20),
      field.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -20),
      field.heightAnchor.constraint(equalToConstant: 48),
    ])
  }

  override func viewDidAppear(_ animated: Bool) {
    super.viewDidAppear(animated)
    field.becomeFirstResponder()
  }
}

/// 按自绘输入框（Flutter 的 `FlutterTextInputView` 一类）的规则编辑的文档，用来在真实的跨进程键盘代理后面核对上屏（#10873）：`insertText` 只替换当前选区并清掉标记，不替换标记文本；`setMarkedText` 替换标记文本，没有标记时替换选区；`unmarkText` 只把标记变成普通文字。系统 `UITextField` 的 `insertText` 会顺带替换标记文本，测不出这类宿主的问题。
private final class SelectionOnlyInsertEditor: UIView, UITextInput {
  private final class Position: UITextPosition {
    let offset: Int
    init(_ offset: Int) { self.offset = offset }
  }

  private final class Range: UITextRange {
    let range: NSRange
    init(_ range: NSRange) { self.range = range }
    override var start: UITextPosition { Position(range.location) }
    override var end: UITextPosition { Position(NSMaxRange(range)) }
    override var isEmpty: Bool { range.length == 0 }
  }

  private var text = "" as NSString
  private var selection = NSRange(location: 0, length: 0)
  private var marked: NSRange?
  private let label = UILabel()

  weak var inputDelegate: UITextInputDelegate?
  lazy var tokenizer: UITextInputTokenizer = UITextInputStringTokenizer(textInput: self)
  var markedTextStyle: [NSAttributedString.Key: Any]?
  var autocorrectionType: UITextAutocorrectionType = .no
  var autocapitalizationType: UITextAutocapitalizationType = .none
  var spellCheckingType: UITextSpellCheckingType = .no

  override init(frame: CGRect) {
    super.init(frame: frame)
    layer.borderWidth = 1
    layer.borderColor = UIColor.separator.cgColor
    label.translatesAutoresizingMaskIntoConstraints = false
    addSubview(label)
    NSLayoutConstraint.activate([
      label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
      label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
      label.centerYAnchor.constraint(equalTo: centerYAnchor),
    ])
    isAccessibilityElement = true
    accessibilityIdentifier = "selectionOnlyInsertEditor"
    accessibilityTraits = .staticText
    accessibilityValue = ""
    addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(focus)))
  }

  required init?(coder: NSCoder) { nil }

  override var canBecomeFirstResponder: Bool { true }

  @objc private func focus() { becomeFirstResponder() }

  private func clamp(_ range: NSRange) -> NSRange {
    let location = min(max(range.location, 0), text.length)
    return NSRange(location: location, length: min(max(range.length, 0), text.length - location))
  }

  private func offset(_ position: UITextPosition) -> Int { (position as? Position)?.offset ?? 0 }

  private func replaceCharacters(in range: NSRange, with string: String) {
    text = text.replacingCharacters(in: clamp(range), with: string) as NSString
  }

  private func changed() {
    label.text = text as String
    accessibilityValue = text as String
  }

  var hasText: Bool { text.length > 0 }

  func insertText(_ string: String) {
    let range = clamp(selection)
    replaceCharacters(in: range, with: string)
    selection = NSRange(location: range.location + (string as NSString).length, length: 0)
    marked = nil
    changed()
  }

  func deleteBackward() {
    marked = nil
    let range = clamp(selection)
    if range.length > 0 {
      replaceCharacters(in: range, with: "")
      selection = NSRange(location: range.location, length: 0)
    } else if range.location > 0 {
      let removed = text.rangeOfComposedCharacterSequence(at: range.location - 1)
      replaceCharacters(in: removed, with: "")
      selection = NSRange(location: removed.location, length: 0)
    }
    changed()
  }

  func text(in range: UITextRange) -> String? {
    (range as? Range).map { text.substring(with: clamp($0.range)) }
  }

  func replace(_ range: UITextRange, withText string: String) {
    guard let range = (range as? Range).map({ clamp($0.range) }) else { return }
    replaceCharacters(in: range, with: string)
    selection = NSRange(location: range.location + (string as NSString).length, length: 0)
    changed()
  }

  var selectedTextRange: UITextRange? {
    get { Range(selection) }
    set { if let range = newValue as? Range { selection = clamp(range.range) } }
  }

  var markedTextRange: UITextRange? { marked.map { Range($0) } }

  func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
    let string = markedText ?? ""
    let range = clamp(marked ?? selection)
    replaceCharacters(in: range, with: string)
    let length = (string as NSString).length
    marked = length == 0 ? nil : NSRange(location: range.location, length: length)
    selection = clamp(NSRange(location: range.location + selectedRange.location, length: selectedRange.length))
    changed()
  }

  func unmarkText() {
    marked = nil
    changed()
  }

  var beginningOfDocument: UITextPosition { Position(0) }
  var endOfDocument: UITextPosition { Position(text.length) }

  func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
    let from = offset(fromPosition), to = offset(toPosition)
    return Range(NSRange(location: min(from, to), length: abs(to - from)))
  }

  func position(from position: UITextPosition, offset delta: Int) -> UITextPosition? {
    let target = offset(position) + delta
    return (0...text.length).contains(target) ? Position(target) : nil
  }

  func position(from position: UITextPosition, in direction: UITextLayoutDirection, offset delta: Int) -> UITextPosition? {
    switch direction {
    case .left, .up: return self.position(from: position, offset: -delta)
    default: return self.position(from: position, offset: delta)
    }
  }

  func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
    let lhs = offset(position), rhs = offset(other)
    return lhs < rhs ? .orderedAscending : lhs > rhs ? .orderedDescending : .orderedSame
  }

  func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int { offset(toPosition) - offset(from) }

  func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
    switch direction {
    case .left, .up: return range.start
    default: return range.end
    }
  }

  func characterRange(byExtending position: UITextPosition, in direction: UITextLayoutDirection) -> UITextRange? {
    switch direction {
    case .left, .up: return textRange(from: beginningOfDocument, to: position)
    default: return textRange(from: position, to: endOfDocument)
    }
  }

  func baseWritingDirection(for position: UITextPosition, in direction: UITextStorageDirection) -> NSWritingDirection {
    .leftToRight
  }

  func setBaseWritingDirection(_ writingDirection: NSWritingDirection, for range: UITextRange) {}

  func firstRect(for range: UITextRange) -> CGRect { bounds }

  func caretRect(for position: UITextPosition) -> CGRect {
    CGRect(x: max(bounds.maxX - 10, 0), y: 4, width: 2, height: max(bounds.height - 8, 0))
  }

  func selectionRects(for range: UITextRange) -> [UITextSelectionRect] { [] }

  func closestPosition(to point: CGPoint) -> UITextPosition? { endOfDocument }

  func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? { range.end }

  func characterRange(at point: CGPoint) -> UITextRange? { nil }
}

private final class SelectionOnlyInsertEditorViewController: UIViewController {
  private let editor = SelectionOnlyInsertEditor()

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .systemBackground
    editor.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(editor)
    NSLayoutConstraint.activate([
      editor.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 24),
      editor.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 20),
      editor.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -20),
      editor.heightAnchor.constraint(equalToConstant: 48),
    ])
  }

  override func viewDidAppear(_ animated: Bool) {
    super.viewDidAppear(animated)
    editor.becomeFirstResponder()
  }
}
