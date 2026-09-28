import Foundation

struct KeyboardSkinTrial: Codable, Identifiable {
  let id: UUID
  let name: String
  let previousSelection: String?
  let previousDesign: Data?
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
  func begin(name: String, design: CustomKeyboardSkin) throws -> KeyboardSkinTrial {
    try restorePending()
    let trial = KeyboardSkinTrial(id: UUID(), name: name,
      previousSelection: defaults.string(forKey: KeyboardSkinPreference.key),
      previousDesign: defaults.data(forKey: CustomKeyboardSkinStore.key), design: design.normalized)
    let data = try JSONEncoder().encode(trial)
    try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    try data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    // The keyboard takes its skin from the shared document, so a trial the document did not take would not show.
    guard KeyboardSkinPreference.writeDocument(.custom, design: trial.design, stateRoot: stateRoot) else {
      try? FileManager.default.removeItem(at: file)
      throw PersonalDictionaryStore.StoreError.unavailable
    }
    defaults.set(try JSONEncoder().encode(trial.design), forKey: CustomKeyboardSkinStore.key)
    defaults.set(KeyboardSkin.custom.rawValue, forKey: KeyboardSkinPreference.key)
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
    // Do not undo a different skin explicitly selected while the trial was open.
    guard defaults.string(forKey: KeyboardSkinPreference.key) == KeyboardSkin.custom.rawValue,
          let data = defaults.data(forKey: CustomKeyboardSkinStore.key),
          (try? JSONDecoder().decode(CustomKeyboardSkin.self, from: data)) == trial.design else { return }
    let previousDesign = trial.previousDesign.flatMap { try? JSONDecoder().decode(CustomKeyboardSkin.self, from: $0) }
    _ = KeyboardSkinPreference.writeDocument(
      trial.previousSelection.flatMap(KeyboardSkin.init(rawValue:)) ?? .forest,
      design: previousDesign ?? CustomKeyboardSkin(), stateRoot: stateRoot)
    if let previous = trial.previousDesign { defaults.set(previous, forKey: CustomKeyboardSkinStore.key) }
    else { defaults.removeObject(forKey: CustomKeyboardSkinStore.key) }
    if let previous = trial.previousSelection { defaults.set(previous, forKey: KeyboardSkinPreference.key) }
    else { defaults.removeObject(forKey: KeyboardSkinPreference.key) }
  }
}
