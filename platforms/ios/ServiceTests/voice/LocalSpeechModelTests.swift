import XCTest

final class LocalSpeechModelTests: XCTestCase {
  private var scratch: URL!

  override func setUpWithError() throws {
    scratch = FileManager.default.temporaryDirectory.appendingPathComponent("local-speech-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: scratch)
  }

  // The same cases as shared/voice/tests/local_asr.cpp, so the phone tidies a transcript exactly as the desktops do.
  func testTidyMatchesTheSharedRecognizer() {
    XCTAssertEqual(LocalSpeechText.tidy("我们今天下午要 review 一下这个 P R， 然后把 C I 的 pipeline 修好"),
                   "我们今天下午要 review 一下这个 PR，然后把 CI 的 pipeline 修好")
    XCTAssertEqual(LocalSpeechText.tidy("  hello  world "), "hello world")
    XCTAssertEqual(LocalSpeechText.tidy("I am a B student"), "I am a B student")
    XCTAssertEqual(LocalSpeechText.tidy("A B C"), "ABC")
    XCTAssertEqual(LocalSpeechText.tidy("最后 deploy 到 staging 环境 。"), "最后 deploy 到 staging 环境。")
    XCTAssertEqual(LocalSpeechText.tidy(""), "")
    XCTAssertEqual(LocalSpeechText.tidy("我们去 café."), "我们去 café.")
    XCTAssertEqual(LocalSpeechText.tidy("𠮷野家."), "𠮷野家。")
    XCTAssertEqual(LocalSpeechText.tidy("昨天是 monday.Today is 礼拜二。The day after tomorrow 是星期三."),
                   "昨天是 monday. Today is 礼拜二。The day after tomorrow 是星期三。")
    XCTAssertEqual(LocalSpeechText.tidy("真的吗?好的!那就这样,走吧"), "真的吗？好的！那就这样，走吧")
    XCTAssertEqual(LocalSpeechText.tidy("版本 3.5 和 U.S. 还有 e.g. 不变"), "版本 3.5 和 U.S. 还有 e.g. 不变")
    XCTAssertEqual(LocalSpeechText.tidy("Done.Next step"), "Done. Next step")
  }

  func testSegmentsJoinWithASpaceOnlyBetweenLatinWords() {
    XCTAssertEqual(LocalSpeechText.joinSegments(["hello", "world"]), "hello world")
    XCTAssertEqual(LocalSpeechText.joinSegments(["你好", "世界"]), "你好世界")
    XCTAssertEqual(LocalSpeechText.joinSegments(["部署到 staging", "然后"]), "部署到 staging然后")
    XCTAssertEqual(LocalSpeechText.joinSegments(["hello.", "world"]), "hello.world")
    XCTAssertEqual(LocalSpeechText.joinSegments(["", "abc", "", "123"]), "abc 123")
  }

  func testTransducerHotwordsDropWordsTheModelCannotSpell() {
    let tokens = LocalSpeechText.tokenSet("<blk> 0\n水 1\n杉 2\n输 3\n入 4\n法 5\n")
    XCTAssertEqual(tokens, ["<blk>", "水", "杉", "输", "入", "法"])
    XCTAssertFalse(LocalSpeechText.cjkSpelledAsWords(tokens))
    let words = LocalSpeechText.transducerHotwords(["水杉", "输入法", "鹅", "Open AI", "C++", "rock'n-roll", "a/b", "  "], tokens: tokens,
                                                   spacedCJK: false)
    XCTAssertEqual(words, "水杉\n输入法\nOpen AI\nrock'n-roll\na b\n")
    let many = (0..<250).map { _ in "水杉" }
    XCTAssertEqual(LocalSpeechText.transducerHotwords(many, tokens: tokens, spacedCJK: false).split(separator: "\n").count, 200)
  }

  // X-ASR 的 token 表里每个汉字都是带 `▁` 的独立的词，与 shared/voice/LocalAsr.cpp 的 cjk_spelled_as_words 和 transducer_hotwords 的 spaced_cjk 分支一致。
  func testHotwordsForTokensThatSpellEveryHanziAsAWord() {
    let tokens = LocalSpeechText.tokenSet("<blk> 0\n<sos/eos> 1\n▁, 2\n▁写 3\n▁上 4\n▁去 5\n▁现 6\n▁在 7\n▁时 8\n▁fr 9\nequ 10\n")
    XCTAssertTrue(LocalSpeechText.cjkSpelledAsWords(tokens))
    XCTAssertFalse(LocalSpeechText.cjkSpelledAsWords(tokens.union(["写"])), "one bare hanzi token means the model spells hanzi as characters")
    XCTAssertFalse(LocalSpeechText.cjkSpelledAsWords(["<blk>", "▁fr", "equ", "▁a"]), "no hanzi at all")
    XCTAssertFalse(LocalSpeechText.cjkSpelledAsWords(["<blk>", "▁the", "▁and"]), "English pieces as long as `▁` plus a hanzi are not hanzi")
    XCTAssertFalse(LocalSpeechText.cjkSpelledAsWords(["<blk>", "▁，", "▁。"]), "`▁` plus CJK punctuation is not a hanzi")
    XCTAssertFalse(LocalSpeechText.cjkSpelledAsWords([]))
    let words = LocalSpeechText.transducerHotwords(["写上去", "现在时", "Open AI", "写A", "Open 写", "写 上", "鹅", "现在鹅", "  "],
                                                   tokens: tokens, spacedCJK: true)
    XCTAssertEqual(words, "写 上 去\n现 在 时\nOpen AI\n写 A\nOpen 写\n写 上\n")
    XCTAssertEqual(LocalSpeechText.transducerHotwords(["写上去"], tokens: tokens, spacedCJK: false), "",
                   "spelled as characters, the hanzi are not in the token set")
  }

  func testFunAsrHotwordsAreCommaSeparatedAndCapped() {
    XCTAssertEqual(LocalSpeechText.funAsrHotwords(["水杉", "", "a,b", "输入法"]), "水杉,输入法")
    XCTAssertEqual(LocalSpeechText.funAsrHotwords((0..<40).map(String.init)).split(separator: ",").count, 30)
  }

  func testSenseVoicePinsOnlyTheLanguagesItWouldMisjudge() {
    XCTAssertEqual(LocalSpeechText.senseVoiceLanguage("zh-cn"), "auto")
    XCTAssertEqual(LocalSpeechText.senseVoiceLanguage("en-us"), "auto")
    XCTAssertEqual(LocalSpeechText.senseVoiceLanguage("auto"), "auto")
    XCTAssertEqual(LocalSpeechText.senseVoiceLanguage("zh-HK"), "yue")
    XCTAssertEqual(LocalSpeechText.senseVoiceLanguage("yue"), "yue")
    XCTAssertEqual(LocalSpeechText.senseVoiceLanguage("ja-JP"), "ja")
    XCTAssertEqual(LocalSpeechText.senseVoiceLanguage("ko"), "ko")
    XCTAssertTrue((1...4).contains(LocalSpeechText.threadCount))
  }

  func testManifestNamesExistingFilesAndRejectsBrokenOnes() throws {
    let model = try makeModel(id: "sense-voice", manifest: [
      "kind": "offline_sense_voice", "hotwords": "pinyin",
      "files": ["model": "model.int8.onnx", "tokens": "tokens.txt", "vad": "silero_vad.onnx"],
    ], files: ["model.int8.onnx", "tokens.txt"])
    let manifest = try LocalSpeechModelManifest(directory: model)
    XCTAssertEqual(manifest.kind, .offlineSenseVoice)
    XCTAssertEqual(manifest.hotwords, "pinyin")
    XCTAssertEqual(try manifest.file("tokens"), model.appendingPathComponent("tokens.txt").path)
    XCTAssertThrowsError(try manifest.file("vad"), "a named file that is missing on disk")
    XCTAssertThrowsError(try manifest.file("encoder"), "a role the manifest does not name")
    XCTAssertNil(try manifest.optionalFile("bpe_vocab"))

    XCTAssertEqual(manifest.modelType, "", "no model_type leaves sherpa to probe the encoder")

    let offline = try makeModel(id: "x-asr-zh-en-offline", manifest: [
      "kind": "offline_transducer", "hotwords": "native", "modeling_unit": "cjkchar+bpe", "model_type": "transducer",
      "files": ["encoder": "encoder.onnx", "decoder": "decoder.onnx", "joiner": "joiner.onnx", "tokens": "tokens.txt",
                "bpe_vocab": "bpe.vocab", "vad": "silero_vad.onnx"],
    ], files: ["encoder.onnx", "decoder.onnx", "joiner.onnx", "tokens.txt", "bpe.vocab", "silero_vad.onnx"])
    let transducer = try LocalSpeechModelManifest(directory: offline)
    XCTAssertEqual(transducer.kind, .offlineTransducer)
    XCTAssertEqual(transducer.kind.rawValue, "offline_transducer")
    XCTAssertEqual(transducer.modelType, "transducer")
    XCTAssertEqual(transducer.modelingUnit, "cjkchar+bpe")
    XCTAssertEqual(transducer.hotwords, "native")
    XCTAssertEqual(try transducer.optionalFile("bpe_vocab"), offline.appendingPathComponent("bpe.vocab").path)

    let unknown = try makeModel(id: "whisper", manifest: ["kind": "offline_whisper", "files": [:]], files: [])
    XCTAssertThrowsError(try LocalSpeechModelManifest(directory: unknown))
    XCTAssertThrowsError(try LocalSpeechModelManifest(directory: scratch.appendingPathComponent("missing"))) { error in
      XCTAssertEqual((error as? ServiceFailure)?.message, "所选目录不是已安装的本地语音模型。")
    }
  }

  func testManifestRejectsOversizedFiles() throws {
    let model = try makeModel(id: "oversized", manifest: [
      "kind": "online_transducer",
      "files": [:],
      "padding": String(repeating: "x", count: 256 * 1024),
    ], files: [])
    XCTAssertThrowsError(try LocalSpeechModelManifest(directory: model)) { error in
      XCTAssertEqual((error as? ServiceFailure)?.message, "本地语音模型的描述文件已损坏，请删除后重新下载。")
    }
  }

  func testTokenVocabularyReadIsBounded() throws {
    let model = try makeModel(id: "large-tokens", manifest: [
      "kind": "online_transducer",
      "files": ["tokens": "tokens.txt"],
    ], files: ["tokens.txt"])
    try Data(repeating: 0x41, count: LocalSpeechModelManifest.maximumTokensBytes + 1)
      .write(to: model.appendingPathComponent("tokens.txt"))

    XCTAssertThrowsError(try LocalSpeechModelManifest(directory: model)
      .textFile("tokens", maximumBytes: LocalSpeechModelManifest.maximumTokensBytes))
  }

  func testManifestRejectsFilesOutsideModelDirectory() throws {
    let model = try makeModel(id: "escape", manifest: [
      "kind": "offline_sense_voice",
      "files": ["model": "../secret.txt"],
    ], files: [])
    let secret = model.deletingLastPathComponent().appendingPathComponent("secret.txt")
    try Data([0]).write(to: secret)
    XCTAssertThrowsError(try LocalSpeechModelManifest(directory: model).file("model"))

    let symlinkModel = try makeModel(id: "symlink", manifest: [
      "kind": "offline_sense_voice",
      "files": ["model": "linked.txt"],
    ], files: [])
    try FileManager.default.createSymbolicLink(
      at: symlinkModel.appendingPathComponent("linked.txt"), withDestinationURL: secret)
    XCTAssertThrowsError(try LocalSpeechModelManifest(directory: symlinkModel).file("model"))
  }

  func testManifestRejectsASymlinkedManifest() throws {
    let model = try makeModel(id: "manifest-link", manifest: [
      "kind": "online_transducer", "files": [:],
    ], files: [])
    let outside = try makeModel(id: "manifest-outside", manifest: [
      "kind": "online_transducer", "files": [:],
    ], files: [])
    let manifest = model.appendingPathComponent(LocalSpeechModelManifest.fileName)
    try FileManager.default.removeItem(at: manifest)
    try FileManager.default.createSymbolicLink(at: manifest,
                                               withDestinationURL: outside.appendingPathComponent(LocalSpeechModelManifest.fileName))

    XCTAssertThrowsError(try LocalSpeechModelManifest(directory: model))
    XCTAssertFalse(LocalSpeechModelManifest.isModelDirectory(model))
  }

  func testStoredPathFollowsTheModelIntoAMovedContainer() throws {
    let root = scratch.appendingPathComponent("voice-models", isDirectory: true)
    let model = try makeModel(id: "zipformer", manifest: ["kind": "online_transducer", "files": [:]], files: [], root: root)
    XCTAssertEqual(LocalSpeechModelLocation.resolve(storedPath: model.path, root: root)?.standardizedFileURL, model.standardizedFileURL)
    // Where the same model lived before an app update moved the container.
    let old = "/private/var/mobile/Containers/Data/Application/OLD/Library/Application Support/voice-models/zipformer"
    XCTAssertEqual(LocalSpeechModelLocation.resolve(storedPath: old, root: root)?.standardizedFileURL, model.standardizedFileURL)
    XCTAssertNil(LocalSpeechModelLocation.resolve(storedPath: old, root: nil))
    XCTAssertNil(LocalSpeechModelLocation.resolve(storedPath: "zipformer", root: root))
    XCTAssertNil(LocalSpeechModelLocation.resolve(storedPath: "", root: root))
    XCTAssertNil(LocalSpeechModelLocation.resolve(storedPath: root.appendingPathComponent("gone").path, root: root))
    XCTAssertTrue(LocalSpeechModelLocation.names(old, model: "zipformer"))
    XCTAssertFalse(LocalSpeechModelLocation.names(old, model: "sense-voice"))
    XCTAssertFalse(LocalSpeechModelLocation.names("zipformer", model: "zipformer"))
    XCTAssertFalse(LocalSpeechModelLocation.names(" ", model: "zipformer"))
  }

  func testStoredPathCannotSelectAModelOutsideTheManagedRoot() throws {
    let root = scratch.appendingPathComponent("voice-models", isDirectory: true)
    let outside = try makeModel(id: "outside", manifest: ["kind": "online_transducer", "files": [:]], files: [])
    XCTAssertNil(LocalSpeechModelLocation.resolve(storedPath: outside.path, root: root))
  }

  func testMovedModelCannotEscapeManagedRootThroughASymlink() throws {
    let root = scratch.appendingPathComponent("voice-models", isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let outside = try makeModel(id: "zipformer", manifest: ["kind": "online_transducer", "files": [:]], files: [])
    let link = root.appendingPathComponent("zipformer", isDirectory: true)
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: outside)

    let old = "/private/var/mobile/Containers/Data/Application/OLD/Library/Application Support/voice-models/zipformer"
    XCTAssertNil(LocalSpeechModelLocation.resolve(storedPath: old, root: root))
  }

  func testManagedRootSymlinkCannotExposeAnExternalModel() throws {
    let root = scratch.appendingPathComponent("voice-models", isDirectory: true)
    let outside = try makeModel(id: "zipformer", manifest: ["kind": "online_transducer", "files": [:]], files: [])
    try FileManager.default.createSymbolicLink(at: root, withDestinationURL: outside.deletingLastPathComponent())

    XCTAssertNil(LocalSpeechModelLocation.resolve(storedPath: root.appendingPathComponent("zipformer").path,
                                                  root: root))
  }

  func testCatalogEntriesDecodeWithDefaultsForMissingFields() throws {
    let json = #"{"models":[{"id":"a","title":"A","streaming":true,"default":true,"memory":314572800,"archive_size":10,"installed_size":20,"license_spdx":"Apache-2.0"},{"id":"b","desktop_only":true,"installed":true,"path":"/x/b"}],"default":"a"}"#
    let catalog = try JSONDecoder().decode(LocalSpeechModelStore.Catalog.self, from: Data(json.utf8))
    XCTAssertEqual(catalog.default, "a")
    XCTAssertEqual(catalog.models.map(\.id), ["a", "b"])
    XCTAssertTrue(catalog.models[0].streaming && catalog.models[0].isDefault && !catalog.models[0].installed)
    XCTAssertEqual(catalog.models[0].memory, 314_572_800)
    XCTAssertEqual(catalog.models[0].licenseSPDX, "Apache-2.0")
    XCTAssertEqual(catalog.models[1].title, "b")
    XCTAssertTrue(catalog.models[1].desktopOnly && catalog.models[1].installed)
    XCTAssertEqual(catalog.models[1].path, "/x/b")
  }

  /// The real host-api: the shared catalog through the same FFI the settings page uses.
  func testHostCatalogListsTheSharedModelsAndNothingInstalled() throws {
    let root = scratch.appendingPathComponent("voice-models", isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let catalog = try LocalSpeechModelStore.catalog(root: root)
    XCTAssertFalse(catalog.models.isEmpty)
    XCTAssertTrue(catalog.models.contains { $0.id == catalog.default })
    XCTAssertFalse(catalog.models.contains(where: \.installed))
    XCTAssertTrue(catalog.models.contains { !$0.desktopOnly }, "the phone has at least one model it can run")
    let offline = try XCTUnwrap(catalog.models.first { $0.id == "x-asr-zh-en-offline" }, "the offline X-ASR transducer is listed")
    XCTAssertFalse(offline.desktopOnly)
    XCTAssertFalse(offline.streaming)
    XCTAssertEqual(offline.hotwords, "native")
    for model in catalog.models {
      XCTAssertFalse(model.title.isEmpty)
      XCTAssertGreaterThan(model.archiveSize, 0)
      XCTAssertGreaterThan(model.memory, 0)
    }
  }

  func testHostFailuresReadAsMessagesAndHotwordHelpersNeverFail() {
    XCTAssertEqual(LocalSpeechModelStore.message(for: "local_model_network: timed out"),
                   LocalSpeechModelStore.message(for: "local_model_network"))
    XCTAssertTrue(LocalSpeechModelStore.message(for: "local_model_something_new").contains("local_model_something_new"))
    XCTAssertEqual(LocalSpeechModelStore.correct("你好", hotwords: []), "你好")
    XCTAssertEqual(LocalSpeechModelStore.hotwords(resources: scratch.appendingPathComponent("missing"), stateRoot: scratch), [])
  }

  /// 走真实的 host-api：transducer 不自带 ITN，最终文本里的汉字数字改成数字；自己会写数字的模型原样保留。
  func testNumberNormalizationOnlyRewritesTransducerTranscripts() {
    XCTAssertEqual(LocalSpeechModelStore.normalizeNumbers("二零零二年收窄了百分之五十七，一夜之间", kind: "offline_transducer"),
                   "2002年收窄了57%，一夜之间")
    XCTAssertEqual(LocalSpeechModelStore.normalizeNumbers("下午三点十分出发", kind: "online_transducer"), "下午3点10分出发")
    XCTAssertEqual(LocalSpeechModelStore.normalizeNumbers("二零零二年", kind: "offline_sense_voice"), "二零零二年")
    XCTAssertEqual(LocalSpeechModelStore.normalizeNumbers("", kind: "offline_transducer"), "")
    // 共享层拒绝的请求（文本超长）保留原文。
    let tooLong = String(repeating: "一", count: 10_001)
    XCTAssertEqual(LocalSpeechModelStore.normalizeNumbers(tooLong, kind: "offline_transducer"), tooLong)
  }

  func testMirrorRejectsCredentialsQueriesAndFragments() {
    XCTAssertTrue(LocalSpeechModelStore.isValidMirror("https://mirror.example.test/gh/"))
    for value in [
      "https://user:pass@mirror.example.test",
      "https://mirror.example.test?query=unexpected",
      "https://mirror.example.test/#fragment",
      "https:///path",
    ] {
      XCTAssertFalse(LocalSpeechModelStore.isValidMirror(value), value)
    }
  }

  @discardableResult
  private func makeModel(id: String, manifest: [String: Any], files: [String], root: URL? = nil) throws -> URL {
    let directory = (root ?? scratch).appendingPathComponent(id, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try JSONSerialization.data(withJSONObject: manifest).write(to: directory.appendingPathComponent(LocalSpeechModelManifest.fileName))
    for file in files { try Data([0]).write(to: directory.appendingPathComponent(file)) }
    return directory
  }
}
