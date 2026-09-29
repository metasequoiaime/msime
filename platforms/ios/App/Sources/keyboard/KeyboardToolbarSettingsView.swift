import SwiftUI

/// 键盘工具栏: which features sit on the keyboard's top toolbar. It used to be a section of 键盘设置; the design gives it a page of its own under 设置.
struct KeyboardToolbarSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var toolbar = TouchToolbarPreference()
  @State private var saveFailed = false

  var body: some View {
    Form {
      Section {
        ForEach(TouchToolbarPreference.options, id: \.name) { option in
          Toggle(option.title, isOn: Binding(
            get: { toolbar[keyPath: option.keyPath] },
            set: { enabled in
              var next = toolbar
              next[keyPath: option.keyPath] = enabled
              save(next)
            }))
          .accessibilityIdentifier("appToolbar_\(option.name)")
        }
      } header: {
        SettingsGroupHeader(title: "工具栏按钮")
      } footer: {
        Text(saveFailed
          ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
          : "打开的功能显示在键盘顶部工具栏；关掉的仍在键盘的「更多」里。已经打开的键盘要重新唤出才生效。")
      }
    }
    .navigationTitle("键盘工具栏").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
  }

  /// A refused write leaves the switch where the document is, instead of showing a bar the keyboard will not draw.
  private func save(_ next: TouchToolbarPreference) {
    saveFailed = !TouchToolbarPreference.save(next)
    toolbar = saveFailed ? TouchToolbarPreference.load() : next
  }

  /// The document is what the keyboard will use, including a value synced from another device that no keyboard has mirrored into the App Group yet.
  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    toolbar = TouchToolbarPreference(in: preferences)
  }
}
