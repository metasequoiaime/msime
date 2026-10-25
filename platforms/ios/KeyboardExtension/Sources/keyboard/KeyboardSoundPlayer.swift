import AVFoundation

/// 键盘扩展自己播放的按键音效包（`plugins.key_sound.pack`），对应 Android 的 `ImeKeyFeedback` 和 HarmonyOS 的 `KeySoundPlayer`。哪一下按键播哪个样本由 `KeySoundPackPolicy` 决定，这里只管加载和出声。
///
/// 包经 `msime_client_key_sound_pack` 在后台队列上读出，每个样本建几只 `AVAudioPlayer` 并 `prepareToPlay`，按键时主线程只挑一只从头重放，不碰磁盘，也不在按键路径上建任何东西。音频会话是 `.ambient`：和别的应用正在放的音乐混音、不打断它，并跟随静音开关，与系统按键声一样在静音模式下不响。
///
/// 只在按键音开着且有完全访问时加载（`KeySoundPackPolicy.wantedPack`）：没有完全访问时 iOS 不让键盘扩展播放自己的声音。还没加载好、加载失败时调用方退回 `UIDevice.playInputClick`。
@MainActor
final class KeyboardSoundPlayer {
  /// 每个样本的播放器数：快速连打时上一声还没放完，下一声用另一只，不把它从头掐断。
  static let voicesPerSample = 3

  /// 已经加载好的样本；没有时为 nil。
  private(set) var loaded: KeySoundPackPolicy.Samples?
  private var voices: [URL: [AVAudioPlayer]] = [:]
  private var cursor: [URL: Int] = [:]
  /// 正在加载或已经加载的包；nil 表示什么也不要。
  private var target: String?
  /// 每次换目标都前进，被新的选择赶超的那次加载回来后直接丢弃。
  private var generation: UInt64 = 0
  private let queue = DispatchQueue(label: "app.msime.ios.key-sound", qos: .userInitiated)
  private let resolve: @Sendable (String) -> Result<Any, MetasequoiaInputSessionBridge.PluginFailure>

  /// `resolve` 只给测试替换；键盘用共享状态目录和随包的 `sound-packs/`。
  init(resolve: @escaping @Sendable (String) -> Result<Any, MetasequoiaInputSessionBridge.PluginFailure> = {
    MetasequoiaInputSessionBridge.keySoundPack($0)
  }) {
    self.resolve = resolve
  }

  /// 要 `pack` 的样本，nil 时放掉已加载的。同一个包已经加载或正在加载时什么也不做，所以键盘每次出现、每次重读偏好后都可以调用。`completion` 只给测试等加载结束，在主线程调用。
  func want(_ pack: String?, completion: (@MainActor () -> Void)? = nil) {
    guard pack != target else { completion?(); return }
    guard let pack else {
      release()
      completion?()
      return
    }
    target = pack
    generation &+= 1
    guard loaded?.pack != pack else { completion?(); return }
    let generation = generation
    let resolve = resolve
    queue.async { [weak self] in
      let prepared = Prepared.load(pack, resolve: resolve)
      DispatchQueue.main.async {
        MainActor.assumeIsolated {
          defer { completion?() }
          guard let self, self.generation == generation else { return }
          self.install(prepared, pack: pack)
        }
      }
    }
  }

  /// 播放一个样本，`file` 来自 `KeySoundPackPolicy.playback`。这个样本没能加载时用普通键的；什么都没有或播放失败时返回 false，调用方退回系统按键声。
  func play(_ file: URL, gain: Float) -> Bool {
    guard let loaded, let players = voices[file] ?? voices[loaded.standard], !players.isEmpty else { return false }
    let index = cursor[file, default: 0] % players.count
    cursor[file] = index + 1
    let player = players[index]
    player.volume = gain
    player.currentTime = 0
    return player.play()
  }

  /// 放掉播放器，还在路上的加载回来后也丢掉；下一次 `want` 重新加载。收到内存警告时调用。
  func release() {
    target = nil
    generation &+= 1
    loaded = nil
    voices = [:]
    cursor = [:]
  }

  private func install(_ prepared: Prepared, pack: String) {
    switch prepared {
    case .ready(let samples, let players):
      loaded = samples
      voices = players.voices
      cursor = [:]
      DiagnosticLog.shared.write("key_sound_loaded pack=\(pack) samples=\(players.voices.count)")
    case .failed(let reason):
      // 留着上一个包的样本也播不出来（`playback` 只认选中的包），一起放掉；清掉目标，下一次 `want` 再试。
      loaded = nil
      voices = [:]
      cursor = [:]
      target = nil
      DiagnosticLog.shared.write("key_sound_unavailable pack=\(pack) reason=\(reason)")
    }
  }

  /// 后台队列上建好、交给主线程的播放器。`AVAudioPlayer` 不是 Sendable，但建好之后只在主线程上用。
  private final class Players: @unchecked Sendable {
    let voices: [URL: [AVAudioPlayer]]
    init(_ voices: [URL: [AVAudioPlayer]]) { self.voices = voices }
  }

  private enum Prepared: @unchecked Sendable {
    case ready(KeySoundPackPolicy.Samples, Players)
    case failed(String)

    /// 读包、配音频会话、建播放器。读文件，只在后台队列上调用。
    static func load(_ pack: String,
                     resolve: (String) -> Result<Any, MetasequoiaInputSessionBridge.PluginFailure>) -> Prepared {
      let value: Any
      switch resolve(pack) {
      case .success(let resolved): value = resolved
      case .failure(let failure): return .failed(failure.code.isEmpty ? "unavailable" : failure.code)
      }
      guard let samples = KeySoundPackPolicy.Samples(pack: pack, value: value) else { return .failed("not_keys") }
      do {
        try AVAudioSession.sharedInstance().setCategory(.ambient)
      } catch {
        return .failed("audio_session")
      }
      var voices: [URL: [AVAudioPlayer]] = [:]
      for file in samples.files {
        let players = (0..<KeyboardSoundPlayer.voicesPerSample).compactMap { _ -> AVAudioPlayer? in
          guard let player = try? AVAudioPlayer(contentsOf: file) else { return nil }
          player.prepareToPlay()
          return player
        }
        if !players.isEmpty { voices[file] = players }
      }
      guard voices[samples.standard] != nil else { return .failed("sample") }
      return .ready(samples, Players(voices))
    }
  }
}
