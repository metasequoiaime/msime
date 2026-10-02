import AppKit
import SwiftUI

struct MacEmojiFlowCell {
  let rect: CGRect
  let row: Int
  let fontSize: CGFloat
}

/// Windows 04a8df56 flow metrics, using the native panel's 2/3 coordinate scale.
enum MacEmojiFlow {
  static let scale: CGFloat = 2 / 3
  static let height: CGFloat = 84 * scale
  static let gap: CGFloat = 6 * scale
  static let rowGap: CGFloat = 4 * scale
  static let padding: CGFloat = 12 * scale
  static let fontSize: CGFloat = 18 * scale
  static let minimumFontSize: CGFloat = 9 * scale

  static func measure(_ text: String, _ size: CGFloat) -> CGSize {
    (text as NSString).size(withAttributes: [.font: NSFont.systemFont(ofSize: size)])
  }

  static func fittedSize(_ text: String, width: CGFloat, height: CGFloat,
    measure: (String, CGFloat) -> CGSize) -> CGFloat {
    var low = minimumFontSize
    var high = fontSize
    while high - low > 0.5 * scale {
      let mid = (low + high) / 2
      let candidate = measure(text, mid)
      if candidate.width <= width && candidate.height <= height { low = mid }
      else { high = mid }
    }
    return low
  }

  static func cells(texts: [String], width: CGFloat,
    measure: (String, CGFloat) -> CGSize = measure) -> [MacEmojiFlowCell] {
    guard width.isFinite, width > 0 else { return [] }
    var x: CGFloat = 0
    var row = 0
    return texts.map { text in
      let innerWidth = max(0, width - padding * 2)
      let natural = measure(text, fontSize)
      var measured = natural
      if measured.width > innerWidth {
        let size = fittedSize(text, width: innerWidth, height: height - 16 * scale, measure: measure)
        measured = measure(text, size)
      }
      let cellWidth = min(width, max(52 * scale, measured.width + padding * 2 + 6 * scale))
      let paintSize: CGFloat
      if text.utf16.count <= 4 { paintSize = 42 * scale }
      else if natural.width > cellWidth || natural.height > height {
        paintSize = fittedSize(text, width: cellWidth, height: height, measure: measure)
      } else { paintSize = fontSize }
      if x > 0 && x + cellWidth > width + 0.5 * scale { x = 0; row += 1 }
      let result = MacEmojiFlowCell(rect: CGRect(x: x, y: CGFloat(row) * (height + rowGap),
        width: cellWidth, height: height), row: row, fontSize: paintSize)
      x += cellWidth + gap
      return result
    }
  }

  /// 把分节各自排好的流式单元按节顺序接成一列，行号依次续接，供跨节的上下方向键使用；矩形仍是各节内的坐标。
  static func stacked(_ sections: [[MacEmojiFlowCell]]) -> [MacEmojiFlowCell] {
    var base = 0
    return sections.flatMap { cells -> [MacEmojiFlowCell] in
      let shifted = cells.map { MacEmojiFlowCell(rect: $0.rect, row: base + $0.row, fontSize: $0.fontSize) }
      base += (cells.map(\.row).max() ?? -1) + 1
      return shifted
    }
  }

  static func vertical(from index: Int, direction: Int, cells: [MacEmojiFlowCell]) -> Int? {
    guard cells.indices.contains(index) else { return nil }
    let current = cells[index]
    return cells.indices.filter { cells[$0].row == current.row + direction }.min {
      abs(cells[$0].rect.midX - current.rect.midX) < abs(cells[$1].rect.midX - current.rect.midX)
    } ?? index
  }
}

/// Place real layout frames, so ScrollViewReader and accessibility use the same geometry.
struct MacEmojiFlowPlacement: Layout {
  let cells: [MacEmojiFlowCell]
  let width: CGFloat

  func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
    CGSize(width: max(0, width), height: cells.last?.rect.maxY ?? 0)
  }

  func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
    for (index, subview) in subviews.enumerated() where cells.indices.contains(index) {
      let rect = cells[index].rect
      subview.place(at: CGPoint(x: bounds.minX + rect.minX, y: bounds.minY + rect.minY),
        anchor: .topLeading, proposal: ProposedViewSize(rect.size))
    }
  }
}

struct MacEmojiFlowGrid<ID: Hashable>: View {
  let items: [MacEmojiCatalogItem]
  let cells: [MacEmojiFlowCell]
  let width: CGFloat
  let palette: MacEmojiPalette
  let selected: (Int) -> Bool
  let identity: (Int) -> ID
  let copy: (Int) -> Void

  var body: some View {
    MacEmojiFlowPlacement(cells: cells, width: width) {
      ForEach(Array(items.enumerated()), id: \.offset) { index, item in
        if cells.indices.contains(index) {
          let cell = cells[index]
          Button { copy(index) } label: {
            Text(item.text).font(.system(size: cell.fontSize)).lineLimit(1)
              .frame(width: cell.rect.width, height: cell.rect.height).clipped()
          }
          .buttonStyle(MacEmojiCellStyle(palette: palette, selected: selected(index)))
          .frame(width: cell.rect.width, height: cell.rect.height)
          .id(identity(index))
          .help([item.group, item.annotation].filter { !$0.isEmpty }.joined(separator: " · "))
          .accessibilityLabel(item.annotation.isEmpty ? item.text : item.annotation)
        }
      }
    }
  }
}
