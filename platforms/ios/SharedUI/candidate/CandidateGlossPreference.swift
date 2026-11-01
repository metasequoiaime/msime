import Foundation

/// Controls the offline annotation shown below iOS candidates.
///
/// The keyboard and its containing app share this value through the App Group. It is deliberately
/// Enabled by default now that glosses occupy their own row, matching the macOS candidate surface.
enum CandidateGlossPreference {
  static let key = "candidate.englishGloss"

  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }

  static var enabled: Bool {
    get { defaults.object(forKey: key) as? Bool ?? true }
    set { defaults.set(newValue, forKey: key) }
  }
}

/// 「显示读音」：共享偏好 `candidate_pronunciation` 在 App Group 里的镜像，键盘每次重读共享文档时更新，默认关闭。
///
/// 英文音标表是共享资源包 `pronunciations`（ECDICT 生成的 `en-phonetic.db`，约 5.7 MB），法、日、西、俄、德、韩释义的读音表是 `target-pronunciations`（维基词典生成的 `<语言>-phonetic.db`，合计不到 4 MB）：App 在用户点「下载」后经 `IOSResourcePacks` 装到共享状态目录的 `resource-packs/<id>/`，键盘只读本机文件，不联网。
enum CandidatePronunciationPreference {
  static let key = "candidate.pronunciation"
  static let sharedKey = "candidate_pronunciation"
  static let packID = "pronunciations"
  static let targetPackID = "target-pronunciations"

  static var enabled: Bool {
    get { CandidateGlossPreference.defaults.object(forKey: key) as? Bool ?? false }
    set { CandidateGlossPreference.defaults.set(newValue, forKey: key) }
  }

  /// 交给 `msime_client_pronunciation_request` 的补充资源目录：资源包的根，共享层在它下面找 `pronunciations/en-phonetic.db`。
  static func supplementaryResources(stateRoot: String) -> String {
    URL(fileURLWithPath: stateRoot, isDirectory: true).appendingPathComponent("resource-packs", isDirectory: true).path
  }

  /// 读音表装好没有：英文表，或者 `language`（释义的目标语言，小写代码）那一张，资源目录旁随包带着的或者下载的资源包都算。只看文件在不在，资源包是原子发布的，文件在就是装完了。
  static func tableInstalled(resources: String?, stateRoot: String, language: String = "en",
                             fileManager: FileManager = .default) -> Bool {
    var tables = ["pronunciations/en-phonetic.db"]
    if language != "en", CandidatePronunciationModel.languages.contains(language) {
      tables.append("\(targetPackID)/\(language)-phonetic.db")
    }
    var roots: [URL] = []
    if let resources, !resources.isEmpty { roots.append(URL(fileURLWithPath: resources).deletingLastPathComponent()) }
    if !stateRoot.isEmpty { roots.append(URL(fileURLWithPath: supplementaryResources(stateRoot: stateRoot), isDirectory: true)) }
    return roots.contains { root in tables.contains { fileManager.fileExists(atPath: root.appendingPathComponent($0).path) } }
  }
}
