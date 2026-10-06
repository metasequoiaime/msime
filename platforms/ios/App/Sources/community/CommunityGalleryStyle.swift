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
    }.padding(10).background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
      .overlay(RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous).strokeBorder(Color.primary.opacity(0.035), lineWidth: 1))
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
