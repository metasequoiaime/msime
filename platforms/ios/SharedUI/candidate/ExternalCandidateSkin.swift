import Foundation

@_silgen_name("msime_client_skin_catalog")
private func msimeSkinCatalog(_ directory: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_skin_import")
private func msimeSkinImport(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_string_free")
private func msimeSkinCatalogStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// A candidate skin package the user imported into `<App Group>/MSIME/skins`, as the Rust catalog scan lists it, so a package the settings page reports as an issue is never offered.
///
/// 绘制归 client-core：自定义主题在浅色槽位 `custom_theme.candidate_skin` 或深色槽位 `custom_theme.candidate_skin_dark` 里指名它，`msime_client_resolve_theme` 从这个目录读取。与 Windows 候选窗一样，皮肤包只在清单声明的排列和明暗下绘制，并且只在它的底所属的明暗下绘制；候选栏是横排的。
struct ExternalCandidateSkin: Equatable {
  var name = ""
  /// The global theme the package is drawn over (`base` in skin.toml), which becomes the custom theme's base when it is picked.
  let base: String
  /// `light` / `dark` → colour key → value, exactly as the manifest spells them.
  let candidate: [String: [String: String]]
  let themes: Set<String>
  let horizontal: Bool

  /// The directory the settings app imports skins into; the keyboard extension shares it through the App Group.
  static var defaultRoot: URL? {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier)?
      .appendingPathComponent("MSIME", isDirectory: true)
      .appendingPathComponent("skins", isDirectory: true)
  }

  /// The catalog's packages, or nil when the ABI refused the root.
  static func scan(_ root: URL) -> [(id: String, skin: ExternalCandidateSkin)]? {
    let path = Array(root.path.utf8)
    guard path.count <= 16384 else { return nil }
    let raw = path.withUnsafeBufferPointer { msimeSkinCatalog($0.baseAddress, UInt($0.count)) }
    guard let raw else { return nil }
    defer { msimeSkinCatalogStringFree(raw) }
    guard let reply = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any],
          reply["ok"] as? Bool == true,
          let value = reply["value"] as? [String: Any],
          let packages = value["packages"] as? [[String: Any]] else { return nil }
    return packages.compactMap { package in
      guard let id = package["id"] as? String, let base = package["base"] as? String else { return nil }
      var candidate: [String: [String: String]] = [:]
      for theme in ["light", "dark"] {
        guard let palette = (package["candidate"] as? [String: Any])?[theme] as? [String: Any] else { continue }
        candidate[theme] = palette.compactMapValues { $0 as? String }
      }
      let themes = Set(package["themes"] as? [String] ?? [])
      let layouts = package["layouts"] as? [String] ?? []
      return (id, ExternalCandidateSkin(name: package["name"] as? String ?? id, base: base, candidate: candidate,
                                        themes: themes, horizontal: layouts.contains("horizontal")))
    }
  }

  enum ImportFailure: Error, Equatable {
    /// The folder's name is not one the catalog lists: lowercase ASCII letters, digits, `.`, `_` and `-`, starting with a letter or digit, and not a global theme id.
    case name
    /// The folder has no `skin.toml`.
    case manifest
    case storage
  }

  /// Copy a folder the user picked in Files into `root`, the way the Tauri shell imports one on iOS: the same Rust import, which checks the name and manifest before copying and replaces a skin of the same name whole. Returns the id the catalog lists it under. Touches the disk; call it off the main thread.
  static func importFolder(_ source: URL, root: URL) -> Result<String, ImportFailure> {
    guard let request = try? JSONSerialization.data(withJSONObject: ["source": source.path, "directory": root.path])
    else { return .failure(.storage) }
    let raw = request.withUnsafeBytes { bytes in
      msimeSkinImport(bytes.bindMemory(to: UInt8.self).baseAddress, UInt(bytes.count))
    }
    guard let raw else { return .failure(.storage) }
    defer { msimeSkinCatalogStringFree(raw) }
    guard let reply = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any]
    else { return .failure(.storage) }
    if reply["ok"] as? Bool == true, let id = (reply["value"] as? [String: Any])?["id"] as? String { return .success(id) }
    switch reply["error"] as? String {
    case "skin_name": return .failure(.name)
    case "skin_manifest": return .failure(.manifest)
    default: return .failure(.storage)
    }
  }

  /// Delete an imported skin. Only a name the catalog could list is accepted, so nothing outside `root` is reachable.
  static func remove(_ id: String, root: URL) -> Bool {
    let bytes = Array(id.utf8)
    guard let first = bytes.first, bytes.count <= 64,
          (first >= 0x61 && first <= 0x7a) || (first >= 0x30 && first <= 0x39),
          bytes.allSatisfy({ ($0 >= 0x61 && $0 <= 0x7a) || ($0 >= 0x30 && $0 <= 0x39) || $0 == 0x2e || $0 == 0x5f || $0 == 0x2d })
    else { return false }
    let folder = root.appendingPathComponent(id, isDirectory: true)
    return (try? FileManager.default.removeItem(at: folder)) != nil
  }
}
