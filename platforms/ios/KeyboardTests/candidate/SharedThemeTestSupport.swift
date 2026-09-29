import XCTest

// Selecting a theme from the keyboard or the app writes the shared document the simulator keeps between runs, and its App Group copies. A test that selects one puts all of them back, so a later test (or a UI test) starts from the theme the simulator had.
extension XCTestCase {
  func preserveSharedTheme() {
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    let theme = document?["global_theme"]
    let custom = document?["custom_theme"]
    let defaults = KeyboardFeedbackPreference.defaults
    let selection = defaults.object(forKey: GlobalThemePreference.key)
    let design = defaults.object(forKey: CustomKeyboardSkinStore.key)
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { document in
        document["global_theme"] = theme
        document["custom_theme"] = custom
      }
      if let selection { defaults.set(selection, forKey: GlobalThemePreference.key) } else { defaults.removeObject(forKey: GlobalThemePreference.key) }
      if let design { defaults.set(design, forKey: CustomKeyboardSkinStore.key) } else { defaults.removeObject(forKey: CustomKeyboardSkinStore.key) }
      KeyboardTheme.reload(MetasequoiaInputSessionBridge.loadSharedPreferences())
    }
  }
}
