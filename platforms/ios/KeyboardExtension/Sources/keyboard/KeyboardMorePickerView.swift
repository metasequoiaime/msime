import UIKit

/// 键盘的功能面板（更多工具），从工具栏的品牌键打开：工具排成横向分页的网格，下面带页码圆点，手机每页 4 × 2，iPad 键盘每页 6 × 2。
///
/// 面板没有标题栏，也没有「返回」按钮；上方的工具栏一直可见，由它关闭面板。每次切换开关或换皮肤后都会调用 `update(tools:)`，它就地重新设置已有格子的样式（列表变了时挪动格子），让面板停在用户所在的那一页：在第 2 页切换「按键音」不会跳回第 1 页。
final class KeyboardMorePickerView: UIView {
  private static let topMargin: CGFloat = 10
  private static let bottomMargin: CGFloat = 4
  private static let rowGap: CGFloat = 16
  private static let columnGap: CGFloat = 4

  private let grid: KeyboardPagedGridView
  private var tiles: [KeyboardFunctionTileView] = []

  init(tools: [KeyboardTool], formFactor: KeyboardFormFactor) {
    let tablet = formFactor == .tablet
    grid = KeyboardPagedGridView(
      columns: tablet ? 6 : 4, rows: 2, rowHeight: KeyboardFunctionTileView.height,
      rowGap: Self.rowGap, columnGap: Self.columnGap, sidePadding: tablet ? 48 : 4)
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardMorePicker"
    accessibilityLabel = "更多工具"
    // 面板打开时上方的工具栏仍然可用，所以 VoiceOver 必须还能读到它。
    accessibilityViewIsModal = false
    grid.translatesAutoresizingMaskIntoConstraints = false
    addSubview(grid)
    NSLayoutConstraint.activate([
      grid.topAnchor.constraint(equalTo: topAnchor, constant: Self.topMargin),
      grid.leadingAnchor.constraint(equalTo: leadingAnchor),
      grid.trailingAnchor.constraint(equalTo: trailingAnchor),
      grid.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -Self.bottomMargin),
    ])
    update(tools: tools)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 当前显示的页，从 0 开始。
  var currentPage: Int { grid.currentPage }

  var pageCount: Int { grid.pageCount }

  /// 用当前皮肤按 `tools` 重画面板，保持所在页不变。格子按 `KeyboardTool.id` 对应：工具还在的格子就地重设样式，新工具建新格子。
  func update(tools: [KeyboardTool]) {
    let skin = KeyboardTheme.current
    // 面板盖住按键，所以用皮肤的背景；使用原生设计令牌的键盘保持透明，让按键背后的系统背景在这里也透出来。
    backgroundColor = skin.drawsNativeBackground ? .clear : skin.background
    grid.apply(skin: skin)
    if tools.map(\.id) == tiles.map(\.tool.id) {
      for (tile, tool) in zip(tiles, tools) { tile.configure(tool, skin: skin) }
      return
    }
    var reusable = Dictionary(tiles.map { ($0.tool.id, $0) }, uniquingKeysWith: { first, _ in first })
    tiles = tools.map { tool in
      guard let tile = reusable.removeValue(forKey: tool.id) else { return KeyboardFunctionTileView(tool: tool, skin: skin) }
      tile.configure(tool, skin: skin)
      return tile
    }
    grid.setTiles(tiles, preservingPage: true)
  }

  /// 不带动画回到第一页，供面板再次打开时使用。
  func resetToFirstPage() {
    grid.scrollToPage(0, animated: false)
  }
}
