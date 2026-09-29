import AVFoundation
import AuthenticationServices
import Foundation
import Darwin
import Security
import Tauri
import UIKit
import UniformTypeIdentifiers

private struct SetAppIconArgs: Decodable {
  let style: String
}

private struct SaveAccountSessionArgs: Decodable {
  let value: String
}

private struct AppleSignInArgs: Decodable {
  let challengeId: String
  let nonce: String
}

private struct CopyTextArgs: Decodable {
  let text: String
}

private struct SaveVoiceTextArgs: Decodable {
  let text: String
}

private struct VoiceRequestHeader: Decodable {
  let name: String
  let value: String
}

private struct VoiceTranscriptionArgs: Decodable {
  let requestId: String
  let provider: String
  let endpoint: String
  let model: String
  let token: String
  let headers: [VoiceRequestHeader]
  let enableItn: Bool
  let enablePunctuation: Bool
  let enableDdc: Bool
  let boostingTableId: String
}

private struct VoiceControlArgs: Decodable {
  let requestId: String?
}

private struct PreviewKeyboardHapticsArgs: Decodable {
  let strength: String
}

struct VoicePluginFailure: Error {
  let code: String
}

private final class VoiceTranscriptionTransport: NSObject, URLSessionDataDelegate,
    URLSessionTaskDelegate {
  private static let maximumResponseBytes = 1024 * 1024
  private var session: URLSession?
  private var task: URLSessionDataTask?
  private var response: HTTPURLResponse?
  private var body = Data()
  private var completed = false
  private var completion: ((Result<String, VoicePluginFailure>) -> Void)?

  init(request: URLRequest, completion: @escaping (Result<String, VoicePluginFailure>) -> Void) {
    self.completion = completion
    super.init()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
    configuration.urlCache = nil
    let session = URLSession(configuration: configuration, delegate: self,
      delegateQueue: OperationQueue.main)
    self.session = session
    let task = session.dataTask(with: request)
    self.task = task
    task.resume()
  }

  func cancel() {
    task?.cancel()
    session?.invalidateAndCancel()
    completion = nil
  }

  private func finish(_ result: Result<String, VoicePluginFailure>) {
    guard !completed else { return }
    completed = true
    let completion = completion
    self.completion = nil
    task = nil
    session?.finishTasksAndInvalidate()
    session = nil
    completion?(result)
  }

  func urlSession(_ session: URLSession, dataTask: URLSessionDataTask,
                  didReceive response: URLResponse,
                  completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
    guard let http = response as? HTTPURLResponse,
          (200..<300).contains(http.statusCode),
          response.expectedContentLength <= Int64(Self.maximumResponseBytes) else {
      completionHandler(.cancel)
      finish(.failure(VoicePluginFailure(code: "voice_service")))
      return
    }
    self.response = http
    completionHandler(.allow)
  }

  func urlSession(_ session: URLSession, dataTask: URLSessionDataTask,
                  didReceive data: Data) {
    guard data.count <= Self.maximumResponseBytes,
          body.count <= Self.maximumResponseBytes - data.count else {
      dataTask.cancel()
      finish(.failure(VoicePluginFailure(code: "voice_response")))
      return
    }
    body.append(data)
  }

  func urlSession(_ session: URLSession, task: URLSessionTask,
                  didCompleteWithError error: Error?) {
    guard !completed else { return }
    guard error == nil, response != nil,
          let document = try? JSONSerialization.jsonObject(with: body) as? [String: Any],
          let text = document["text"] as? String,
          text.count <= 10_000,
          !text.unicodeScalars.contains(where: { $0.value == 0 }) else {
      finish(.failure(VoicePluginFailure(code: "voice_response")))
      return
    }
    finish(.success(text))
  }

  func urlSession(_ session: URLSession, task: URLSessionTask,
                  willPerformHTTPRedirection response: HTTPURLResponse,
                  newRequest request: URLRequest,
                  completionHandler: @escaping (URLRequest?) -> Void) {
    completionHandler(nil)
  }
}

private final class IOSVoiceTranscriptionSession {
  let args: VoiceTranscriptionArgs
  let invoke: Invoke
  var recorder: AVAudioRecorder?
  var file: URL?
  var timeout: DispatchWorkItem?
  var transport: VoiceTranscriptionTransport?
  var doubaoTransport: IOSVoiceDoubaoTransport?
  var stopRequested = false

  init(args: VoiceTranscriptionArgs, invoke: Invoke) {
    self.args = args
    self.invoke = invoke
  }
}

/// Native recording boundary for the Tauri settings host. The keyboard extension
/// never links this service and never receives provider credentials.
private final class IOSVoiceTranscriptionService {
  private static let maximumAudioBytes = 2_100_000
  private var active: IOSVoiceTranscriptionSession?
  private var backgroundObserver: NSObjectProtocol?

  init() {
    backgroundObserver = NotificationCenter.default.addObserver(
      forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main
    ) { [weak self] _ in
      self?.cancelForBackground()
    }
  }

  deinit {
    if let backgroundObserver {
      NotificationCenter.default.removeObserver(backgroundObserver)
    }
  }

  /// A settings app can be suspended while recording or waiting for a network
  /// response. Cancel the native session at the process lifecycle boundary so
  /// microphone capture, temporary audio and pending provider requests do not
  /// outlive the visible app. The shared panel receives the normal cancellation
  /// error and can be reopened without a stale busy state.
  private func cancelForBackground() {
    guard let session = active else { return }
    cancel(requestId: session.args.requestId)
  }

  private func valid(_ args: VoiceTranscriptionArgs) -> Bool {
    let endpoint = args.endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
    let model = args.model.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !args.requestId.isEmpty, args.requestId.utf8.count <= 64,
          args.requestId.utf8.allSatisfy({ byte in
            (byte >= 48 && byte <= 57) || (byte >= 65 && byte <= 90) ||
              (byte >= 97 && byte <= 122) || byte == 45
          }),
          model.utf8.count <= 512,
          !model.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
          args.token.utf8.count <= 16 * 1024,
          !args.token.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
          args.boostingTableId.utf8.count <= 4_096,
          !args.boostingTableId.unicodeScalars.contains(
            where: { CharacterSet.controlCharacters.contains($0) }),
          endpoint.utf8.count <= 2_048,
          let components = URLComponents(string: endpoint),
          components.host?.isEmpty == false,
          components.user == nil, components.password == nil, components.fragment == nil else {
      return false
    }
    if ["openai", "siliconflow", "groq", "everyapi", "mistral"].contains(args.provider) {
      return components.scheme?.lowercased() == "https" && !model.isEmpty &&
        args.headers.isEmpty && args.boostingTableId.isEmpty
    }
    guard args.provider == "doubao", components.scheme?.lowercased() == "wss",
          model.isEmpty, args.token.isEmpty, (3...4).contains(args.headers.count) else {
      return false
    }
    let allowed = Set([
      "x-api-key", "x-api-app-key", "x-api-access-key",
      "x-api-resource-id", "x-api-request-id",
    ])
    guard args.headers.allSatisfy({
      allowed.contains($0.name) && !$0.value.isEmpty && $0.value.utf8.count <= 8_192 &&
        !$0.value.unicodeScalars.contains(
          where: { CharacterSet.controlCharacters.contains($0) })
    }), Set(args.headers.map(\.name)).count == args.headers.count else { return false }
    let names = Set(args.headers.map(\.name))
    let shared = names.contains("x-api-resource-id") && names.contains("x-api-request-id")
    let apiKey = names == Set(["x-api-key", "x-api-resource-id", "x-api-request-id"])
    let legacy = names == Set([
      "x-api-app-key", "x-api-access-key", "x-api-resource-id", "x-api-request-id",
    ])
    return shared && (apiKey || legacy)
  }

  func start(_ args: VoiceTranscriptionArgs, invoke: Invoke) {
    guard valid(args) else {
      invoke.reject("invalid_voice", code: "invalid_voice")
      return
    }
    guard active == nil else {
      invoke.reject("busy", code: "busy")
      return
    }
    let session = IOSVoiceTranscriptionSession(args: args, invoke: invoke)
    active = session
    AVAudioSession.sharedInstance().requestRecordPermission { [weak self, weak session] granted in
      DispatchQueue.main.async {
        guard let self, let session, self.active === session else { return }
        guard granted else {
          self.fail(session, code: "microphone_permission")
          return
        }
        self.beginRecording(session)
      }
    }
  }

  func stop(requestId: String?) {
    guard let session = active,
          requestId == nil || requestId == session.args.requestId else { return }
    session.stopRequested = true
    if session.recorder != nil {
      finishRecording(session)
    }
  }

  func cancel(requestId: String?) {
    guard let session = active,
          requestId == nil || requestId == session.args.requestId else { return }
    active = nil
    cleanUp(session)
    session.invoke.reject("cancelled", code: "cancelled")
  }

  private func beginRecording(_ session: IOSVoiceTranscriptionSession) {
    do {
      let audioSession = AVAudioSession.sharedInstance()
      try audioSession.setCategory(.record, mode: .default)
      try audioSession.setActive(true)
      let file = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString + ".wav")
      let recorder = try AVAudioRecorder(url: file, settings: [
        AVFormatIDKey: kAudioFormatLinearPCM,
        AVSampleRateKey: 16_000,
        AVNumberOfChannelsKey: 1,
        AVLinearPCMBitDepthKey: 16,
        AVLinearPCMIsFloatKey: false,
        AVLinearPCMIsBigEndianKey: false,
      ])
      session.file = file
      session.recorder = recorder
      guard recorder.prepareToRecord(), recorder.record() else {
        throw VoicePluginFailure(code: "voice_recording")
      }
      let timeout = DispatchWorkItem { [weak self, weak session] in
        guard let self, let session, self.active === session else { return }
        self.finishRecording(session)
      }
      session.timeout = timeout
      DispatchQueue.main.asyncAfter(deadline: .now() + 60, execute: timeout)
      if session.stopRequested {
        finishRecording(session)
      }
    } catch {
      fail(session, code: "voice_recording")
    }
  }

  private func finishRecording(_ session: IOSVoiceTranscriptionSession) {
    guard active === session, session.transport == nil, session.doubaoTransport == nil else {
      return
    }
    session.timeout?.cancel()
    session.timeout = nil
    session.recorder?.stop()
    session.recorder = nil
    try? AVAudioSession.sharedInstance().setActive(false,
      options: .notifyOthersOnDeactivation)
    guard let file = session.file else { return }
    session.file = nil
    let audio = try? Self.readBoundedFile(file, maximumBytes: Self.maximumAudioBytes)
    try? FileManager.default.removeItem(at: file)
    guard let audio, audio.count >= 44, audio.count <= Self.maximumAudioBytes else {
      fail(session, code: "voice_recording")
      return
    }
    if session.args.provider == "doubao" {
      startDoubao(session, audio: audio)
      return
    }
    guard let request = transcriptionRequest(session.args, audio: audio) else {
      fail(session, code: "voice_recording")
      return
    }
    session.transport = VoiceTranscriptionTransport(request: request) {
      [weak self, weak session] result in
      guard let self, let session, self.active === session else { return }
      self.active = nil
      session.transport = nil
      switch result {
      case .success(let text): session.invoke.resolve(["text": text])
      case .failure(let failure):
        session.invoke.reject(failure.code, code: failure.code)
      }
    }
  }

  private static func readBoundedFile(_ url: URL, maximumBytes: Int) throws -> Data {
    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }
    var result = Data()
    result.reserveCapacity(min(maximumBytes, 64 * 1024))
    while true {
      let remaining = maximumBytes - result.count
      let chunk = try handle.read(upToCount: min(64 * 1024, remaining + 1)) ?? Data()
      if chunk.isEmpty { return result }
      guard chunk.count <= remaining else { throw VoicePluginFailure(code: "voice_recording") }
      result.append(chunk)
    }
  }

  private func startDoubao(_ session: IOSVoiceTranscriptionSession, audio: Data) {
    let headers = Dictionary(uniqueKeysWithValues:
      session.args.headers.map { ($0.name, $0.value) })
    guard let transport = IOSVoiceDoubaoTransport(
      endpoint: session.args.endpoint.trimmingCharacters(in: .whitespacesAndNewlines),
      headers: headers, enableITN: session.args.enableItn,
      punctuation: session.args.enablePunctuation, DDC: session.args.enableDdc,
      boostingTable: session.args.boostingTableId, wav: audio,
      completion: { [weak self, weak session] result in
        guard let self, let session, self.active === session else { return }
        self.active = nil
        session.doubaoTransport = nil
        switch result {
        case .success(let text): session.invoke.resolve(["text": text])
        case .failure(let failure):
          session.invoke.reject(failure.code, code: failure.code)
        }
      }) else {
      fail(session, code: "voice_recording")
      return
    }
    session.doubaoTransport = transport
    transport.start()
  }

  private func transcriptionRequest(_ args: VoiceTranscriptionArgs, audio: Data) -> URLRequest? {
    let endpoint = args.endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
    guard let url = URL(string: endpoint) else { return nil }
    let boundary = "MSIME-\(UUID().uuidString)"
    var body = Data()
    func append(_ value: String) { body.append(Data(value.utf8)) }
    append("--\(boundary)\r\n")
    append("Content-Disposition: form-data; name=\"file\"; filename=\"recording.wav\"\r\n")
    append("Content-Type: audio/wav\r\n\r\n")
    body.append(audio)
    append("\r\n--\(boundary)\r\n")
    append("Content-Disposition: form-data; name=\"model\"\r\n\r\n")
    append(args.model.trimmingCharacters(in: .whitespacesAndNewlines))
    append("\r\n--\(boundary)--\r\n")
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.timeoutInterval = 60
    request.httpBody = body
    request.setValue("multipart/form-data; boundary=\(boundary)",
      forHTTPHeaderField: "Content-Type")
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    if !args.token.isEmpty {
      request.setValue("Bearer \(args.token)", forHTTPHeaderField: "Authorization")
    }
    return request
  }

  private func fail(_ session: IOSVoiceTranscriptionSession, code: String) {
    guard active === session else { return }
    active = nil
    cleanUp(session)
    session.invoke.reject(code, code: code)
  }

  private func cleanUp(_ session: IOSVoiceTranscriptionSession) {
    session.timeout?.cancel()
    session.timeout = nil
    session.transport?.cancel()
    session.transport = nil
    session.doubaoTransport?.cancel()
    session.doubaoTransport = nil
    session.recorder?.stop()
    session.recorder = nil
    if let file = session.file {
      try? FileManager.default.removeItem(at: file)
    }
    session.file = nil
    try? AVAudioSession.sharedInstance().setActive(false,
      options: .notifyOthersOnDeactivation)
  }
}

private struct VoiceTextHandoff: Encodable {
  let version: Int = 1
  let id: UUID
  let text: String
  let createdAt: Date
  let expiresAt: Date
}

private final class VoiceTextHandoffWriter {
  private static let maximumBytes = 256 * 1024
  private static let lifetime: TimeInterval = 600
  private let directory: URL?

  init() {
    directory = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.msime.ios")?
      .appendingPathComponent("VoiceHandoff", isDirectory: true)
  }

  func save(_ text: String, now: Date = Date()) throws {
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          text.count <= 10_000, !text.unicodeScalars.contains(where: { $0.value == 0 }),
          let directory else { throw NSError(domain: "voice_handoff", code: 1) }
    let entry = VoiceTextHandoff(id: UUID(), text: text, createdAt: now,
      expiresAt: now.addingTimeInterval(Self.lifetime))
    let data = try JSONEncoder().encode(entry)
    guard data.count <= Self.maximumBytes else { throw NSError(domain: "voice_handoff", code: 2) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
      attributes: [.posixPermissions: 0o700])
    let descriptor = open(directory.appendingPathComponent("transfer.lock").path,
      O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw NSError(domain: "voice_handoff", code: 3) }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
      throw NSError(domain: "voice_handoff", code: 4)
    }
    try data.write(to: directory.appendingPathComponent("result.json"),
      options: [.atomic, .completeFileProtection])
  }
}

private struct SaveKeyboardPreferencesArgs: Decodable {
  let inputScheme: String
  let traditionalChineseOutput: Bool
  let soundEnabled: Bool
  let hapticsEnabled: Bool
  let hapticStrength: String
  let englishSuggestions: Bool
  let candidatePaletteFollowsDesktop: Bool
  let inlinePreedit: Bool
  /// Absent unless the page offered the iPad switch, so a phone never writes it.
  let tabletFullKeys: Bool?
  let dictionaryLearning: Bool
  let globalTheme: String
  let customKeyboardSkin: String?
}

private struct SaveKeyboardAIArgs: Decodable {
  let enabled: Bool
  let provider: String
  let endpoint: String
  let model: String
  let prompt: String
  let token: String
}

/// Compatibility storage for the native keyboard AI surface. The canonical
/// settings document lives in Rust; this mirror exists because the extension
/// cannot call into the Tauri WebView while it is active.
private final class IOSKeyboardAIStore {
  private static let group = "group.app.msime.ios"
  private static let configurationKey = "keyboard.ai.configuration"
  private static let keychainService = "app.msime.ios.keyboard-ai"
  private static let providers: Set<String> = [
    "everyAPI", "openAI", "anthropic", "gemini", "deepSeek", "qwen", "kimi",
    "zhipu", "siliconFlow", "openRouter", "custom",
  ]

  private var defaults: UserDefaults {
    UserDefaults(suiteName: Self.group) ?? .standard
  }

  private func keychainQuery(account: String? = nil) -> [String: Any] {
    var query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrAccessGroup as String: Self.group,
      kSecAttrService as String: Self.keychainService,
    ]
    if let account { query[kSecAttrAccount as String] = account }
    return query
  }

  private func origin(for endpoint: String) throws -> String {
    guard endpoint.utf8.count <= 2_048,
          !endpoint.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
          let url = URL(string: endpoint.trimmingCharacters(in: .whitespacesAndNewlines)),
          url.scheme?.lowercased() == "https",
          let host = url.host, !host.isEmpty,
          url.user == nil, url.password == nil, url.fragment == nil else {
      throw NSError(domain: "keyboard_ai", code: 1)
    }
    return "https://\(host.lowercased()):\(url.port ?? 443)"
  }

  private func bounded(_ value: String, maximum: Int, allowEmpty: Bool = true) -> Bool {
    value.utf8.count <= maximum
      && (allowEmpty || !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
      && !value.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) })
  }

  func save(_ args: SaveKeyboardAIArgs) throws {
    guard bounded(args.provider, maximum: 64), Self.providers.contains(args.provider),
          bounded(args.endpoint, maximum: 2_048), bounded(args.model, maximum: 512),
          bounded(args.prompt, maximum: 16 * 1_024), bounded(args.token, maximum: 16 * 1_024)
    else { throw NSError(domain: "keyboard_ai", code: 2) }

    guard args.enabled else {
      defaults.removeObject(forKey: Self.configurationKey)
      let status = SecItemDelete(keychainQuery() as CFDictionary)
      guard status == errSecSuccess || status == errSecItemNotFound else {
        throw NSError(domain: "keyboard_ai", code: 3)
      }
      return
    }

    guard !args.endpoint.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          !args.model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          !args.prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          !args.token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
      throw NSError(domain: "keyboard_ai", code: 4)
    }
    let account = try origin(for: args.endpoint)
    let query = keychainQuery(account: account)
    let attributes = [kSecValueData as String: Data(args.token.utf8)]
    var status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
    if status == errSecItemNotFound {
      var item = query.merging(attributes) { _, new in new }
      item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
      status = SecItemAdd(item as CFDictionary, nil)
    }
    guard status == errSecSuccess else { throw NSError(domain: "keyboard_ai", code: 5) }

    let document: [String: Any] = [
      "provider": args.provider,
      "voiceProvider": "custom",
      "voiceAppKey": "",
      "voiceResourceID": "",
      "doubaoEnableITN": true,
      "doubaoEnablePunctuation": true,
      "doubaoEnableDDC": false,
      "doubaoBoostingTableID": "",
      "endpoint": args.endpoint,
      "model": args.model,
      "prompt": args.prompt,
    ]
    let data = try JSONSerialization.data(withJSONObject: document, options: [])
    guard data.count <= 32 * 1_024 else { throw NSError(domain: "keyboard_ai", code: 6) }
    defaults.set(data, forKey: Self.configurationKey)
  }
}

/// App Group adapter for preferences that the keyboard extension can change
/// without opening the Tauri settings app. The keys and fallback behaviour are
/// fixed to MSIME-Apple develop@81e79abec7b53e7243fb8cbe82a42a4dde1e528f.
private struct IOSKeyboardPreferenceStore {
  static let maximumCustomSkinBytes = 800_000
  static let schemeOrder = [
    "quanpin", "nineKey", "shuangpin", "ziranma", "microsoft", "shoudao", "wubi",
    "japaneseNineKey", "japanese", "handwriting", "thoughtfulReply",
  ]
  /// The global theme ids (`GlobalTheme::ALL` in client-core), the only values `globalTheme` may hold.
  static let themeOrder = ["system", "shuishan", "light", "paper", "night", "ink", "custom"]
  static let hapticStrengths = ["light", "medium", "strong"]

  private var defaults: UserDefaults {
    UserDefaults(suiteName: "group.app.msime.ios") ?? .standard
  }

  private func migrateJapaneseSchemes() {
    guard !defaults.bool(forKey: "japaneseSchemesSplit") else { return }
    if var enabled = defaults.stringArray(forKey: "enabledInputSchemes"),
       enabled.contains("japanese"), !enabled.contains("japaneseNineKey") {
      enabled.append("japaneseNineKey")
      defaults.set(enabled, forKey: "enabledInputSchemes")
    }
    if defaults.string(forKey: "chineseInputScheme") == "japanese",
       !defaults.bool(forKey: "japaneseRomanKeys") {
      defaults.set("japaneseNineKey", forKey: "chineseInputScheme")
    }
    defaults.set(true, forKey: "japaneseSchemesSplit")
  }

  private func enabledSchemes() -> [String] {
    migrateJapaneseSchemes()
    guard let stored = defaults.stringArray(forKey: "enabledInputSchemes") else {
      return Self.schemeOrder
    }
    let enabled = Self.schemeOrder.filter(stored.contains)
    return enabled.isEmpty ? ["quanpin"] : enabled
  }

  private func selectedScheme() -> String {
    let enabled = enabledSchemes()
    let legacy = defaults.bool(forKey: "inputSchemeUsesShuangpin") ? "shuangpin" : "quanpin"
    let selected = defaults.string(forKey: "chineseInputScheme") ?? legacy
    return enabled.contains(selected) ? selected : enabled[0]
  }

  private func customSkinJSON() -> String? {
    guard let data = defaults.data(forKey: "customKeyboardSkin.v1"),
          !data.isEmpty, data.count <= Self.maximumCustomSkinBytes,
          let document = try? JSONSerialization.jsonObject(with: data),
          document is [String: Any] else {
      return nil
    }
    return String(data: data, encoding: .utf8)
  }

  func snapshot() -> [String: Any] {
    let strength = defaults.string(forKey: "keyboardHapticStrength") ?? "medium"
    let theme = defaults.string(forKey: "globalTheme") ?? "system"
    return [
      "inputScheme": selectedScheme(),
      "traditionalChineseOutput": defaults.bool(forKey: "chineseOutputUsesTraditional"),
      "soundEnabled": defaults.object(forKey: "keyboardSoundEnabled") as? Bool ?? true,
      "hapticsEnabled": defaults.bool(forKey: "keyboardHapticsEnabled"),
      "hapticStrength": Self.hapticStrengths.contains(strength) ? strength : "medium",
      "englishSuggestions": defaults.object(forKey: "english.suggestions") as? Bool ?? true,
      "candidatePaletteFollowsDesktop": defaults.bool(forKey: "candidate_palette_follows_desktop"),
      "inlinePreedit": defaults.bool(forKey: "keyboard.inline_preedit"),
      "tabletFullKeys": defaults.object(forKey: "keyboard.tablet.fullKeys") as? Bool ?? true,
      "dictionaryLearning": defaults.bool(forKey: "dictionaryLearningEnabled"),
      "globalTheme": Self.themeOrder.contains(theme) ? theme : "system",
      "customKeyboardSkin": customSkinJSON() as Any? ?? NSNull(),
    ]
  }

  func save(_ args: SaveKeyboardPreferencesArgs) throws -> [String: Any] {
    guard Self.schemeOrder.contains(args.inputScheme),
          Self.hapticStrengths.contains(args.hapticStrength),
          Self.themeOrder.contains(args.globalTheme) else {
      throw NSError(domain: "keyboard_preferences", code: 1)
    }
    if let custom = args.customKeyboardSkin {
      let data = Data(custom.utf8)
      guard !data.isEmpty, data.count <= Self.maximumCustomSkinBytes,
            let document = try? JSONSerialization.jsonObject(with: data),
            document is [String: Any] else {
        throw NSError(domain: "keyboard_preferences", code: 2)
      }
    }

    let enabled = enabledSchemes()
    let selected = enabled.contains(args.inputScheme) ? args.inputScheme : enabled[0]
    defaults.set(selected, forKey: "chineseInputScheme")
    defaults.set(["shuangpin", "ziranma", "microsoft", "shoudao"].contains(selected),
                 forKey: "inputSchemeUsesShuangpin")
    defaults.set(args.traditionalChineseOutput, forKey: "chineseOutputUsesTraditional")
    defaults.set(args.soundEnabled, forKey: "keyboardSoundEnabled")
    defaults.set(args.hapticsEnabled, forKey: "keyboardHapticsEnabled")
    defaults.set(args.hapticStrength, forKey: "keyboardHapticStrength")
    defaults.set(args.englishSuggestions, forKey: "english.suggestions")
    defaults.set(args.candidatePaletteFollowsDesktop, forKey: "candidate_palette_follows_desktop")
    defaults.set(args.inlinePreedit, forKey: "keyboard.inline_preedit")
    if let fullKeys = args.tabletFullKeys { defaults.set(fullKeys, forKey: "keyboard.tablet.fullKeys") }
    defaults.set(args.dictionaryLearning, forKey: "dictionaryLearningEnabled")
    defaults.set(args.globalTheme, forKey: "globalTheme")
    if let custom = args.customKeyboardSkin {
      defaults.set(Data(custom.utf8), forKey: "customKeyboardSkin.v1")
    } else {
      defaults.removeObject(forKey: "customKeyboardSkin.v1")
    }
    return snapshot()
  }
}

private struct AccountSessionKeychain {
  static let maximumPayloadBytes = 16 * 1024

  private var query: [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: "app.msime.backend.account",
      kSecAttrAccount as String: "https://api.msime.app",
    ]
  }

  private var legacyQuery: [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: "app.msime.ios.community",
      kSecAttrAccount as String: "api.msime.app",
    ]
  }

  private func loadData(_ baseQuery: [String: Any]) throws -> Data? {
    var lookup = baseQuery
    lookup[kSecReturnData as String] = true
    lookup[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(lookup as CFDictionary, &result)
    if status == errSecItemNotFound {
      return nil
    }
    guard status == errSecSuccess, let data = result as? Data else {
      throw NSError(domain: "secure_storage", code: Int(status))
    }
    return data
  }

  private func decode(_ data: Data) throws -> String {
    guard !data.isEmpty, data.count <= Self.maximumPayloadBytes,
          let value = String(data: data, encoding: .utf8) else {
      throw NSError(domain: "secure_storage", code: Int(errSecDecode))
    }
    return value
  }

  func load() throws -> String? {
    if let data = try loadData(query) {
      return try decode(data)
    }
    guard let data = try loadData(legacyQuery) else {
      return nil
    }
    return try decode(data)
  }

  func save(_ value: String) throws {
    let data = Data(value.utf8)
    guard !data.isEmpty, data.count <= Self.maximumPayloadBytes else {
      throw NSError(domain: "secure_storage", code: Int(errSecParam))
    }
    let attributes: [String: Any] = [
      kSecValueData as String: data,
      kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
    ]
    var status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
    if status == errSecItemNotFound {
      status = SecItemAdd(query.merging(attributes) { _, new in new } as CFDictionary, nil)
    }
    guard status == errSecSuccess else {
      throw NSError(domain: "secure_storage", code: Int(status))
    }
    try clearLegacy()
  }

  private func clearLegacy() throws {
    let status = SecItemDelete(legacyQuery as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else {
      throw NSError(domain: "secure_storage", code: Int(status))
    }
  }

  func clear() throws {
    try clearLegacy()
    let status = SecItemDelete(query as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else {
      throw NSError(domain: "secure_storage", code: Int(status))
    }
  }
}

/// Keeps the native Apple authorization delegate alive while a Tauri invoke is
/// pending. The identity token is resolved directly to Rust and never exposed
/// to the WebView.
private final class AppleSignInCoordinator: NSObject, ASAuthorizationControllerDelegate,
    ASAuthorizationControllerPresentationContextProviding {
  let invoke: Invoke
  let challengeId: String
  private let finish: () -> Void

  init(invoke: Invoke, challengeId: String, finish: @escaping () -> Void) {
    self.invoke = invoke
    self.challengeId = challengeId
    self.finish = finish
  }

  func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
    let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
    return scenes.flatMap(\.windows).first(where: \.isKeyWindow)
      ?? scenes.first?.windows.first ?? UIWindow()
  }

  func authorizationController(controller: ASAuthorizationController,
                               didCompleteWithAuthorization authorization: ASAuthorization) {
    defer { finish() }
    guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
          credential.state == challengeId,
          let data = credential.identityToken,
          let token = String(data: data, encoding: .utf8),
          !token.isEmpty,
          token.utf8.count <= 16 * 1024,
          !token.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
      invoke.reject("apple_sign_in", code: "apple_sign_in")
      return
    }
    invoke.resolve(["credential": token])
  }

  func authorizationController(controller: ASAuthorizationController,
                               didCompleteWithError error: Error) {
    finish()
    if let authorizationError = error as? ASAuthorizationError,
       authorizationError.code == .canceled {
      invoke.reject("account_cancelled", code: "account_cancelled")
    } else {
      invoke.reject("apple_sign_in", code: "apple_sign_in")
    }
  }
}

/// Keeps the folder picker's delegate alive while the invoke is pending. It reports the folder, or nil when the user dismissed the picker, exactly once, whichever way the sheet went away.
private final class SkinFolderPickerCoordinator: NSObject, UIDocumentPickerDelegate,
    UIAdaptivePresentationControllerDelegate {
  private var finish: ((URL?) -> Void)?

  init(finish: @escaping (URL?) -> Void) {
    self.finish = finish
  }

  private func complete(_ url: URL?) {
    let finish = self.finish
    self.finish = nil
    finish?(url)
  }

  func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
    complete(urls.first)
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    complete(nil)
  }

  func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
    complete(nil)
  }
}

final class MobilePlatformPlugin: Plugin {
  private static let onboardingKey = "hasCompletedOnboarding"
  private let accountSession = AccountSessionKeychain()
  private let keyboardPreferences = IOSKeyboardPreferenceStore()
  private let keyboardAI = IOSKeyboardAIStore()
  private let voiceHandoff = VoiceTextHandoffWriter()
  private let voiceTranscription = IOSVoiceTranscriptionService()
  private var appleSignIn: AppleSignInCoordinator?
  private var skinFolderPicker: SkinFolderPickerCoordinator?
  /// The picked folder while Rust copies it; security-scoped access is per process, so holding it here is what lets the copy read the folder.
  private var skinFolderAccess: (url: URL, scoped: Bool)?
  private var previewFeedback: UIImpactFeedbackGenerator?

  /// Only iPhones have the Taptic Engine keyboard feedback drives, so the settings page hides the vibration controls elsewhere, as the native settings app does; only iPads draw the full-width keyboard that carries the digit row and Tab key, so the switch is reported there alone. The idiom is read on the main thread, where UIKit answers it.
  private func resolveKeyboardPreferences(_ invoke: Invoke, _ snapshot: [String: Any]) {
    onMain {
      var snapshot = snapshot
      let idiom = UIDevice.current.userInterfaceIdiom
      snapshot["hapticsAvailable"] = idiom == .phone
      if idiom != .pad { snapshot.removeValue(forKey: "tabletFullKeys") }
      invoke.resolve(snapshot)
    }
  }

  private func onMain(_ action: @escaping () -> Void) {
    if Thread.isMainThread {
      action()
    } else {
      DispatchQueue.main.async(execute: action)
    }
  }

  private func iconName(for style: String) -> String?? {
    switch style {
    case "classic": return .some(nil)
    case "forest": return .some("AppIconForest")
    case "sky": return .some("AppIconSky")
    case "dusk": return .some("AppIconDusk")
    case "vermilion": return .some("AppIconVermilion")
    default: return nil
    }
  }

  private func selectedStyle(for iconName: String?) -> String {
    switch iconName {
    case "AppIconForest": return "forest"
    case "AppIconSky": return "sky"
    case "AppIconDusk": return "dusk"
    case "AppIconVermilion": return "vermilion"
    default: return "classic"
    }
  }

  private func resolveInfo(_ invoke: Invoke, application: UIApplication) {
    invoke.resolve([
      "supported": application.supportsAlternateIcons,
      "selected": selectedStyle(for: application.alternateIconName),
    ])
  }

  @objc public func openSystemKeyboardSettings(_ invoke: Invoke) {
    onMain { [self] in
      guard let url = URL(string: UIApplication.openSettingsURLString) else {
        invoke.reject("system_settings", code: "system_settings")
        return
      }
      UIApplication.shared.open(url, options: [:]) { opened in
        self.onMain {
          if opened {
            invoke.resolve()
          } else {
            invoke.reject("system_settings", code: "system_settings")
          }
        }
      }
    }
  }

  @objc public func onboardingStatus(_ invoke: Invoke) {
    invoke.resolve(["completed": UserDefaults.standard.bool(forKey: Self.onboardingKey)])
  }

  @objc public func completeOnboarding(_ invoke: Invoke) {
    UserDefaults.standard.set(true, forKey: Self.onboardingKey)
    invoke.resolve()
  }

  @objc public func appIconInfo(_ invoke: Invoke) {
    onMain { [self] in
      resolveInfo(invoke, application: UIApplication.shared)
    }
  }

  @objc public func setAppIcon(_ invoke: Invoke) {
    let args: SetAppIconArgs
    do {
      args = try invoke.parseArgs(SetAppIconArgs.self)
    } catch {
      invoke.reject("invalid_app_icon", code: "invalid_app_icon")
      return
    }
    guard let requestedName = iconName(for: args.style) else {
      invoke.reject("invalid_app_icon", code: "invalid_app_icon")
      return
    }

    onMain { [self] in
      let application = UIApplication.shared
      guard application.supportsAlternateIcons else {
        resolveInfo(invoke, application: application)
        return
      }
      application.setAlternateIconName(requestedName) { error in
        self.onMain {
          // Simulator runtimes may report an I/O failure after applying the icon.
          // Trust the state the OS exposes after completion before rejecting.
          if error != nil && application.alternateIconName != requestedName {
            invoke.reject("app_icon", code: "app_icon")
          } else {
            self.resolveInfo(invoke, application: application)
          }
        }
      }
    }
  }

  @objc public func loadSession(_ invoke: Invoke) {
    do {
      let value = try accountSession.load()
      invoke.resolve(["value": value as Any? ?? NSNull()])
    } catch {
      invoke.reject("secure_storage", code: "secure_storage")
    }
  }

  @objc public func saveSession(_ invoke: Invoke) {
    do {
      let args = try invoke.parseArgs(SaveAccountSessionArgs.self)
      try accountSession.save(args.value)
      invoke.resolve()
    } catch {
      invoke.reject("secure_storage", code: "secure_storage")
    }
  }

  @objc public func clearSession(_ invoke: Invoke) {
    do {
      try accountSession.clear()
      invoke.resolve()
    } catch {
      invoke.reject("secure_storage", code: "secure_storage")
    }
  }

  @objc public func signInWithApple(_ invoke: Invoke) {
    let args: AppleSignInArgs
    do {
      args = try invoke.parseArgs(AppleSignInArgs.self)
    } catch {
      invoke.reject("invalid_apple_sign_in", code: "invalid_apple_sign_in")
      return
    }
    guard !args.challengeId.isEmpty, args.challengeId.utf8.count <= 256,
          !args.challengeId.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
          !args.nonce.isEmpty, args.nonce.utf8.count <= 4096,
          !args.nonce.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
      invoke.reject("invalid_apple_sign_in", code: "invalid_apple_sign_in")
      return
    }
    onMain { [weak self] in
      guard let self else { return }
      guard self.appleSignIn == nil else {
        invoke.reject("busy", code: "busy")
        return
      }
      let request = ASAuthorizationAppleIDProvider().createRequest()
      request.requestedScopes = []
      request.nonce = args.nonce
      request.state = args.challengeId
      let coordinator = AppleSignInCoordinator(invoke: invoke, challengeId: args.challengeId) { [weak self] in
        self?.appleSignIn = nil
      }
      self.appleSignIn = coordinator
      let controller = ASAuthorizationController(authorizationRequests: [request])
      controller.delegate = coordinator
      controller.presentationContextProvider = coordinator
      controller.performRequests()
    }
  }

  /// Lets the user pick a skin folder in Files and hands its path to Rust, which copies it into the App Group skin folder. The folder stays readable until `endSkinFolderAccess`. Dismissing the picker resolves with no path: changing one's mind is not a failure.
  @objc public func pickSkinFolder(_ invoke: Invoke) {
    onMain { [weak self] in
      guard let self else { return }
      guard self.skinFolderPicker == nil, self.skinFolderAccess == nil else {
        invoke.reject("busy", code: "busy")
        return
      }
      var presenter = self.manager.viewController
      while let presented = presenter?.presentedViewController {
        presenter = presented
      }
      // The app itself targets iOS 16; the guard is for this package's older manifest floor.
      guard let presenter, #available(iOS 14.0, *) else {
        invoke.reject("skin_import", code: "skin_import")
        return
      }
      let coordinator = SkinFolderPickerCoordinator { [weak self] url in
        guard let self else { return }
        self.skinFolderPicker = nil
        guard let url else {
          invoke.resolve(["path": NSNull()])
          return
        }
        // A folder inside the app's own container is readable without a scope, and reports false here.
        self.skinFolderAccess = (url, url.startAccessingSecurityScopedResource())
        invoke.resolve(["path": url.path])
      }
      self.skinFolderPicker = coordinator
      let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder])
      picker.allowsMultipleSelection = false
      picker.delegate = coordinator
      picker.presentationController?.delegate = coordinator
      presenter.present(picker, animated: true)
    }
  }

  @objc public func endSkinFolderAccess(_ invoke: Invoke) {
    onMain { [self] in
      if let access = skinFolderAccess, access.scoped {
        access.url.stopAccessingSecurityScopedResource()
      }
      skinFolderAccess = nil
      invoke.resolve()
    }
  }

  /// Installed UIKit families for the shared font picker; the Rust side sorts and bounds them.
  @objc public func listFontFamilies(_ invoke: Invoke) {
    invoke.resolve(["families": UIFont.familyNames])
  }

  @objc public func copyText(_ invoke: Invoke) {
    let args: CopyTextArgs
    do {
      args = try invoke.parseArgs(CopyTextArgs.self)
    } catch {
      invoke.reject("invalid_clipboard_text", code: "invalid_clipboard_text")
      return
    }
    guard !args.text.isEmpty, args.text.utf16.count <= 4_000,
          !args.text.unicodeScalars.contains(where: { $0.value == 0 }) else {
      invoke.reject("invalid_clipboard_text", code: "invalid_clipboard_text")
      return
    }
    onMain {
      UIPasteboard.general.string = args.text
      invoke.resolve()
    }
  }

  @objc public func saveVoiceText(_ invoke: Invoke) {
    do {
      let args = try invoke.parseArgs(SaveVoiceTextArgs.self)
      try voiceHandoff.save(args.text)
      invoke.resolve()
    } catch {
      invoke.reject("voice_handoff", code: "voice_handoff")
    }
  }

  @objc public func recognizeVoice(_ invoke: Invoke) {
    let args: VoiceTranscriptionArgs
    do {
      args = try invoke.parseArgs(VoiceTranscriptionArgs.self)
    } catch {
      invoke.reject("invalid_voice", code: "invalid_voice")
      return
    }
    onMain { [self] in voiceTranscription.start(args, invoke: invoke) }
  }

  @objc public func stopVoice(_ invoke: Invoke) {
    let args: VoiceControlArgs
    do {
      args = try invoke.parseArgs(VoiceControlArgs.self)
    } catch {
      invoke.reject("invalid_voice", code: "invalid_voice")
      return
    }
    onMain { [self] in
      voiceTranscription.stop(requestId: args.requestId)
      invoke.resolve()
    }
  }

  @objc public func cancelVoice(_ invoke: Invoke) {
    let args: VoiceControlArgs
    do {
      args = try invoke.parseArgs(VoiceControlArgs.self)
    } catch {
      invoke.reject("invalid_voice", code: "invalid_voice")
      return
    }
    onMain { [self] in
      voiceTranscription.cancel(requestId: args.requestId)
      invoke.resolve()
    }
  }

  @objc public func loadKeyboardPreferences(_ invoke: Invoke) {
    resolveKeyboardPreferences(invoke, keyboardPreferences.snapshot())
  }

  @objc public func saveKeyboardPreferences(_ invoke: Invoke) {
    do {
      let args = try invoke.parseArgs(SaveKeyboardPreferencesArgs.self)
      resolveKeyboardPreferences(invoke, try keyboardPreferences.save(args))
    } catch {
      invoke.reject("keyboard_preferences", code: "keyboard_preferences")
    }
  }
  @objc public func saveKeyboardAI(_ invoke: Invoke) {
    do {
      let args = try invoke.parseArgs(SaveKeyboardAIArgs.self)
      try keyboardAI.save(args)
      invoke.resolve()
    } catch {
      invoke.reject("keyboard_ai", code: "keyboard_ai")
    }
  }


  @objc public func previewKeyboardHaptics(_ invoke: Invoke) {
    let args: PreviewKeyboardHapticsArgs
    do {
      args = try invoke.parseArgs(PreviewKeyboardHapticsArgs.self)
    } catch {
      invoke.reject("invalid_feedback", code: "invalid_feedback")
      return
    }
    let style: UIImpactFeedbackGenerator.FeedbackStyle
    let intensity: CGFloat
    switch args.strength {
    case "light": style = .light; intensity = 0.45
    case "medium": style = .medium; intensity = 0.75
    case "strong": style = .heavy; intensity = 1.0
    default:
      invoke.reject("invalid_feedback", code: "invalid_feedback")
      return
    }
    onMain { [self] in
      let generator = UIImpactFeedbackGenerator(style: style)
      previewFeedback = generator
      generator.prepare()
      generator.impactOccurred(intensity: intensity)
      invoke.resolve()
    }
  }
}

@_cdecl("init_plugin_msime_mobile_platform")
func initPlugin() -> Plugin {
  MobilePlatformPlugin()
}
