import SwiftUI

extension View {
  /// 底部弹窗的高度贴合内容，不再默认占满整屏、下面空一大片。加在弹窗里的 `Form`、`List` 或 `ScrollView` 上（不是它外面的导航容器）：iOS 18 起读出滚动内容的高度，弹窗停在这个高度，往上拉可以到整屏；内容比屏幕还高时系统把它限在整屏。键盘弹起时弹窗贴着键盘上沿、保持贴合的高度，不被系统撑到整屏。iOS 17 读不到滚动内容的尺寸，退回半屏起。`enabled` 为假时什么也不做，供同一个页面既被推入又被弹出的情况。macOS 没有底部弹窗，什么也不做。
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
  @State private var height: CGFloat?
  @State private var selection: PresentationDetent = .medium
  @State private var keyboardShown = false

  func body(content: Content) -> some View {
    if #available(iOS 18, *) {
      content
        // `.height` 档位不含底部安全区，系统在它下面另加主屏指示条那一截，所以只计内容和上方的导航栏，不计底部留白；底部留白在键盘弹起时还会加上键盘高度。
        .onScrollGeometryChange(for: CGFloat.self) { geometry in
          geometry.contentSize.height + geometry.contentInsets.top
        } action: { _, measured in
          fit(ceil(measured))
        }
        .presentationDetents(detents, selection: $selection)
        .presentationDragIndicator(.visible)
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in keyboardShown = true }
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in keyboardShown = false }
    } else {
      content
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
    }
  }

  /// 键盘弹起时，档位里只要还有整屏，系统就把停在贴合高度的弹窗撑到整屏，输入框和键盘之间空出一大块（真机实测，添加常用语一打开就聚焦输入框，整屏弹出）。所以键盘在、用户又没拉到整屏时只给贴合的那一档，弹窗贴着键盘上沿；键盘收起再把整屏放回来。
  private var detents: Set<PresentationDetent> {
    guard let height else { return [.medium, .large] }
    if keyboardShown && selection != .large { return [.height(height)] }
    return [.height(height), .large]
  }

  /// 量到新高度时，没被用户拉到整屏的弹窗跟着换到新高度；差不到 1pt 的不动，免得来回跳。
  private func fit(_ measured: CGFloat) {
    guard measured > 0, abs((height ?? 0) - measured) >= 1 else { return }
    height = measured
    if selection != .large { selection = .height(measured) }
  }
}
#endif
