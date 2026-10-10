import SwiftUI

struct ProviderIcon: View {
  let id: String
  var size: CGFloat = 40
  /// Choices drawn with a system symbol rather than a provider logo: the custom endpoint and the two on-device recognizers.
  private static let symbols = ["custom": "slider.horizontal.3", "local": "cpu", "system": "waveform"]

  var body: some View {
    Group {
      if let symbol = Self.symbols[id] {
        Image(systemName: symbol)
          .font(.system(size: size * 0.45, weight: .medium))
          .foregroundStyle(MetasequoiaTheme.accent)
      } else {
        // 阿里云百炼的语音识别用千问的标志。
        Image(id == "everyAPI" ? "EveryAPI" : id == "bailian" ? "Provider-qwen" : "Provider-\(id)")
          .resizable().scaledToFit().padding(id == "everyAPI" ? 0 : size * 0.16)
      }
    }
    .frame(width: size, height: size)
    .background(Color(uiColor: .secondarySystemGroupedBackground))
    .clipShape(RoundedRectangle(cornerRadius: size * 0.25, style: .continuous))
    .accessibilityIdentifier(id == "everyAPI" ? "everyAPIProviderLogo" : "providerLogo_\(id)")
  }
}

struct ProviderOption: Identifiable {
  let id: String
  let title: String
  let endpoint: String
  /// Shown instead of the endpoint's host, for a choice that has no endpoint.
  var note: String?
  var subtitle: String { note ?? URL(string: endpoint)?.host ?? "连接自己的服务或代理" }
}

struct ProviderPickerView: View {
  let options: [ProviderOption]
  let selected: String
  let onSelect: (String) -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var search = ""

  var body: some View {
    NavigationView {
      List {
        ForEach(options.filter { search.isEmpty || $0.title.localizedCaseInsensitiveContains(search)
          || $0.subtitle.localizedCaseInsensitiveContains(search) }) { option in
          Button {
            onSelect(option.id)
            dismiss()
          } label: {
            HStack(spacing: 14) {
              ProviderIcon(id: option.id)
              VStack(alignment: .leading, spacing: 4) {
                Text(option.title).font(.body.weight(.medium)).foregroundStyle(.primary)
                Text(option.subtitle).font(.caption).foregroundStyle(.secondary)
              }
              Spacer(minLength: 8)
              if selected == option.id {
                Image(systemName: "checkmark.circle.fill")
                  .foregroundStyle(MetasequoiaTheme.accent)
              }
            }.contentShape(Rectangle()).padding(.vertical, 5)
          }
          .buttonStyle(.plain)
          .accessibilityLabel(option.title)
          .accessibilityValue(selected == option.id ? "已选择" : "")
        }
      }
      .searchable(text: $search, placement: .navigationBarDrawer(displayMode: .always), prompt: "搜索服务商")
      .navigationTitle("选择服务商")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() } } }
    }.navigationViewStyle(.stack)
      .tint(MetasequoiaTheme.accent)
  }
}
