import SwiftUI

/// 二级设置页共用的行与状态条。
///
/// 放在 shared/backend-ui 而不是 platforms/ios/App/Sources:这个目录里的云词库、词包等页面和应用内其余二级页是同一批,读者不会知道它们的源码分属两处,却看得出它们长得不一样。
///
/// 这个目录两端都编:iOS 走 project.yml,macOS 的账号窗口走 cmake/BackendAccount.cmake 里那张显式文件表 —— 往这里加文件必须同时加进那张表,否则 macOS 侧会在编译期报找不到符号。所以这里不碰任何只有一端才有的东西。
///
/// 这些页面原先各写各的:动作是一排裸文字按钮,说明文字挤在动作同一组里当成一个列表行,加载和报错也当列表行插在条目中间。于是同一个标签页点进去,每一页都是另一套写法,而加载一结束列表还会跳一下。
struct SettingsRowLabel: View {
  let title: String
  var detail: String?
  let symbol: String
  var destructive = false

  /// 图标块只有两种颜色:强调色,和表示这一行会删东西的红。
  ///
  /// 原先每个调用点自带一个颜色,于是同一页十几行就是十几种,粉的青的靛的棕的各来一个 —— 颜色本该说明一行的性质,这里只是让每行长得不一样,读者得先把颜色全部忽略掉才能看内容。参数一旦存在就会被填,所以这里把它删掉,而不是改成默认值。
  private var tint: Color { destructive ? .red : .accentColor }

  var body: some View {
    HStack(spacing: 12) {
      Image(systemName: symbol).font(.system(size: 15, weight: .semibold))
        .foregroundStyle(tint)
        .frame(width: 30, height: 30)
        .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 9))
      VStack(alignment: .leading, spacing: 2) {
        Text(title).foregroundStyle(destructive ? Color.red : Color.primary)
        if let detail {
          Text(detail).font(.caption).foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
      Spacer(minLength: 0)
    }
    .padding(.vertical, 2)
    .contentShape(Rectangle())
  }
}

/// 点一下就做一件事的行。用 `.plain` 是因为列表里的按钮默认整行染成强调色,那样图标的颜色就白给了。
struct SettingsActionRow: View {
  let title: String
  var detail: String?
  let symbol: String
  var destructive = false
  var enabled = true
  let action: () -> Void

  var body: some View {
    Button(action: action) {
      SettingsRowLabel(title: title, detail: detail, symbol: symbol, destructive: destructive)
    }
    .buttonStyle(.plain)
    .disabled(!enabled)
    .opacity(enabled ? 1 : 0.45)
  }
}

/// 分区标题旁的「?」：解释性说明不常驻页面，点「?」在气泡里显示，点别处收起。规则与 iOS 的 `DesignInfoTip`（`platforms/ios/App/Sources/app/DesignKit.swift`）相同：解释进「?」，状态、校验、错误、当前值和紧贴按钮讲后果的一句话留在脚注。这个目录两端都编，不能引用只有 iOS 才有的 DesignKit，所以单独写一份；读屏把说明当作「?」的名称读出。
struct SettingsInfoTip: View {
  let text: String
  @State private var presented = false

  var body: some View {
    Button { presented = true } label: {
      Image(systemName: "questionmark.circle")
        .font(.system(size: 15))
        .foregroundStyle(.secondary)
        .frame(width: 26, height: 26)
        .contentShape(Rectangle())
    }
    .buttonStyle(.borderless)
    .accessibilityLabel(text)
    .accessibilityIdentifier("designInfoTip")
    .popover(isPresented: $presented) {
      bubble
    }
  }

  @ViewBuilder private var bubble: some View {
    let content = Text(text)
      .font(.system(size: 14))
      .multilineTextAlignment(.leading)
      .frame(maxWidth: 280, alignment: .leading)
      .fixedSize(horizontal: false, vertical: true)
      .padding(.horizontal, 14)
      .padding(.vertical, 12)
    // 手机上 popover 默认会变成整页弹窗；固定成气泡。macOS 13.0 还没有这个修饰符，那里 popover 本来就是气泡。
    if #available(iOS 16.4, macOS 13.3, *) {
      content.presentationCompactAdaptation(.popover)
    } else {
      content
    }
  }
}

/// 分区标题加「?」：标题照旧由调用方画，`info` 为空时只有标题；没有标题的分区只画「?」。
struct SettingsSectionInfoHeader<Title: View>: View {
  let info: String?
  private let title: Title

  init(info: String?, @ViewBuilder title: () -> Title) {
    self.info = info
    self.title = title()
  }

  var body: some View {
    HStack(spacing: 4) {
      title
      if let info { SettingsInfoTip(text: info).textCase(nil) }
    }
  }
}

extension SettingsSectionInfoHeader where Title == EmptyView {
  init(info: String?) {
    self.init(info: info) { EmptyView() }
  }
}

/// 只读的一行事实 —— 云端版本、条目数这类。跟动作行同一套排版,免得同一页里「能点的」和「不能点的」长得像两个控件族。
struct SettingsFactRow: View {
  let title: String
  var detail: String?
  let symbol: String

  var body: some View {
    SettingsRowLabel(title: title, detail: detail, symbol: symbol)
  }
}

/// 加载与报错固定在底部,不参与列表布局。作为列表行时,它们出现和消失都会把下面的条目推上推下,而这恰好发生在用户正要去点某一行的时候。
struct SettingsStatusBar: View {
  let busy: Bool
  var busyTitle = "正在处理…"
  let message: String?

  var body: some View {
    if busy || message != nil {
      HStack(spacing: 10) {
        if busy {
          ProgressView()
        } else {
          Image(systemName: "exclamationmark.circle.fill").foregroundStyle(.orange)
        }
        Text(busy ? busyTitle : (message ?? ""))
          .font(.footnote)
          .foregroundStyle(busy ? Color.secondary : Color.primary)
          .fixedSize(horizontal: false, vertical: true)
        Spacer(minLength: 0)
      }
      .padding(.horizontal, 20).padding(.vertical, 12)
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(.regularMaterial)
      .overlay(alignment: .top) { Divider() }
      .transition(.move(edge: .bottom).combined(with: .opacity))
    }
  }
}

extension View {
  func settingsStatus(busy: Bool, busyTitle: String = "正在处理…", message: String?) -> some View {
    safeAreaInset(edge: .bottom, spacing: 0) {
      SettingsStatusBar(busy: busy, busyTitle: busyTitle, message: message)
        .animation(.easeInOut(duration: 0.18), value: busy)
        .animation(.easeInOut(duration: 0.18), value: message)
    }
  }
}

extension View {
  /// macOS 没有导航栏标题显示模式这回事,这一条只对 iOS 有意义。
  func inlineNavigationTitle() -> some View {
    #if os(iOS)
    return navigationBarTitleDisplayMode(.inline)
    #else
    return self
    #endif
  }
}
