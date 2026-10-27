import Foundation

/// 「我的词条」经 WebDAV 网盘同步（`docs/dictionary-webdav-sync.md`）时 iOS 这一侧的纯逻辑：偏好里那一组怎么读、给人看的说法、设备列表怎么去重，以及键盘扩展什么时候可以自己记一次本机改动。不碰文件和网络，KeyboardTests 直接测它。
///
/// 地址、账号和应用密码的规则与云剪贴板的网盘完全相同（Rust 的 `dictionary_sync_webdav` 也用 `valid_relay_url` 那一套），当场提示直接用 `IOSCloudClipboardRelayPolicy` 的 `webdavURLIssue`、`usernameIssue`、`passwordIssue`；保存时 Rust 还会校验整份偏好。
enum IOSDictionarySyncPolicy {
  /// 偏好里词库同步的键：`{url, username, password}`，没配置时不写进文档。与 `cloud_clipboard_webdav` 各配各的。
  static let preference = "dictionary_sync_webdav"

  /// 偏好里现在的词库同步配置；没配置时为 nil。应用密码只留着给「测试连接」和「保存」用，设置页只看有没有。
  struct Configuration: Equatable, Sendable {
    let url: String
    let username: String
    /// 已存的应用密码；不显示、不写日志。
    let savedPassword: String
    var hasPassword: Bool { !savedPassword.isEmpty }

    init?(preferences: [String: Any]?) {
      guard let webdav = preferences?[IOSDictionarySyncPolicy.preference] as? [String: Any],
            let url = webdav["url"] as? String, !url.isEmpty else { return nil }
      self.url = url
      username = webdav["username"] as? String ?? ""
      savedPassword = webdav["password"] as? String ?? ""
    }

    /// 设备身份绑定地址和账号：换了其中一个就是一台新设备，换应用密码不换。
    var binding: String { IOSCloudClipboardRelayPolicy.webdavBinding(url: url, username: username) }
  }

  // MARK: - 键盘扩展

  /// 键盘扩展自己调 `run(local_change)` 时，本机同步状态文件最多这么大。`run` 会把整份状态和「我的词条」读进内存，状态文件大致与词条数成正比；超过这个大小（大约几万个词）就留给 App 在打开时记录，免得键盘扩展因为内存上限被系统结束。
  static let keyboardStateLimit = 2 * 1024 * 1024

  /// 键盘扩展这一次能不能自己记录本机改动：有完全访问（没有它就写不了 App Group 容器，也连不上网），App 已经用这个网盘同步过（目录里已经有同步状态和词库密钥串，所以键盘从不生成设备身份、也不替 App 第一次登记），并且状态文件不大。
  static func keyboardMayRecord(fullAccess: Bool, stateBytes: Int?, keystorePresent: Bool) -> Bool {
    guard fullAccess, keystorePresent, let stateBytes else { return false }
    return stateBytes > 0 && stateBytes <= keyboardStateLimit
  }

  // MARK: - 设备

  /// 每台设备只列一条，与共享设置页的 `dictionarySyncDeviceGroups` 相同：设备编号相同的几条登记合成一条，获准、本机任一条是就算是，名称取最后一条非空的，位置留在第一次出现处。Rust 按编号登记，本来就不会重复；这里再挡一次，列表的 `ForEach` 也要求编号唯一。
  static func uniqueDevices(_ devices: [BackendAccountClient.ClipboardDevice]) -> [BackendAccountClient.ClipboardDevice] {
    var order: [String] = []
    var merged: [String: BackendAccountClient.ClipboardDevice] = [:]
    for device in devices {
      guard let seen = merged[device.device_id] else {
        order.append(device.device_id)
        merged[device.device_id] = device
        continue
      }
      merged[device.device_id] = BackendAccountClient.ClipboardDevice(
        device_id: seen.device_id, name: device.name.isEmpty ? seen.name : device.name, created_at: seen.created_at,
        approved: seen.approved || device.approved, current: seen.current || device.current, code: seen.code)
    }
    return order.compactMap { merged[$0] }
  }

  /// 等待本机允许的其他设备（合并之后）；本机自己还没获准时它允许不了别人，列表为空。
  static func awaitingApproval(_ devices: BackendAccountClient.ClipboardDevices?) -> [BackendAccountClient.ClipboardDevice] {
    guard let devices, devices.status == "ready" else { return [] }
    return uniqueDevices(devices.devices).filter { !$0.current && !$0.approved }
  }

  // MARK: - 说法

  /// 同步失败时给人看的一句话，与共享设置页的 `dictionarySyncErrorMessage`（`packages/ui/src/settings/dictionary-sync.ts`）逐字相同。`probe` 为真时是「测试连接」的结果：`dictionary_sync_unsupported` 在那里说的是网盘不合格，在同步时说的是网盘上的格式比本机新。错误码见 `msime_client.h` 的 `msime_client_dictionary_sync`；宿主自己的 `unavailable`（准备不了词库资源）落到最后一句。
  static func failureMessage(_ code: String, probe: Bool = false) -> String {
    switch code {
    case "dictionary_sync_not_configured": return "地址、账号或应用密码不完整。"
    case "invalid_dictionary_sync": return "请求无效，请关闭设置后重新打开再试。"
    case "dictionary_sync_unauthorized": return "网盘拒绝了这个账号或应用密码。"
    case "dictionary_sync_unavailable": return "连不上网盘，请检查地址和网络；证书须由公网信任的机构签发。"
    case "dictionary_sync_rate_limited": return "网盘暂时限制了请求次数，或本机这 30 分钟的同步次数已用完，请稍后再试。"
    case "dictionary_sync_too_large": return "网盘上的词库或本机的同步记录超过了上限，删减一些词条后再试。"
    case "dictionary_sync_rollback": return "网盘上的词库比本机读到过的旧，或者被删改过。如果是你自己清理了网盘上的文件夹，重新测试连接并保存即可，本机的词条都会保留。"
    case "dictionary_sync_unsupported":
      return probe
        ? "这个网盘不支持词库同步需要的文件操作（独占创建、列目录），请换一个 WebDAV 服务或文件夹。"
        : "网盘上的词库是更新版本的格式，请先更新水杉输入法。"
    case "dictionary_sync_corrupt": return "网盘上的词库记录损坏了，自动同步已暂停。"
    case "dictionary_sync_storage": return "本机的同步记录读写失败。"
    case "clipboard_device_pending": return "本设备还在等待允许。"
    case "not_found": return "这台设备已经不在列表里了，请刷新后再试。"
    case "clipboard_key_conflict", "clipboard_keystore_conflict": return "另一台设备同时修改了密钥，请稍后再试。"
    case "clipboard_device_limit": return "设备数已达上限，请先移除不再使用的设备。"
    default: return probe ? "测试连接失败，请稍后再试。" : "同步失败，请稍后再试。"
    }
  }

  /// 「测试连接」失败时给人看的一句话。
  static func probeMessage(_ code: String) -> String { failureMessage(code, probe: true) }

  /// 网盘填写页和同步页下面的说明。
  static let note = "只同步「我的词条」里的词和它们的删除，不同步打字学到的权重和候选顺序。词条在本机加密，网盘只保存密文；新设备要在已经在同步的设备上核对后点「允许」，与云剪贴板分开批准。打开设置 App 时同步，App 在前台时每 30 分钟再同步一次；坚果云免费账号每 30 分钟 600 次请求与其他应用共用。"

  /// 被挡下的大批删除的说明。
  static func heldDeletionsMessage(_ count: Int) -> String {
    "本机一次少了 \(count) 个词，可能是词库被重置或损坏了。为了不把其他设备上的词也删掉，这些删除还没有同步。"
  }

  /// 「上次同步」那一行：成功过就是相对时间，否则说还没有同步过。
  static func lastSuccess(_ milliseconds: UInt64?, now: Date = Date()) -> String {
    guard let milliseconds, milliseconds > 0 else { return "尚未同步" }
    let date = min(Date(timeIntervalSince1970: TimeInterval(milliseconds) / 1000), now)
    return relative.localizedString(for: date, relativeTo: now)
  }

  private static let relative: RelativeDateTimeFormatter = {
    let formatter = RelativeDateTimeFormatter()
    formatter.locale = Locale(identifier: "zh-Hans")
    formatter.unitsStyle = .full
    return formatter
  }()
}
