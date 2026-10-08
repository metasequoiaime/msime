import SwiftUI

/// 明暗与候选颜色：原先放在皮肤网格下面的外观选择器和自定义主题的候选颜色。每个控件都和在原处时一样，写入会同步的共享文档。
struct SkinAppearanceSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @Environment(\.horizontalSizeClass) private var sizeClass
  @State private var themes: [String: String] = [:]
  @State private var themeSaveFailed = false

  var body: some View {
    Form {
      appearanceSection.designRow()
      CustomThemeCandidateSection().designRow()
    }
    .designPage()
    .navigationTitle("明暗与候选颜色")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reloadThemes)
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { reloadThemes() }
    }
  }

  /// 共享文档里的「键盘明暗」（见 `KeyboardAppearancePreference`）。iPad 把键盘之外的各个面板也列出来；手机上把它们收起，因为大多数人只会设置键盘这一项。
  private var appearanceSection: some View {
    Section {
      Picker("颜色模式", selection: theme(AppAppearancePreference.globalKey, fallback: "system")) {
        ForEach(AppAppearancePreference.globalOptions, id: \.id) { Text($0.title).tag($0.id) }
      }.accessibilityIdentifier("globalTheme")
      Picker("设置界面", selection: theme(AppAppearancePreference.settingsKey)) {
        ForEach(AppAppearancePreference.settingsOptions, id: \.id) { Text($0.title).tag($0.id) }
      }.accessibilityIdentifier("settingsTheme")
      Picker("键盘", selection: theme(KeyboardAppearancePreference.keyboardKey)) {
        ForEach(KeyboardAppearancePreference.options, id: \.id) { Text($0.title).tag($0.id) }
      }.accessibilityIdentifier("keyboardTheme")
      if sizeClass == .regular {
        panelPickers
      } else {
        DisclosureGroup("面板明暗") { panelPickers }.accessibilityIdentifier("keyboardPanelThemes")
      }
    } header: {
      SettingsGroupHeader(title: "明暗")
    } footer: {
      Text(themeSaveFailed
        ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
        : "与电脑版的颜色模式、设置界面、屏幕键盘、手写、表情和语音主题同步。颜色模式是各处选“跟随”时的默认值；设置界面就是这个 App，立即生效。键盘选“跟随系统”时先看颜色模式，再跟随当前 App 的外观；面板选“跟随键盘”时和键盘一致。键盘和面板下次打开水杉键盘时应用。")
    }
  }

  private var panelPickers: some View {
    ForEach(KeyboardAppearancePreference.panels, id: \.key) { panel in
      Picker(panel.title, selection: theme(panel.key)) {
        ForEach(KeyboardAppearancePreference.panelOptions, id: \.id) { Text($0.title).tag($0.id) }
      }.accessibilityIdentifier(panel.key)
    }
  }

  private func theme(_ key: String, fallback: String = "follow") -> Binding<String> {
    Binding(get: { themes[key] ?? fallback }, set: { value in
      themes[key] = value
      themeSaveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { $0[key] = value }
      if themeSaveFailed { reloadThemes() }
      if key == AppAppearancePreference.globalKey || key == AppAppearancePreference.settingsKey {
        NotificationCenter.default.post(name: AppAppearancePreference.didChange, object: nil)
      }
    })
  }

  private func reloadThemes() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    let keys = [AppAppearancePreference.settingsKey, KeyboardAppearancePreference.keyboardKey] + KeyboardAppearancePreference.panels.map(\.key)
    themes = keys.reduce(into: [:]) { themes, key in themes[key] = preferences[key] as? String ?? "follow" }
    themes[AppAppearancePreference.globalKey] = preferences[AppAppearancePreference.globalKey] as? String ?? "system"
  }
}
