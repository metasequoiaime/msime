import XCTest

/// 按键音效包的选择、按键类别与退回规则（`KeySoundPackPolicy`），与 Android 的 `ImeKeyFeedback`、HarmonyOS 的 `KeySoundPolicy` 相同；后面几项对着测试宿主随包的 `sound-packs/` 跑真的 `msime_client_plugins` 和 `msime_client_key_sound_pack`。
@MainActor
final class KeySoundPackPolicyTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-key-sound-\(UUID().uuidString)", isDirectory: true)
    XCTAssertNoThrow(try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true))
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// 没选、空字符串或不是字符串时都是内置的「清脆键盘」。
  func testSelectedPackFallsBackToTheDefaultPack() {
    XCTAssertEqual(KeySoundPackPolicy.selectedPack(in: nil), "default")
    XCTAssertEqual(KeySoundPackPolicy.selectedPack(in: ["plugins": ["key_sound": ["pack": ""]]]), "default")
    XCTAssertEqual(KeySoundPackPolicy.selectedPack(in: ["plugins": ["key_sound": ["pack": 3]]]), "default")
    XCTAssertEqual(KeySoundPackPolicy.selectedPack(in: ["plugins": ["key_sound": ["pack": "msime-bubble"]]]), "msime-bubble")
  }

  /// 音量只认 0–100 的整数，其余按 client-core 的默认 50；播放器音量按振幅线性缩放。
  func testVolumeIsAStrictPercentage() {
    let volume = { (value: Any) in KeySoundPackPolicy.volume(in: ["plugins": ["key_sound": ["volume": value]]]) }
    XCTAssertEqual(KeySoundPackPolicy.volume(in: nil), 50)
    XCTAssertEqual(volume(70), 70)
    XCTAssertEqual(volume(0), 0)
    XCTAssertEqual(volume(true), 50)
    XCTAssertEqual(volume(30.5), 50)
    XCTAssertEqual(volume(101), 50)
    XCTAssertEqual(volume("80"), 50)
    XCTAssertEqual(KeySoundPackPolicy.gain(volume: 50), 0.5)
    XCTAssertEqual(KeySoundPackPolicy.gain(volume: 100), 1)
  }

  /// 选包只改 `key_sound.pack`：开关、音量和 `plugins` 里别的选择原样保留。
  func testChoosingAPackKeepsTheRestOfThePlugins() {
    var document: [String: Any] = [
      "plugins": ["key_sound": ["enabled": true, "volume": 70, "pack": "default"], "helpcode_pack_quanpin": "custom-mine"],
    ]
    KeySoundPackPolicy.choose("msime-typewriter", document: &document)
    let plugins = document["plugins"] as? [String: Any]
    let keySound = plugins?["key_sound"] as? [String: Any]
    XCTAssertEqual(keySound?["pack"] as? String, "msime-typewriter")
    XCTAssertEqual(keySound?["volume"] as? Int, 70)
    XCTAssertEqual(keySound?["enabled"] as? Bool, true)
    XCTAssertEqual(plugins?["helpcode_pack_quanpin"] as? String, "custom-mine")

    var empty: [String: Any] = [:]
    KeySoundPackPolicy.choose("msime-8bit", document: &empty)
    XCTAssertEqual(KeySoundPackPolicy.selectedPack(in: empty), "msime-8bit")
  }

  /// 列表只要按键音包，按目录顺序；选中的包不在本机时补在末尾，让用户看得见。
  func testListOffersKeyPacksInCatalogOrder() {
    let catalog: [String: Any] = ["packages": [
      ["kind": "sound", "mode": "keys", "id": "default", "name": "清脆键盘", "builtin": true],
      ["kind": "sound", "mode": "sequence", "id": "twinkle", "name": "小星星", "builtin": true],
      ["kind": "music", "id": "msime-music-lofi", "name": "Lo-fi 午后", "builtin": true],
      ["kind": "helpcode", "id": "custom-mine", "name": "我的码表"],
      ["kind": "sound", "mode": "keys", "id": "rain", "name": ""],
    ]]
    let packs = KeySoundPackPolicy.packs(inCatalog: catalog)
    XCTAssertEqual(packs.map(\.id), ["default", "rain"])
    XCTAssertEqual(packs.map(\.name), ["清脆键盘", "rain"], "a pack without a name shows its id")
    XCTAssertEqual(KeySoundPackPolicy.packs(inCatalog: "nonsense"), [])

    XCTAssertEqual(KeySoundPackPolicy.options(packs, selected: "rain"), packs)
    let missing = KeySoundPackPolicy.options(packs, selected: "msime-canon")
    XCTAssertEqual(missing.map(\.id), ["default", "rain", "msime-canon"])
    XCTAssertEqual(missing.last?.name, "msime-canon（本机没有）")
    XCTAssertEqual(missing.last?.missing, true)
  }

  /// 空格、回车、退格没有自己的样本时用普通键的；密码框里每个键都是普通键的声音。
  func testSamplesFallBackToTheStandardKeyAndStayUniformInPasswordFields() throws {
    let value: [String: Any] = ["mode": "keys", "sounds": [
      "default": "/packs/a/key.wav", "space": "/packs/a/space.wav", "enter": NSNull(), "backspace": "relative.wav",
    ]]
    let samples = try XCTUnwrap(KeySoundPackPolicy.Samples(pack: "a", value: value))
    let key = URL(fileURLWithPath: "/packs/a/key.wav")
    let space = URL(fileURLWithPath: "/packs/a/space.wav")
    XCTAssertEqual(samples.sample(for: .standard, secure: false), key)
    XCTAssertEqual(samples.sample(for: .space, secure: false), space)
    XCTAssertEqual(samples.sample(for: .enter, secure: false), key)
    XCTAssertEqual(samples.sample(for: .backspace, secure: false), key, "a relative path is not a validated file")
    for keyClass in KeySoundClass.allCases {
      XCTAssertEqual(samples.sample(for: keyClass, secure: true), key)
    }
    XCTAssertEqual(samples.files, [key, space])

    XCTAssertNil(KeySoundPackPolicy.Samples(pack: "t", value: ["mode": "sequence", "sounds": ["default": "/t.wav"]]))
    XCTAssertNil(KeySoundPackPolicy.Samples(pack: "e", value: ["mode": "keys", "sounds": ["default": NSNull()]]))
  }

  /// 关着不响；没有完全访问、还没加载好或加载的不是选中的包时退回系统按键声；对上了才播包里的样本。
  func testPlaybackFallsBackToTheSystemClick() {
    let samples = KeySoundPackPolicy.Samples(pack: "msime-bubble", standard: URL(fileURLWithPath: "/b/key.wav"),
                                             space: URL(fileURLWithPath: "/b/space.wav"))
    let preferences: [String: Any] = ["plugins": ["key_sound": ["pack": "msime-bubble"]]]
    XCTAssertNil(KeySoundPackPolicy.wantedPack(soundEnabled: true, fullAccess: false, preferences: preferences))
    XCTAssertNil(KeySoundPackPolicy.wantedPack(soundEnabled: false, fullAccess: true, preferences: preferences))
    let wanted = KeySoundPackPolicy.wantedPack(soundEnabled: true, fullAccess: true, preferences: preferences)
    XCTAssertEqual(wanted, "msime-bubble")

    let play = { (enabled: Bool, wanted: String?, loaded: KeySoundPackPolicy.Samples?, secure: Bool) in
      KeySoundPackPolicy.playback(soundEnabled: enabled, wanted: wanted, loaded: loaded, keyClass: .space, secure: secure)
    }
    XCTAssertEqual(play(false, wanted, samples, false), .silent)
    XCTAssertEqual(play(true, nil, samples, false), .system, "no full access")
    XCTAssertEqual(play(true, wanted, nil, false), .system, "still loading or failed")
    XCTAssertEqual(play(true, "default", samples, false), .system, "a newly chosen pack that has not loaded yet")
    XCTAssertEqual(play(true, wanted, samples, false), .sample(URL(fileURLWithPath: "/b/space.wav")))
    XCTAssertEqual(play(true, wanted, samples, true), .sample(URL(fileURLWithPath: "/b/key.wav")))
  }

  /// 随包的 `sound-packs/` 只带按键音包：目录列出的正是这五个，全部是内置的，没有旋律包和音乐包。
  func testBundleShipsExactlyTheKeyPacks() throws {
    let directory = try XCTUnwrap(KeySoundPackPolicy.bundledDirectory(), "stage-resources.sh did not stage sound-packs/")
    let folders = try FileManager.default.contentsOfDirectory(atPath: directory.path).filter { !$0.hasPrefix(".") }
    let expected: Set = ["default", "msime-8bit", "msime-bubble", "msime-typewriter", "msime-woodblock"]
    XCTAssertEqual(Set(folders), expected)

    let catalog = try MetasequoiaInputSessionBridge.plugins(["operation": "catalog"], stateRoot: state).get()
    XCTAssertEqual(Set(KeySoundPackPolicy.packs(inCatalog: catalog).map(\.id)), expected)
    let packages = (catalog as? [String: Any])?["packages"] as? [[String: Any]] ?? []
    XCTAssertTrue(packages.filter { $0["kind"] as? String == "sound" }.allSatisfy { $0["builtin"] as? Bool == true })
  }

  /// 每个内置按键音包都解析得出四类样本，文件都在。
  func testEveryBundledPackResolvesItsSamples() throws {
    for pack in ["default", "msime-8bit", "msime-bubble", "msime-typewriter", "msime-woodblock"] {
      let value = try MetasequoiaInputSessionBridge.keySoundPack(pack, stateRoot: state).get()
      let samples = try XCTUnwrap(KeySoundPackPolicy.Samples(pack: pack, value: value), pack)
      for keyClass in KeySoundClass.allCases {
        XCTAssertTrue(FileManager.default.isReadableFile(atPath: samples.sample(for: keyClass, secure: false).path), pack)
      }
    }
    XCTAssertThrowsError(try MetasequoiaInputSessionBridge.keySoundPack("nothing", stateRoot: state).get())
  }

  /// 键盘的播放器把选中的包加载好；加载不了的包不留样本，下一次 `want` 再试，而同一个包加载好之后不重复加载。
  func testPlayerLoadsOnceAndRetriesAFailure() {
    let calls = Counter()
    let root: URL = state
    let player = KeyboardSoundPlayer { pack in
      calls.increment()
      return MetasequoiaInputSessionBridge.keySoundPack(pack, stateRoot: root)
    }
    want(player, "default")
    XCTAssertEqual(player.loaded?.pack, "default")
    want(player, "default")
    XCTAssertEqual(calls.value, 1)

    want(player, "nothing")
    XCTAssertNil(player.loaded)
    want(player, "nothing")
    XCTAssertEqual(calls.value, 3, "a failed pack is tried again")

    want(player, nil)
    XCTAssertNil(player.loaded)
  }

  private func want(_ player: KeyboardSoundPlayer, _ pack: String?) {
    let done = expectation(description: "load \(pack ?? "nothing")")
    player.want(pack) { done.fulfill() }
    wait(for: [done], timeout: 10)
  }

  private final class Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    var value: Int { lock.withLock { count } }
    func increment() { lock.withLock { count += 1 } }
  }
}
