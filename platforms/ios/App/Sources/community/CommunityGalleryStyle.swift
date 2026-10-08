import SwiftUI

struct CommunitySearchField: View {
  @Binding var text: String
  let placeholder: String
  let submit: () -> Void
  var body: some View {
    HStack(spacing: 9) {
      Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
      TextField(placeholder, text: $text).font(.subheadline).submitLabel(.search)
        .onSubmit(submit).autocorrectionDisabled()
      if !text.isEmpty {
        Button { text = ""; submit() } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary) }
          .accessibilityLabel("清除搜索")
      }
    }.padding(.horizontal, 13).frame(height: 44)
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: 14))
  }
}
struct CommunityAuthorLabel: View {
  let name: String
  var body: some View {
    HStack(spacing: 5) {
      Text(String(name.prefix(1))).font(.system(size: 9, weight: .semibold))
        .foregroundStyle(MetasequoiaTheme.accent).frame(width: 19, height: 19)
        .background(MetasequoiaTheme.accentSoft, in: Circle())
      Text(name).font(.caption).foregroundStyle(.secondary).lineLimit(1)
    }
  }
}
struct CommunityResourceCard: View {
  let item: CommunityResource
  private var accent: Color { MetasequoiaTheme.accent }
  var body: some View {
    VStack(alignment: .leading, spacing: 9) {
      cover
      Text(item.name).font(.system(size: 15, weight: .semibold)).lineLimit(1)
      HStack(spacing: 4) {
        CommunityAuthorLabel(name: item.author)
        if item.removed { CommunityRemovedBadge() }
      }
      HStack(spacing: 3) {
        Label("\(item.saves)", systemImage: item.saved ? "bookmark.fill" : "bookmark")
        Spacer(minLength: 2)
        Label(item.rating_count == 0 ? "暂无评分" : String(format: "%.1f", item.rating_average), systemImage: "star")
      }.font(.system(size: 10)).foregroundStyle(.secondary)
    }.padding(10).background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous))
  }
  private var cover: some View {
    VStack(alignment: .leading, spacing: 7) {
      HStack {
        Image(systemName: item.kind.icon).font(.system(size: 13, weight: .semibold))
        Spacer()
        Text(item.kind == .dictionary ? "\(item.content.entries?.count ?? 0) 词条" : "回复模板")
          .font(.system(size: 9, weight: .medium))
      }.foregroundStyle(accent)
      if item.kind == .dictionary {
        VStack(alignment: .leading, spacing: 4) {
          ForEach(Array((item.content.entries ?? []).prefix(3).enumerated()), id: \.offset) { _, word in
            Text(word.word).font(.system(size: 12, weight: .medium)).lineLimit(1)
              .padding(.horizontal, 7).padding(.vertical, 3)
              .background(MetasequoiaTheme.surface.opacity(0.8), in: RoundedRectangle(cornerRadius: 6))
          }
        }
      } else {
        Text(item.content.prompt ?? item.description).font(.system(size: 12, weight: .medium))
          .lineSpacing(3).lineLimit(4).frame(maxWidth: .infinity, alignment: .leading)
      }
      Spacer(minLength: 0)
    }.padding(11).frame(maxWidth: .infinity, alignment: .leading).frame(height: 124)
      .background(LinearGradient(colors: [accent.opacity(0.14), accent.opacity(0.04)], startPoint: .topLeading, endPoint: .bottomTrailing))
      .clipShape(RoundedRectangle(cornerRadius: 12))
  }
}

/// 社区皮肤卡片（获取 / 使用 / 使用中）和资源行（添加 / 已添加）上的色调胶囊：可执行操作时用 accentSoft 和 accent，无事可做后用 segBg 和 sub。
struct CommunityPillLabel: View {
  enum Size {
    /// 皮肤卡片：内边距 4/12，12pt。
    case card
    /// 资源行：内边距 5/14，13pt。
    case row
  }
  let title: String
  /// 为 false 时绘制已完成的样式（使用中、已添加）。
  var offersAction = true
  var size: Size = .card
  var body: some View {
    Text(title)
      .font(.system(size: size == .card ? 12 : 13, weight: .semibold))
      .lineLimit(1).fixedSize()
      .foregroundStyle(offersAction ? MetasequoiaTheme.accent : MetasequoiaTheme.sub)
      .padding(.vertical, size == .card ? 4 : 5)
      .padding(.horizontal, size == .card ? 12 : 14)
      .background(offersAction ? MetasequoiaTheme.accentSoft : MetasequoiaTheme.segBg, in: Capsule())
      .contentShape(Capsule())
  }
}

/// 词库或回复行前面的 40pt 徽标：在 accentSoft 圆角方块上用 accent 色显示名称的第一个字。
struct CommunityGlyphTile: View {
  let name: String
  var body: some View {
    Text(String(name.trimmingCharacters(in: .whitespacesAndNewlines).prefix(1)))
      .font(.system(size: 16, weight: .bold))
      .foregroundStyle(MetasequoiaTheme.accent)
      .frame(width: 40, height: 40)
      .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous))
      .accessibilityHidden(true)
  }
}

/// 社区列表共用的标签，对应 Android 的 `CommunityRequest.usesLabel` / `resourceSubtitle` / `updatedThisWeek`。
enum CommunityListingText {
  /// 「386 次使用」，达到 10,000 起按 万 保留一位小数：「15.8 万 次使用」。
  static func uses(_ downloads: Int) -> String {
    let count = max(0, downloads)
    guard count >= 10_000 else { return "\(count) 次使用" }
    let tenths = Int((Double(count) / 1_000).rounded())
    let value = tenths % 10 == 0 ? "\(tenths / 10)" : "\(tenths / 10).\(tenths % 10)"
    return "\(value) 万 次使用"
  }

  /// 「@作者 · 4,812 条 · 本周更新」：条数只用于词库，只有条目带有最近七天内的 `updated_at` 时才显示更新提示。
  static func resourceMeta(_ item: CommunityResource, now: Date = Date()) -> String {
    var parts: [String] = []
    if !item.author.isEmpty { parts.append("@\(item.author)") }
    if item.kind == .dictionary { parts.append("\((item.content.entries?.count ?? 0).formatted(.number)) 条") }
    if updatedThisWeek(item.updated_at, now: now) { parts.append("本周更新") }
    return parts.joined(separator: " · ")
  }

  /// RFC 3339 时间是否落在 `now` 之前的七天内；nil、无法解析或未来的时间都不算。
  static func updatedThisWeek(_ value: String?, now: Date) -> Bool {
    guard let value, !value.isEmpty else { return false }
    let plain = ISO8601DateFormatter()
    let fractional = ISO8601DateFormatter()
    fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    guard let updated = plain.date(from: value) ?? fractional.date(from: value) else { return false }
    let age = now.timeIntervalSince(updated)
    return age >= 0 && age <= 7 * 24 * 60 * 60
  }
}

/// 社区 各列表共用的页面外壳：内容在季节画布上留 4/16/24 边距，regular 宽度下居中成一列，不超过设计稿 iPad 列表的宽度。
enum CommunityLayout {
  static let regularMaxWidth: CGFloat = 760
}

extension View {
  /// 按设计稿给社区列表加内边距（上 4、两侧 16、下 24），并在 regular 宽度下居中。
  func communityPageContent(regular: Bool) -> some View {
    frame(maxWidth: regular ? CommunityLayout.regularMaxWidth : .infinity)
      .frame(maxWidth: .infinity)
      .padding(.top, 4).padding(.horizontal, 16).padding(.bottom, 24)
  }
}
