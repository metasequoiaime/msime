import Foundation

/// 按键音的声音类别，与 Android `ImeKeyFeedback` 的 `KEY_*`、HarmonyOS 的 `KeySoundClass` 和 client-services 的 `key_sample` 相同：空格、回车、退格各有一声，其余的键都是普通键。
enum KeySoundClass: CaseIterable {
  case standard, space, enter, backspace
}

/// 按键音效包的选择与播放规则，键盘扩展和设置 App 共用。这里不碰音频和 SwiftUI，单元测试直接测它。
///
/// 按键音开关仍是 App Group 里的 `KeyboardFeedbackPreference.soundEnabled`（与 Android、HarmonyOS 一样是本机开关）；用哪个包取共享偏好 `plugins.key_sound.pack`，没选时是内置的 `default`（「清脆键盘」），音量取 `plugins.key_sound.volume`（0–100）。包里有哪些样本由 `msime_client_key_sound_pack` 回答，它给的是 client-services 校验过的绝对路径；这里只决定哪一下按键播哪一个。
enum KeySoundPackPolicy {
  /// 内置的「清脆键盘」，client-core 的 `DEFAULT_SOUND_PACK`。
  static let defaultPack = "default"
  /// client-core `KeySoundPreferences` 的默认音量。
  static let defaultVolume = 50
  /// 随包内置的音效包目录名，`stage-resources.sh` 把它放进键盘扩展。
  static let directoryName = "sound-packs"

  /// 偏好里选中的包；没有这一项、不是字符串或是空字符串时按 `default`，与 Android 的 `packFor` 相同。
  static func selectedPack(in preferences: [String: Any]?) -> String {
    let pack = keySound(in: preferences)?["pack"] as? String ?? ""
    return pack.isEmpty ? defaultPack : pack
  }

  /// 偏好里的音量：0–100 的 JSON 整数，别的值（缺失、布尔、小数、越界）按默认的 50。
  static func volume(in preferences: [String: Any]?) -> Int {
    guard let volume = SharedNumber.strictInt(keySound(in: preferences)?["volume"]), (0...100).contains(volume) else {
      return defaultVolume
    }
    return volume
  }

  /// 播放器的音量，0–1：按振幅缩放，50 是一半，与 HarmonyOS 的 `AudioVolumePolicy.gain` 相同。
  static func gain(volume: Int) -> Float {
    Float(SharedNumber.clamped(volume, to: 0...100)) / 100
  }

  /// 把选中的包写成 `id`；`plugins` 和 `plugins.key_sound` 里的其他键（开关、模式、音量、辅助码包等）原样保留。
  static func choose(_ id: String, document: inout [String: Any]) {
    var plugins = document["plugins"] as? [String: Any] ?? [:]
    var keySound = plugins["key_sound"] as? [String: Any] ?? [:]
    keySound["pack"] = id
    plugins["key_sound"] = keySound
    document["plugins"] = plugins
  }

  private static func keySound(in preferences: [String: Any]?) -> [String: Any]? {
    (preferences?["plugins"] as? [String: Any])?["key_sound"] as? [String: Any]
  }

  // MARK: - 列表

  /// 音效列表里的一项。
  struct Pack: Equatable, Identifiable {
    let id: String
    let name: String
    /// 选中的包不在本机的目录里（例如从桌面同步来的旋律包或别处装的包）：列出来让用户看得见当前选择，键盘这时退回系统按键音。
    var missing = false
  }

  /// `msime_client_plugins` 的 `catalog` 里能当按键音播放的包：`kind = "sound"` 且 `mode = "keys"`，按目录给出的顺序（client-services 把内置包排在前面）。旋律包要逐音变调，iOS 不播，不列。没有名字的包用 id 显示；读不懂的 value 得到空列表。
  static func packs(inCatalog value: Any) -> [Pack] {
    guard let packages = (value as? [String: Any])?["packages"] as? [[String: Any]] else { return [] }
    return packages.compactMap { package in
      guard package["kind"] as? String == "sound", package["mode"] as? String == "keys",
            let id = package["id"] as? String, !id.isEmpty else { return nil }
      let name = package["name"] as? String ?? ""
      return Pack(id: id, name: name.isEmpty ? id : name)
    }
  }

  /// 列表要显示的各项：选中的包已经不在 `packs` 里时在末尾补一项「id（本机没有）」。
  static func options(_ packs: [Pack], selected: String) -> [Pack] {
    guard !packs.contains(where: { $0.id == selected }) else { return packs }
    return packs + [Pack(id: selected, name: "\(selected)（本机没有）", missing: true)]
  }

  // MARK: - 样本

  /// 一个按键音包里四类按键的样本。由 `msime_client_key_sound_pack` 的 value 读出：只接受 `mode = "keys"` 且有普通键样本的包，空格、回车和退格没有自己的样本时用普通键的。
  struct Samples: Equatable {
    let pack: String
    let standard: URL
    let space: URL?
    let enter: URL?
    let backspace: URL?

    init(pack: String, standard: URL, space: URL? = nil, enter: URL? = nil, backspace: URL? = nil) {
      self.pack = pack
      self.standard = standard
      self.space = space
      self.enter = enter
      self.backspace = backspace
    }

    /// `value` 是 `msime_client_key_sound_pack` 应答里的 value；不是按键音包、没有普通键样本或路径不是绝对路径时为 nil。
    init?(pack: String, value: Any) {
      guard let value = value as? [String: Any], value["mode"] as? String == "keys",
            let sounds = value["sounds"] as? [String: Any],
            let standard = Self.file(sounds["default"]) else { return nil }
      self.init(pack: pack, standard: standard, space: Self.file(sounds["space"]),
                enter: Self.file(sounds["enter"]), backspace: Self.file(sounds["backspace"]))
    }

    /// 这一类按键播哪个样本。密码框里每个键都播普通键的声音：空格、回车和删除的声音不同，会让旁人听出密码里按了哪些键（与 HarmonyOS 的 `touchKeyClass` 相同）。
    func sample(for keyClass: KeySoundClass, secure: Bool) -> URL {
      if secure { return standard }
      switch keyClass {
      case .standard: return standard
      case .space: return space ?? standard
      case .enter: return enter ?? standard
      case .backspace: return backspace ?? standard
      }
    }

    /// 要加载的文件，每个只出现一次，按普通键、空格、回车、退格的顺序。
    var files: [URL] {
      var seen: [URL] = []
      for keyClass in KeySoundClass.allCases {
        let file = sample(for: keyClass, secure: false)
        if !seen.contains(file) { seen.append(file) }
      }
      return seen
    }

    private static func file(_ value: Any?) -> URL? {
      guard let path = value as? String, path.hasPrefix("/") else { return nil }
      return URL(fileURLWithPath: path)
    }
  }

  // MARK: - 播放

  /// 一下按键怎么出声。
  enum Playback: Equatable {
    /// 按键音关着。
    case silent
    /// 系统的按键声（`UIDevice.playInputClick`）：没有完全访问、包还没加载好或加载失败时的兜底。
    case system
    /// 包里的这个样本。
    case sample(URL)
  }

  /// 键盘该加载哪个包：按键音开着而且有完全访问时是选中的包，否则为 nil（不加载，已加载的放掉）。没有完全访问时系统不让键盘扩展播放自己的声音。
  static func wantedPack(soundEnabled: Bool, fullAccess: Bool, preferences: [String: Any]?) -> String? {
    soundEnabled && fullAccess ? selectedPack(in: preferences) : nil
  }

  /// 这一下按键的声音。`loaded` 是键盘已经加载好的样本，`wanted` 是 `wantedPack` 的结果：两者是同一个包时播包里的样本，否则（没有完全访问、还在加载、加载失败或刚换了包）退回系统按键声。
  static func playback(soundEnabled: Bool, wanted: String?, loaded: Samples?, keyClass: KeySoundClass,
                       secure: Bool) -> Playback {
    guard soundEnabled else { return .silent }
    guard let wanted, let loaded, loaded.pack == wanted else { return .system }
    return .sample(loaded.sample(for: keyClass, secure: secure))
  }

  // MARK: - 位置

  /// 内置音效包的目录：键盘扩展和它的测试宿主自己带着（`stage-resources.sh` 暂存的 `sound-packs/`）；设置 App 自己不带，读它内嵌的键盘扩展里的那一份。都没有时为 nil，这时 `msime_client_plugins` 不列内置包、`msime_client_key_sound_pack` 找不到内置包。
  static func bundledDirectory(in bundle: Bundle = .main) -> URL? {
    let fm = FileManager.default
    let own = bundle.resourceURL?.appendingPathComponent(directoryName, isDirectory: true)
    let plugins = bundle.builtInPlugInsURL.flatMap {
      try? fm.contentsOfDirectory(at: $0, includingPropertiesForKeys: nil)
    }?.filter { $0.pathExtension == "appex" }
      .map { $0.appendingPathComponent(directoryName, isDirectory: true) } ?? []
    return ([own].compactMap { $0 } + plugins).first {
      fm.isReadableFile(atPath: $0.appendingPathComponent("\(defaultPack)/plugin.toml").path)
    }
  }
}
