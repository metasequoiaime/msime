import SwiftUI
import UIKit

enum AppIconStyle: String, CaseIterable, Identifiable {
  case classic, forest, sky, dusk, vermilion

  var id: String { rawValue }
  var assetSuffix: String {
    switch self {
    case .classic: "Classic"
    case .forest: "Forest"
    case .sky: "Sky"
    case .dusk: "Dusk"
    case .vermilion: "Vermilion"
    }
  }
  var iconName: String? { self == .classic ? nil : "AppIcon\(assetSuffix)" }
  var previewName: String { "AppIconPreview\(assetSuffix)" }
  var title: String {
    switch self {
    case .classic: "原版"
    case .forest: "杉林"
    case .sky: "晴空"
    case .dusk: "暮紫"
    case .vermilion: "朱砂"
    }
  }
  var detail: String {
    switch self {
    case .classic: "经典黑白，简洁如初"
    case .forest: "杉叶青绿，沉静自然"
    case .sky: "清透蓝调，轻盈明亮"
    case .dusk: "晚霞淡紫，温柔入夜"
    case .vermilion: "朱红印记，纸上东方"
    }
  }
}

@MainActor
protocol AppIconClient {
  var supportsAlternateIcons: Bool { get }
  var alternateIconName: String? { get }
  func setIcon(_ name: String?) async throws
}

@MainActor
private struct SystemAppIconClient: AppIconClient {
  var supportsAlternateIcons: Bool { UIApplication.shared.supportsAlternateIcons }
  var alternateIconName: String? { UIApplication.shared.alternateIconName }
  func setIcon(_ name: String?) async throws {
    try await UIApplication.shared.setAlternateIconName(name)
  }
}

@MainActor
final class AppIconSettingsModel: ObservableObject {
  @Published private(set) var selected: AppIconStyle?
  @Published private(set) var pending: AppIconStyle?
  @Published var errorMessage: String?
  private let client: any AppIconClient

  var isSupported: Bool { client.supportsAlternateIcons }

  init(client: any AppIconClient) {
    self.client = client
    refresh()
  }

  convenience init() { self.init(client: SystemAppIconClient()) }

  // The OS persists this choice. Read it again after returning to the app.
  func refresh() {
    guard pending == nil else { return }
    selected = AppIconStyle.allCases.first { $0.iconName == client.alternateIconName }
  }

  func select(_ icon: AppIconStyle) async {
    guard isSupported, pending == nil, icon != selected else { return }
    pending = icon
    errorMessage = nil
    do {
      try await client.setIcon(icon.iconName)
    } catch {
      // The Simulator reports an I/O error even when the icon was applied. Report the failure
      // only when the system still holds a different icon.
      if client.alternateIconName != icon.iconName {
        errorMessage = "图标未能更换，请稍后重试。\n\(error.localizedDescription)"
      }
    }
    pending = nil
    refresh()
  }
}

struct AppIconSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @StateObject private var model = AppIconSettingsModel()

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 24) {
        VStack(alignment: .leading, spacing: 8) {
          Text("让水杉，带上你的颜色")
            .font(.title2.bold())
          Text("挑选喜欢的图标，点一下换到主屏幕。")
            .font(.subheadline).foregroundStyle(.secondary)
        }
        .padding(.top, 8)

        if !model.isSupported {
          Label("当前设备暂不支持更换 App 图标。", systemImage: "info.circle")
            .font(.footnote).foregroundStyle(.secondary)
        }

        LazyVGrid(columns: [GridItem(.adaptive(minimum: 144), spacing: 14)], spacing: 14) {
          ForEach(AppIconStyle.allCases) { icon in
            iconCard(icon)
          }
        }
        Text("更换后，系统可能会显示确认提示。随时可以切回原版。")
          .font(.footnote).foregroundStyle(.secondary)
      }
      .padding(20)
      .frame(maxWidth: 680)
      .frame(maxWidth: .infinity)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("App 图标")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear { model.refresh() }
    .onChange(of: scenePhase) { if $0 == .active { model.refresh() } }
    .alert("暂时无法更换图标", isPresented: Binding(
      get: { model.errorMessage != nil },
      set: { if !$0 { model.errorMessage = nil } }
    )) {
      Button("好", role: .cancel) { model.errorMessage = nil }
    } message: {
      Text(model.errorMessage ?? "")
    }
  }

  private func iconCard(_ icon: AppIconStyle) -> some View {
    let selected = model.selected == icon
    let pending = model.pending == icon
    return Button {
      Task { await model.select(icon) }
    } label: {
      VStack(spacing: 12) {
        Image(icon.previewName)
          .resizable().interpolation(.high).aspectRatio(contentMode: .fit)
          .frame(width: 88, height: 88)
          .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
          .overlay(RoundedRectangle(cornerRadius: 20).strokeBorder(.primary.opacity(0.06)))
          .accessibilityHidden(true)
        VStack(spacing: 4) {
          Text(icon.title).font(.headline).foregroundStyle(.primary)
          Text(icon.detail).font(.caption).foregroundStyle(.secondary)
            .multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true)
        }
        HStack(spacing: 5) {
          if pending {
            ProgressView().controlSize(.small)
          } else {
            Image(systemName: selected ? "checkmark.circle.fill" : "circle")
          }
          Text(pending ? "更换中" : selected ? "使用中" : "使用此图标")
        }
        .font(.caption.weight(.medium))
        .foregroundStyle(selected ? MetasequoiaTheme.accent : .secondary)
      }
      .frame(maxWidth: .infinity)
      .padding(.vertical, 20).padding(.horizontal, 10)
      // 设计稿里的卡片放在季节卡片色上，没有边框也没有阴影；只有选中的图标带一圈主色描边。
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: 22))
      .overlay(RoundedRectangle(cornerRadius: 22).strokeBorder(selected ? MetasequoiaTheme.accent : .clear, lineWidth: 2))
      .contentShape(RoundedRectangle(cornerRadius: 22))
    }
    .buttonStyle(.plain)
    .disabled(!model.isSupported || model.pending != nil)
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("\(icon.title)，\(icon.detail)")
    .accessibilityValue(pending ? "更换中" : selected ? "使用中" : "未选择")
    .accessibilityAddTraits(.isButton)
    .accessibilityAddTraits(selected ? [.isSelected] : [])
    .accessibilityIdentifier("appIcon_\(icon.rawValue)")
  }
}
