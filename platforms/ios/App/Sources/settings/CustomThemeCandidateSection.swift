import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// 外部皮肤与候选颜色, the candidate half of the custom theme, on the 主题 page with the rest of 自定义主题及高级 (DECISIONS: 外部皮肤 / 取色器 move to 主题).
///
/// The candidate package, mode and colours are part of the shared global theme (`custom_theme`) and sync with the desktop; the switch that lets them replace the keyboard palette on the strip is iOS-only (see CandidatePalette). Picking a package or a colour selects the custom theme, so `onThemeChange` lets the page refresh its theme cards and preview.
struct CustomThemeCandidateSection: View {
  var onThemeChange: () -> Void = {}
  @Environment(\.scenePhase) private var scenePhase
  @Environment(\.colorScheme) private var colorScheme
  @AppStorage(CandidatePalette.followsDesktopKey, store: CandidatePalette.defaults)
  private var followsDesktopPalette = false
  @AppStorage(GlobalThemePreference.key, store: KeyboardFeedbackPreference.defaults)
  private var selectedTheme = GlobalThemeCatalog.systemId
  /// `custom_theme.candidate_skin`, empty when the custom theme uses no package.
  @State private var candidateSkin = ""
  @State private var candidateTheme = "follow"
  @State private var appMode = "system"
  @State private var themeId = GlobalThemeCatalog.systemId
  @State private var customTheme: [String: Any] = [:]
  @State private var candidateColors: [String: String] = [:]
  @State private var installedSkins: [(id: String, skin: ExternalCandidateSkin)] = []
  @State private var importingSkin = false
  @State private var skinStatus = ""
  @State private var saveFailed = false
  private let tablet = UIDevice.current.userInterfaceIdiom == .pad

  var body: some View {
    Section {
      Toggle(isOn: $followsDesktopPalette) {
        VStack(alignment: .leading, spacing: 2) {
          Text("候选栏使用主题配色")
          Text("关闭时候选栏和按键一起使用键盘皮肤的颜色；打开后使用本页的主题和候选颜色。").font(.footnote).foregroundStyle(.secondary)
        }
      }.accessibilityIdentifier("candidatePaletteFollowsDesktop")
      if followsDesktopPalette {
        Picker("外部皮肤", selection: packageBinding) {
          Text("不使用外部皮肤").tag("")
          ForEach(installedSkins.filter { $0.skin.horizontal }, id: \.id) { package in
            Text(Self.title(package.skin)).tag(package.id)
          }
        }.accessibilityIdentifier("candidateSkin")
        Button("导入皮肤…") { importingSkin = true }
          .accessibilityIdentifier("candidateSkinImport")
        if let selected = installedSkins.first(where: { $0.id == candidateSkin }) {
          Button("删除「\(selected.skin.name)」", role: .destructive) { removeSkin(selected.id) }
            .accessibilityIdentifier("candidateSkinRemove")
        }
        if !skinStatus.isEmpty {
          Text(skinStatus).font(.footnote).foregroundStyle(.secondary)
            .accessibilityIdentifier("candidateSkinStatus")
        }
        Picker("明暗", selection: Binding(get: { candidateTheme }, set: { value in
          candidateTheme = value
          saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { $0["candidate_theme"] = value }
          if saveFailed { reload() }
          onThemeChange()
        })) {
          ForEach(CandidatePalette.themes, id: \.id) { Text($0.title).tag($0.id) }
        }.accessibilityIdentifier("candidateTheme")
        ForEach(CandidatePalette.editableColors, id: \.slot) { item in
          ColorPicker(item.title, selection: colorBinding(item.slot), supportsOpacity: false)
            .accessibilityIdentifier("candidate_\(item.slot)_color")
        }
        if !candidateColors.isEmpty {
          Button("恢复皮肤颜色", role: .destructive) { themeWrite(GlobalThemePreference.clearingCandidateColors) }
            .accessibilityIdentifier("candidateColorsReset")
        }
        palettePreview
      }
    } header: {
      Text("外部皮肤与候选颜色")
    } footer: {
      Text(saveFailed
        ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
        : followsDesktopPalette
        ? "候选栏使用全局主题的候选配色，与电脑版的候选窗同步；首选候选使用主题的高亮色。选外部皮肤或改颜色会切换到「自定义」主题，并以当前主题为底。“明暗”选跟随系统时，先看共享的主题设置。\n\n“导入皮肤”从“文件”里选一个含 skin.toml 的皮肤文件夹，复制进键盘能读到的共享目录，同名皮肤整个替换。候选栏是横排的，只列出支持横排的皮肤；皮肤没声明的明暗下使用它所基于的主题。"
        : "默认关闭，候选栏和按键一起使用键盘主题的颜色。打开后可以导入电脑版的外部候选皮肤、调整候选颜色。")
    }
    .fileImporter(isPresented: $importingSkin, allowedContentTypes: [.folder]) { importSkin($0) }
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in if phase == .active { reload() } }
    .onChange(of: selectedTheme) { reload() }
    .onChange(of: followsDesktopPalette) { onThemeChange() }
  }

  /// The package the custom theme draws; picking one selects the custom theme over the package's base, and 不使用外部皮肤 drops it from the custom theme.
  private var packageBinding: Binding<String> {
    Binding(get: { themeId == GlobalThemeCatalog.customId ? candidateSkin : "" }, set: { id in
      if id.isEmpty {
        themeWrite(GlobalThemePreference.clearingPackage)
      } else if let package = installedSkins.first(where: { $0.id == id }) {
        themeWrite(GlobalThemePreference.applyingPackage(id, base: package.skin.base))
      }
    })
  }

  /// A package that declares one appearance says so, since the other falls back to the default skin.
  private static func title(_ skin: ExternalCandidateSkin) -> String {
    if skin.themes == ["light"] { return skin.name + "（仅浅色）" }
    if skin.themes == ["dark"] { return skin.name + "（仅深色）" }
    return skin.name
  }

  /// The copy runs off the main thread: a picked folder can hold up to the import's size limit of assets.
  private func importSkin(_ result: Result<URL, Error>) {
    guard case .success(let source) = result, let root = ExternalCandidateSkin.defaultRoot else { return }
    skinStatus = "正在导入…"
    DispatchQueue.global(qos: .userInitiated).async {
      let scoped = source.startAccessingSecurityScopedResource()
      let outcome = ExternalCandidateSkin.importFolder(source, root: root)
      if scoped { source.stopAccessingSecurityScopedResource() }
      let installed = ExternalCandidateSkin.scan(root) ?? []
      DispatchQueue.main.async { finishImport(outcome, installed: installed) }
    }
  }

  private func finishImport(_ outcome: Result<String, ExternalCandidateSkin.ImportFailure>,
                            installed: [(id: String, skin: ExternalCandidateSkin)]) {
    installedSkins = installed
    switch outcome {
    case .success(let id):
      guard let package = installed.first(where: { $0.id == id }) else {
        skinStatus = "已复制，但 skin.toml 有误，这款皮肤不能使用。"
        return
      }
      guard package.skin.horizontal else {
        skinStatus = "已导入「\(package.skin.name)」，但它只支持竖排候选窗，iOS 的横排候选栏用不了。"
        return
      }
      themeWrite(GlobalThemePreference.applyingPackage(id, base: package.skin.base))
      skinStatus = saveFailed ? "" : "已导入并选用「\(package.skin.name)」。"
    case .failure(.name):
      skinStatus = "文件夹名只能用小写英文字母、数字、点、下划线和连字符，并以字母或数字开头，也不能和内置皮肤同名。"
    case .failure(.manifest):
      skinStatus = "这个文件夹里没有 skin.toml，不是候选皮肤。"
    case .failure(.storage):
      skinStatus = "导入失败：文件夹读不出来，或者超过 4096 个文件、256 MB。"
    }
  }

  private func removeSkin(_ id: String) {
    guard let root = ExternalCandidateSkin.defaultRoot else { return }
    guard ExternalCandidateSkin.remove(id, root: root) else {
      skinStatus = "删除失败，请再试一次。"
      return
    }
    installedSkins = ExternalCandidateSkin.scan(root) ?? []
    if id == candidateSkin { themeWrite(GlobalThemePreference.clearingPackage) }
    skinStatus = ""
  }

  /// iPad has the width to show both appearances at once; the phone shows the one the keyboard is drawing now.
  @ViewBuilder private var palettePreview: some View {
    if tablet {
      HStack(spacing: 12) {
        previewStrip(dark: false)
        previewStrip(dark: true)
      }
    } else {
      previewStrip(dark: colorScheme == .dark)
    }
  }

  private func previewStrip(dark: Bool) -> some View {
    let palette = CandidatePalette.resolve(previewPreferences, systemDark: dark)
    return HStack(spacing: 6) {
      ForEach(Array(["水杉", "输入", "输入法"].enumerated()), id: \.offset) { index, word in
        HStack(spacing: 3) {
          Text("\(index + 1)").font(.caption).foregroundStyle(Color(palette.number))
          Text(word).foregroundStyle(Color(palette.text))
        }
        .padding(.horizontal, 9).padding(.vertical, 5)
        .background(RoundedRectangle(cornerRadius: 9).fill(Color(index == 0 ? palette.hover : palette.surface)))
        .overlay(RoundedRectangle(cornerRadius: 9).stroke(Color(palette.border)))
      }
    }
    .padding(8).frame(maxWidth: .infinity, alignment: .leading)
    .background(RoundedRectangle(cornerRadius: 12).fill(Color(palette.surface)))
    .accessibilityElement(children: .ignore)
    .accessibilityLabel(dark ? "深色预览" : "浅色预览")
  }

  private var previewPreferences: [String: Any] {
    ["global_theme": themeId, "custom_theme": customTheme, "candidate_theme": candidateTheme, "theme": appMode]
  }

  /// A picked colour; while unset the picker shows the theme's own colour for the current appearance. Picking one moves to the custom theme (THEME_CONTRACT section 5).
  private func colorBinding(_ slot: String) -> Binding<Color> {
    Binding(get: {
      if let custom = ThemeColor.parse(candidateColors[slot]) { return Color(custom) }
      let palette = CandidatePalette.resolve(previewPreferences, systemDark: colorScheme == .dark)
      switch slot {
      case "text": return Color(palette.text)
      case "hover": return Color(palette.hover)
      default: return Color(palette.surface)
      }
    }, set: { color in
      let hex = ThemeColor.hex(UIColor(color))
      candidateColors[slot] = hex
      themeWrite(GlobalThemePreference.pickingCandidateColor(slot, hex: hex))
    })
  }

  /// Write theme fields of the document, keeping the App Group's copy of the selection in step.
  private func themeWrite(_ mapping: (inout [String: Any]) -> Void) {
    saveFailed = !GlobalThemePreference.update(mapping)
    reload()
    onThemeChange()
  }

  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    themeId = GlobalThemePreference.theme(in: preferences)
    customTheme = GlobalThemePreference.customTheme(in: preferences)
    candidateSkin = customTheme["candidate_skin"] as? String ?? ""
    installedSkins = ExternalCandidateSkin.defaultRoot.flatMap(ExternalCandidateSkin.scan) ?? []
    candidateTheme = preferences["candidate_theme"] as? String ?? "follow"
    appMode = preferences["theme"] as? String ?? "system"
    let picked = customTheme["candidate_colors"] as? [String: Any] ?? [:]
    candidateColors = CandidatePalette.editableColors.reduce(into: [:]) { colors, item in
      if let hex = picked[item.slot] as? String, ThemeColor.parse(hex) != nil { colors[item.slot] = hex }
    }
  }
}
