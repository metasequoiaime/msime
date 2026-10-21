import SwiftUI

extension View {
  /// 底部弹窗的高度贴合内容，不再默认占满整屏、下面空一大片。加在弹窗里的 `Form`、`List` 或 `ScrollView` 上（不是它外面的导航容器）：iOS 18 起读出滚动内容的高度，弹窗停在这个高度，往上拉可以到整屏；内容比屏幕还高时系统把它限在整屏。iOS 17 读不到滚动内容的尺寸，退回半屏起。`enabled` 为假时什么也不做，供同一个页面既被推入又被弹出的情况。macOS 没有底部弹窗，什么也不做。
  @ViewBuilder func fittedSheet(_ enabled: Bool = true) -> some View {
    #if os(iOS)
    if enabled { modifier(FittedSheetModifier()) } else { self }
    #else
    self
    #endif
  }
}

#if os(iOS)
private struct FittedSheetModifier: ViewModifier {
  /// 底部最多计这么多：Face ID 机型的主屏指示条区域。键盘弹起时滚动区的底部留白会加上键盘高度，按它算弹窗会跟着键盘变高。
  private static let bottomAllowance: CGFloat = 34

  @State private var height: CGFloat?
  @State private var selection: PresentationDetent = .medium

  func body(content: Content) -> some View {
    if #available(iOS 18, *) {
      content
        .onScrollGeometryChange(for: CGFloat.self) { geometry in
          geometry.contentSize.height + geometry.contentInsets.top + min(geometry.contentInsets.bottom, Self.bottomAllowance)
        } action: { _, measured in
          fit(ceil(measured))
        }
        .presentationDetents(detents, selection: $selection)
        .presentationDragIndicator(.visible)
    } else {
      content
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
    }
  }

  private var detents: Set<PresentationDetent> {
    height.map { [.height($0), .large] } ?? [.medium, .large]
  }

  /// 量到新高度时，没被用户拉到整屏的弹窗跟着换到新高度；差不到 1pt 的不动，免得来回跳。
  private func fit(_ measured: CGFloat) {
    guard measured > 0, abs((height ?? 0) - measured) >= 1 else { return }
    height = measured
    if selection != .large { selection = .height(measured) }
  }
}
#endif
