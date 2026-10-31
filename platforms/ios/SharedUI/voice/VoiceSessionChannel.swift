import Foundation

/// 主 App 后台语音会话的状态，由 App 写、键盘读。键盘扩展不能录音，按住空格说话时由在后台保持录音会话的主 App 录音识别，键盘只显示状态并上屏（与 Android 的 `ImeVoiceEntry` 同一套交互）。录音和凭据不进这个文件，只有识别出的文字。
struct VoiceSessionState: Codable, Equatable, Sendable {
  enum Phase: String, Codable, Sendable {
    /// 会话开着、没有在听写。
    case ready
    case listening
    /// 停止录音后在识别或润色。
    case recognizing
    case finished
    case failed
  }

  var version = 1
  var phase: Phase
  /// 这个状态回应的那次听写；`ready` 时为 nil。
  var request: UUID?
  /// 听写中是实时识别的文字，`finished` 时是最终结果。
  var text = ""
  /// `failed` 时的原因，`recognizing` 时可以是「正在润色…」之类的进度。
  var message = ""
  /// 听写中最近的麦克风电平（0…1），只在 `listening` 时有；键盘拿它驱动聆听面板的光晕。可选字段，旧版本写的状态里没有它也照常解码。
  var level: Float?
  /// 听写中 App 会不会在停顿后自己结束这次录音：点按开始、用本地模型识别时为 true（#10310），只在 `listening` 时有，键盘据此把聆听面板的提示换成「停顿后自动上屏」。可选字段，旧版本写的状态里没有它也照常解码，按不会处理。
  var endsOnPause: Bool?
  /// App 定时刷新；App 被系统结束后它不再更新，键盘据此判断会话已经不在。
  var heartbeat: Date
  /// 会话空闲到这个时刻就由 App 自己关掉。
  var expiresAt: Date

  /// 心跳超过这么久没更新就当作 App 已经不在。App 每 `VoiceSessionChannel.heartbeatInterval` 刷新一次。
  static let staleAfter: TimeInterval = 10

  /// App 的会话此刻能不能接听写命令。
  func isAvailable(now: Date = Date()) -> Bool {
    version == 1 && now.timeIntervalSince(heartbeat) < Self.staleAfter && heartbeat <= now.addingTimeInterval(5) && expiresAt > now
  }
}

/// 键盘发给 App 的听写命令。
struct VoiceSessionCommand: Codable, Equatable, Sendable {
  enum Action: String, Codable, Sendable { case start, stop, cancel }

  var version = 1
  let action: Action
  let request: UUID
  let sentAt: Date
  /// 开始命令带上：这次听写是点按开始的（工具栏、「更多工具」或读屏操作），App 可以在本地识别出字后按停顿结束它（#10310）。按住空格说话为 false，由松手结束。可选字段，旧版本键盘写的命令里没有它，按按住处理，行为不变。
  var tapStarted: Bool?
}

/// 键盘与主 App 之间的按住说话通道：App Group 里的两个小 JSON 文件，各由一方写，写完发一条 Darwin 通知让对方来读。Darwin 通知不带内容，所以内容都在文件里。
final class VoiceSessionChannel: @unchecked Sendable {
  /// App 刷新心跳的间隔。
  static let heartbeatInterval: TimeInterval = 4
  /// 听写中 App 最多隔这么久报一次麦克风电平。
  static let levelInterval: TimeInterval = 0.1
  /// 电平变化不到这么多就不报，安静时不反复写文件、发通知。
  static let levelStep: Float = 0.02
  /// 会话空闲多久后 App 关掉录音会话，状态栏的麦克风标记随之消失。会话开着时麦克风和音频引擎一直在跑（App 在后台不能重新开始录音，所以听写之间停不得），音频线程每秒醒几十次，CPU 进不了深度空闲，低端机上明显耗电发热，而且不管键盘还在不在屏幕上都一样。原来取 5 分钟，偶尔说一句的用户等于麦克风几乎一直开着；时限从每次听写开始和出结果时重新算，接连几句话不会中断，所以取 1 分钟：说完一分钟内不再说就交还麦克风，代价是隔得更久再说时要多跳一次 App。
  static let idleLifetime: TimeInterval = 60
  private static let maximumBytes = 64 * 1024

  /// 按版本区分，装了多个版本时不会串到另一个版本的 App。
  static var stateNotification: String { MSIMEAppEdition.appGroupIdentifier + ".voice-session.state" }
  static var commandNotification: String { MSIMEAppEdition.appGroupIdentifier + ".voice-session.command" }

  private let directory: URL?

  init(directory: URL? = VoiceTextHandoffStore.defaultDirectory) {
    self.directory = directory?.appendingPathComponent("VoiceSession", isDirectory: true)
  }

  /// 这次采到的电平和上次报给键盘的（还没报过当作 0）相差够不够 `levelStep`，够才再报。
  static func publishesLevel(_ level: Float, after published: Float?) -> Bool {
    abs(level - (published ?? 0)) >= levelStep
  }

  func readState() -> VoiceSessionState? { read("state.json") }
  func readCommand() -> VoiceSessionCommand? { read("command.json") }

  /// App 写状态并通知键盘。
  func publish(_ state: VoiceSessionState) {
    store(state, to: "state.json")
    Self.post(Self.stateNotification)
  }

  /// App 只刷新心跳：写状态但不通知，键盘下次需要时自己来读。
  func write(_ state: VoiceSessionState) {
    store(state, to: "state.json")
  }

  /// 键盘写命令并通知 App。
  func send(_ command: VoiceSessionCommand) {
    store(command, to: "command.json")
    Self.post(Self.commandNotification)
  }

  /// 会话结束时 App 删掉状态，键盘下次长按空格会重新打开 App。
  func clearState() {
    guard let directory else { return }
    try? FileManager.default.removeItem(at: directory.appendingPathComponent("state.json"))
    Self.post(Self.stateNotification)
  }

  private func read<T: Decodable>(_ name: String) -> T? {
    guard let directory else { return nil }
    let file = directory.appendingPathComponent(name)
    guard !SafePath.hasRefusedSymbolicLink(file),
          let data = try? BoundedFileReader.read(from: file, maximumBytes: Self.maximumBytes) else { return nil }
    return try? Self.decoder.decode(T.self, from: data)
  }

  private func store<T: Encodable>(_ value: T, to name: String) {
    guard let directory, !SafePath.hasRefusedSymbolicLink(directory),
          let data = try? Self.encoder.encode(value), data.count <= Self.maximumBytes else { return }
    try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    // 用键盘打字时设备一定解锁过，不需要 `.complete`；App 在后台、屏幕锁上时也还要能写心跳。
    try? data.write(to: directory.appendingPathComponent(name), options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
  }

  private static let encoder: JSONEncoder = {
    let encoder = JSONEncoder()
    encoder.dateEncodingStrategy = .millisecondsSince1970
    return encoder
  }()

  private static let decoder: JSONDecoder = {
    let decoder = JSONDecoder()
    decoder.dateDecodingStrategy = .millisecondsSince1970
    return decoder
  }()

  static func post(_ name: String) {
    CFNotificationCenterPostNotification(CFNotificationCenterGetDarwinNotifyCenter(),
                                         CFNotificationName(name as CFString), nil, nil, true)
  }
}

/// 一条 Darwin 通知的观察者，回调在主线程上。释放时自动注销。
final class DarwinNotificationObserver {
  private let name: String
  private let handler: () -> Void

  init(_ name: String, handler: @escaping () -> Void) {
    self.name = name
    self.handler = handler
    CFNotificationCenterAddObserver(
      CFNotificationCenterGetDarwinNotifyCenter(), Unmanaged.passUnretained(self).toOpaque(),
      { _, observer, _, _, _ in
        guard let observer else { return }
        let instance = Unmanaged<DarwinNotificationObserver>.fromOpaque(observer).takeUnretainedValue()
        if Thread.isMainThread { instance.handler() } else { DispatchQueue.main.async { [weak instance] in instance?.handler() } }
      },
      name as CFString, nil, .deliverImmediately)
  }

  deinit {
    CFNotificationCenterRemoveObserver(CFNotificationCenterGetDarwinNotifyCenter(), Unmanaged.passUnretained(self).toOpaque(),
                                       CFNotificationName(name as CFString), nil)
  }
}
