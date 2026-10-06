import Foundation
import Darwin

struct KeyboardSkinTrial: Codable, Identifiable {
  let id: UUID
  let name: String
  /// The App Group `globalTheme` before the trial.
  let previousSelection: String?
  /// The App Group design before the trial.
  let previousDesign: Data?
  /// The document's `global_theme` before the trial.
  let previousTheme: String?
  /// The document's `custom_theme` before the trial, as JSON, nil when it had none.
  let previousCustomTheme: Data?
  let design: CustomKeyboardSkin
}

// Persist the undo record before applying a trial. A killed app restores it on next launch.
struct KeyboardSkinTrialStore {
  private let file: URL
  private let defaults: UserDefaults
  private let stateRoot: URL?
  /// `stateRoot` is for tests, like `MetasequoiaInputSessionBridge.updateSharedPreferences`'s.
  init(directory: URL? = nil, defaults: UserDefaults = KeyboardFeedbackPreference.defaults,
       stateRoot: URL? = nil) throws {
    guard let directory = directory ?? FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier) else {
      throw PersonalDictionaryStore.StoreError.unavailable
    }
    self.file = directory.appendingPathComponent("KeyboardSkinTrial.json")
    self.defaults = defaults
    self.stateRoot = stateRoot
  }
  private func rejectSymlinkAncestors(_ path: URL) throws {
    guard !SafePath.hasRefusedSymbolicLink(path) else { throw PersonalDictionaryStore.StoreError.unavailable }
  }
  func begin(name: String, design: CustomKeyboardSkin) throws -> KeyboardSkinTrial {
    try rejectSymlinkAncestors(file)
    try restorePending()
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: stateRoot)
    let custom = document?["custom_theme"] as? [String: Any]
    let trial = KeyboardSkinTrial(id: UUID(), name: name,
      previousSelection: defaults.string(forKey: GlobalThemePreference.key),
      previousDesign: defaults.data(forKey: CustomKeyboardSkinStore.key),
      previousTheme: document?["global_theme"] as? String,
      previousCustomTheme: custom.flatMap { try? JSONSerialization.data(withJSONObject: $0) },
      design: design.normalized)
    let data = try JSONEncoder().encode(trial)
    try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    try data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    // The keyboard takes its theme from the shared document, so a trial the document did not take would not show.
    guard let mapping = GlobalThemePreference.applyingDesign(trial.design),
          MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, mapping) else {
      try? FileManager.default.removeItem(at: file)
      throw PersonalDictionaryStore.StoreError.unavailable
    }
    defaults.set(try JSONEncoder().encode(trial.design), forKey: CustomKeyboardSkinStore.key)
    defaults.set(GlobalThemeCatalog.customId, forKey: GlobalThemePreference.key)
    return trial
  }
  func finish(_ id: UUID, keep: Bool) throws {
    guard let trial = try pending(), trial.id == id else { return }
    if !keep { restore(trial) }
    try FileManager.default.removeItem(at: file)
  }
  func restorePending() throws {
    guard let trial = try pending() else { return }
    try finish(trial.id, keep: false)
  }
  private func pending() throws -> KeyboardSkinTrial? {
    try rejectSymlinkAncestors(file)
    guard FileManager.default.fileExists(atPath: file.path) else { return nil }
    guard let size = try file.resourceValues(forKeys: [.fileSizeKey]).fileSize, size <= 2_000_000 else {
      throw PersonalDictionaryStore.StoreError.invalidState
    }
    let data: Data
    do {
      data = try BoundedFileReader.read(from: file, maximumBytes: 2_000_000)
    } catch {
      throw PersonalDictionaryStore.StoreError.invalidState
    }
    return try JSONDecoder().decode(KeyboardSkinTrial.self, from: data)
  }
  private func restore(_ trial: KeyboardSkinTrial) {
    // Do not undo a different theme or design explicitly selected while the trial was open.
    guard defaults.string(forKey: GlobalThemePreference.key) == GlobalThemeCatalog.customId,
          let data = defaults.data(forKey: CustomKeyboardSkinStore.key),
          (try? JSONDecoder().decode(CustomKeyboardSkin.self, from: data)) == trial.design else { return }
    let previousCustom = trial.previousCustomTheme.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
    _ = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { document in
      document["global_theme"] = trial.previousTheme ?? GlobalThemeCatalog.systemId
      if let previousCustom { document["custom_theme"] = previousCustom } else { document.removeValue(forKey: "custom_theme") }
    }
    if let previous = trial.previousDesign { defaults.set(previous, forKey: CustomKeyboardSkinStore.key) }
    else { defaults.removeObject(forKey: CustomKeyboardSkinStore.key) }
    if let previous = trial.previousSelection { defaults.set(previous, forKey: GlobalThemePreference.key) }
    else { defaults.removeObject(forKey: GlobalThemePreference.key) }
  }
}
