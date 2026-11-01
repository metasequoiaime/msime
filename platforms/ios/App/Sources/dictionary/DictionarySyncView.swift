import SwiftUI
import UIKit

/// 「词库 → 词库同步」：把「我的词条」经用户自己的 WebDAV 网盘在设备之间同步（`docs/dictionary-webdav-sync.md`）。与「我的 → 云剪贴板」的网盘填写页做法相同，但两者各配各的、分开批准设备。
///
/// 页面上有：网盘的地址、账号和应用密码（填写页里测试连接通过才保存）；同步状态和「立即同步」；本机等待批准时的核对码；被挡下的大批删除的两个选择；等待本机批准的设备和全部设备（每台一条）；很少用到的「重置密钥」「以本机为准重新开始」和「停止同步」。规则、合并、额度和设备批准都在 Rust 里，页面只显示它给的结果，错误码的说法见 `IOSDictionarySyncPolicy.failureMessage`。
///
/// 打开页面时以 `opened` 同步一次（Rust 一分钟内不重复拉取），之后读一次设备列表。每次请求都在专用线程上，结果回来时页面已经离开就丢掉。
struct DictionarySyncView: View {
  @ObservedObject private var center = DictionarySyncCenter.shared
  @State private var configuration = DictionarySyncCenter.configuration()
  @State private var status: IOSDictionarySync.Status?
  @State private var devices: BackendAccountClient.ClipboardDevices?
  @State private var busy = false
  @State private var message: String?
  @State private var editing = false
  @State private var confirmation: Confirmation?
  /// 页面每出现一次加一；旧请求回来时据此丢弃，不让它改写新页面。
  @State private var generation = 0

  private enum Confirmation: Identifiable {
    case confirmDeletions(Int), reset, restart, trustRemote, disable
    case remove(BackendAccountClient.ClipboardDevice)
    case reject(BackendAccountClient.ClipboardDevice)

    var id: String {
      switch self {
      case .confirmDeletions: return "confirmDeletions"
      case .reset: return "reset"
      case .trustRemote: return "trustRemote"
      case .restart: return "restart"
      case .disable: return "disable"
      case .remove(let device): return "remove-\(device.device_id)"
      case .reject(let device): return "reject-\(device.device_id)"
      }
    }
  }

  private var working: Bool { busy || center.running > 0 }
  private var isPending: Bool { status?.isPending == true || devices?.isPending == true }
  private var ownCode: String? { status?.code ?? devices?.code }
  private var deviceList: [BackendAccountClient.ClipboardDevice] {
    IOSDictionarySyncPolicy.uniqueDevices(devices?.devices ?? [])
  }
  private var awaitingApproval: [BackendAccountClient.ClipboardDevice] {
    isPending ? [] : IOSDictionarySyncPolicy.awaitingApproval(devices)
  }
  /// 最近一次同步的错误，没有时是状态里记着的上一次错误。
  private var lastError: String? { center.lastRun?.error ?? status?.last_error }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 24) {
        webdavGroup
        if configuration != nil {
          statusGroup
          if isPending { pendingCard }
          if let held = status?.held_deletions, held > 0 { heldDeletionsCard(held) }
          if !awaitingApproval.isEmpty { approvalsGroup }
          if !deviceList.isEmpty { devicesGroup }
          maintenanceGroup
        }
      }
      .padding(.horizontal, 16).padding(.top, 8).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .designCanvasBackground()
    .settingsStatus(busy: working, busyTitle: "正在同步…", message: message)
    .navigationTitle("词库同步")
    .navigationBarTitleDisplayMode(.inline)
    // 不用 `.task` 读第一次：iOS 27 上它可能先于页面可见就运行，这里在 onAppear 里读，离开时作废。
    .onAppear {
      generation += 1
      configuration = DictionarySyncCenter.configuration()
      if configuration != nil { refresh(.opened) }
    }
    .onDisappear {
      generation += 1
      busy = false
    }
    // 不经本页发起的同步（App 回到前台的 `opened`、按时的 `periodic`）只更新 `center.lastRun`：页面开着时照它换上状态，否则「待发送」「上次同步」要离开再进来才变。键盘扩展记下的改动在 App 回到前台的那一次 `opened` 里反映出来。
    .onReceive(center.$lastRun) { run in
      if let run { adopt(run.status) }
    }
    .sheet(isPresented: $editing) {
      DictionarySyncWebDAVEditor(saved: configuration) { saved() }
    }
    .confirmationDialog(confirmationTitle, isPresented: Binding(
      get: { confirmation != nil }, set: { if !$0 { confirmation = nil } }), titleVisibility: .visible,
      presenting: confirmation) { item in
      Button(confirmationButton(item), role: .destructive) { confirm(item) }
      Button("取消", role: .cancel) { confirmation = nil }
    } message: { item in
      Text(confirmationMessage(item))
    }
  }

  // MARK: - 网盘

  private var webdavGroup: some View {
    VStack(alignment: .leading, spacing: 7) {
      DesignSectionHeader(subject: "WebDAV 网盘", info: IOSDictionarySyncPolicy.note) { SettingsGroupHeader(title: "WebDAV 网盘") }
        .padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        if let configuration {
          row("服务器地址", CloudClipboardSyncModeSection.shorten(configuration.url), identifier: "dictionarySyncURL")
          DesignDivider()
          row("账号", CloudClipboardSyncModeSection.shorten(configuration.username), identifier: "dictionarySyncUsername")
          DesignDivider()
          row("应用密码", configuration.hasPassword ? "已填写" : "未填写", identifier: "dictionarySyncPassword")
        } else {
          Button { editing = true } label: {
            DesignNavRowLabel(title: "设置 WebDAV 网盘", info: "坚果云、Nextcloud 等支持 WebDAV 的网盘都可以，不需要登录水杉账号。")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("dictionarySyncSetUp")
        }
      }
      .disabled(working)
    }
  }

  private func row(_ title: String, _ value: String, identifier: String) -> some View {
    Button { editing = true } label: { DesignNavRowLabel(title: title, value: value) }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier(identifier)
  }

  // MARK: - 状态

  private var statusGroup: some View {
    VStack(alignment: .leading, spacing: 7) {
      SettingsGroupHeader(title: "同步状态").padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        valueRow("上次同步", IOSDictionarySyncPolicy.lastSuccess(status?.last_success_ms), identifier: "dictionarySyncLastSuccess")
        DesignDivider()
        valueRow("共享词库", status.map { "\($0.words) 个词条" } ?? "—", identifier: "dictionarySyncWords")
        DesignDivider()
        valueRow("待发送", status.map { "\($0.pending_upload)" } ?? "—", identifier: "dictionarySyncPendingUpload")
        DesignDivider()
        valueRow("待写入本机", status.map { "\($0.pending_apply)" } ?? "—", identifier: "dictionarySyncPendingApply")
        if let budgetWait = IOSDictionarySyncPolicy.budgetWait(status?.budget_wait_until) {
          DesignDivider()
          Text(budgetWait)
            .designSubtext()
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 16).padding(.vertical, 12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityIdentifier("dictionarySyncBudgetWait")
        }
        if let lastError {
          DesignDivider()
          Text(IOSDictionarySyncPolicy.failureMessage(lastError))
            .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.danger)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 16).padding(.vertical, 12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityIdentifier("dictionarySyncError")
        }
        if lastError == "dictionary_sync_rollback" {
          DesignDivider()
          Button("确认网盘现状并恢复同步") { confirmation = .trustRemote }
            .disabled(working)
            .padding(16)
        }
        DesignDivider()
        Button { refresh(.manual) } label: {
          HStack {
            Text("立即同步").font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.accent)
            Spacer()
            Image(systemName: "arrow.triangle.2.circlepath").foregroundStyle(MetasequoiaTheme.accent).accessibilityHidden(true)
          }
          .padding(.horizontal, 16)
          .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
          .contentShape(Rectangle())
        }
        .buttonStyle(PressFillButtonStyle())
        .disabled(working)
        .accessibilityIdentifier("dictionarySyncNow")
      }
      if let footer = statusFooter {
        Text(footer).designSubtext()
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, 16)
      }
    }
  }

  /// 状态卡下面的说明：远端改动这一轮没写进本机（键盘正占着词库）、时钟不准。
  private var statusFooter: String? {
    var lines: [String] = []
    if center.lastRun?.report?.deferred == true || (status?.pending_apply ?? 0) > 0 {
      lines.append("键盘正在使用词库时，收到的词条会等它收起后再写入本机。")
    }
    if status?.clock_skew == true {
      lines.append("收到过时间比本机快一天以上的改动，请检查各设备的日期和时间是否准确。")
    }
    return lines.isEmpty ? nil : lines.joined(separator: "\n")
  }

  private func valueRow(_ title: String, _ value: String, identifier: String) -> some View {
    HStack(spacing: 12) {
      Text(title).font(.system(size: 16)).foregroundStyle(.primary)
      Spacer(minLength: 8)
      Text(value).font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.sub).multilineTextAlignment(.trailing)
    }
    .padding(.horizontal, 16)
    .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
    .accessibilityElement(children: .combine)
    .accessibilityIdentifier(identifier)
  }

  /// 本机等待批准：核对码和去哪里批准。
  private var pendingCard: some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      VStack(alignment: .leading, spacing: 10) {
        Text("本机正在等待批准").font(.system(size: 16, weight: .semibold)).foregroundStyle(.primary)
        if let ownCode {
          Text(ownCode).font(.system(size: 24, weight: .semibold, design: .monospaced)).foregroundStyle(.primary)
            .textSelection(.enabled)
            .accessibilityLabel("核对码 \(ownCode)")
            .accessibilityIdentifier("dictionarySyncOwnCode")
        }
        Text("在一台已经在同步词库的设备上打开词库同步，确认那里显示的核对码与上面相同，再点「允许」。批准后点「刷新」，本机的词条就会开始同步。")
          .designSubtext()
          .fixedSize(horizontal: false, vertical: true)
        Button("刷新") { refresh(.manual) }
          .font(.system(size: 15))
          .disabled(working)
          .accessibilityIdentifier("dictionarySyncPendingRefresh")
      }
      .padding(16)
      .frame(maxWidth: .infinity, alignment: .leading)
    }
  }

  /// 被挡下的大批删除：同步删除，或者从网盘把这些词写回本机。
  private func heldDeletionsCard(_ count: Int) -> some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      VStack(alignment: .leading, spacing: 10) {
        Text("有一批删除还没有同步").font(.system(size: 16, weight: .semibold)).foregroundStyle(.primary)
        Text(IOSDictionarySyncPolicy.heldDeletionsMessage(count))
          .designSubtext()
          .fixedSize(horizontal: false, vertical: true)
        HStack(spacing: 20) {
          Button("从网盘恢复") { refresh(.manual, restoreDeletions: true) }
            .fontWeight(.semibold)
            .accessibilityIdentifier("dictionarySyncRestoreDeletions")
          Spacer()
          Button("同步删除", role: .destructive) { confirmation = .confirmDeletions(count) }
            .accessibilityIdentifier("dictionarySyncConfirmDeletions")
        }
        .font(.system(size: 15))
        .disabled(working)
      }
      .padding(16)
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .accessibilityIdentifier("dictionarySyncHeldDeletions")
  }

  // MARK: - 设备

  private var approvalsGroup: some View {
    VStack(alignment: .leading, spacing: 7) {
      SettingsGroupHeader(title: "等待批准的设备").padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        ForEach(Array(awaitingApproval.enumerated()), id: \.element.id) { index, device in
          if index > 0 { DesignDivider() }
          VStack(alignment: .leading, spacing: 8) {
            Text(Self.name(device)).font(.system(size: 15)).foregroundStyle(.primary)
            Text(device.code).font(.system(size: 17, weight: .semibold, design: .monospaced)).foregroundStyle(.primary)
              .accessibilityIdentifier("dictionarySyncDeviceCode")
            Text("只在那台设备上显示的核对码与这里相同时才点「允许」。")
              .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
            HStack(spacing: 20) {
              Spacer()
              Button("拒绝") { confirmation = .reject(device) }
                .foregroundStyle(MetasequoiaTheme.danger)
              Button("允许") { approve(device) }
                .fontWeight(.semibold)
                .foregroundStyle(MetasequoiaTheme.accent)
                .accessibilityIdentifier("dictionarySyncApprove")
            }
            .font(.system(size: 15))
            .disabled(working)
          }
          .padding(16)
          .frame(maxWidth: .infinity, alignment: .leading)
        }
      }
    }
  }

  private var devicesGroup: some View {
    VStack(alignment: .leading, spacing: 7) {
      SettingsGroupHeader(title: "设备").padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        ForEach(Array(deviceList.enumerated()), id: \.element.id) { index, device in
          if index > 0 { DesignDivider() }
          HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
              Text(Self.name(device) + (device.current ? "（本机）" : "")).font(.system(size: 15)).foregroundStyle(.primary)
              Text(device.approved ? "已批准" : "等待批准").font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
            }
            Spacer(minLength: 8)
            if !device.current && !isPending {
              Button(device.approved ? "移除" : "拒绝") {
                confirmation = device.approved ? .remove(device) : .reject(device)
              }
              .font(.system(size: 15))
              .foregroundStyle(MetasequoiaTheme.danger)
              .disabled(working)
            }
          }
          .padding(.horizontal, 16)
          .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
          .accessibilityElement(children: .contain)
          .accessibilityIdentifier("dictionarySyncDevice")
        }
      }
    }
  }

  private static func name(_ device: BackendAccountClient.ClipboardDevice) -> String {
    device.name.isEmpty ? "未命名设备" : device.name
  }

  /// 很少用到的几项。
  private var maintenanceGroup: some View {
    VStack(alignment: .leading, spacing: 7) {
      SettingsGroupHeader(title: "其他").padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        dangerRow("重置密钥", identifier: "dictionarySyncReset") { confirmation = .reset }
        DesignDivider()
        dangerRow("以本机为准重新开始", identifier: "dictionarySyncRestart") { confirmation = .restart }
        DesignDivider()
        dangerRow("停止同步", identifier: "dictionarySyncDisable") { confirmation = .disable }
      }
      .disabled(working)
      Text("「重置密钥」和「以本机为准重新开始」只在已获批准的设备都用不了时使用，两者都不会删除任何设备上的词条。")
        .designSubtext()
        .fixedSize(horizontal: false, vertical: true)
        .padding(.horizontal, 16)
    }
  }

  private func dangerRow(_ title: String, identifier: String, action: @escaping () -> Void) -> some View {
    Button(action: action) {
      Text(title).font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.danger)
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
        .contentShape(Rectangle())
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityIdentifier(identifier)
  }

  // MARK: - 确认

  private var confirmationTitle: String {
    switch confirmation {
    case .confirmDeletions(let count): return "同步删除这 \(count) 个词？"
    case .reset: return "重置词库同步的密钥？"
    case .trustRemote: return "确认网盘现状并恢复同步？"
    case .restart: return "以本机为准重新开始？"
    case .disable: return "停止词库同步？"
    case .remove(let device): return "移除「\(Self.name(device))」？"
    case .reject(let device): return "拒绝「\(Self.name(device))」？"
    case nil: return ""
    }
  }

  private func confirmationButton(_ item: Confirmation) -> String {
    switch item {
    case .confirmDeletions: return "同步删除"
    case .reset: return "重置"
    case .trustRemote: return "恢复同步"
    case .restart: return "重新开始"
    case .disable: return "停止同步"
    case .remove: return "移除"
    case .reject: return "拒绝"
    }
  }

  private func confirmationMessage(_ item: Confirmation) -> String {
    switch item {
    case .confirmDeletions(let count):
      return "这 \(count) 个词会在所有同步词库的设备上删除。若是本机词库被重置或损坏，请改选「从网盘恢复」。"
    case .reset:
      return "会换一把只给本机的新密钥，其他所有设备都要在这里重新批准才能继续同步；网盘上的记录随后用新密钥重写，任何设备上的词条都不会因此删除。"
    case .trustRemote:
      return "仅在你自己清理过网盘文件夹时使用。保留当前本机词条，但忘掉同步位置和待发改动；未上传的删除可能从网盘重新合并回来。"
    case .restart:
      return "网盘上的词库会以本机的词条重新开始，其他设备获准后会把它们的词合并进来，不会因此删词。只在持有旧密钥的设备都不在了时使用。"
    case .disable:
      return "会清除本机保存的网盘地址、账号和应用密码，不再同步。网盘上的记录和本机的词条都不受影响。"
    case .remove:
      return "移除后这台设备不能再读取同步的词库。为此会换一把新的密钥，其他设备都要重新批准；移除之后建议在网盘上换一个应用密码。"
    case .reject:
      return "拒绝后这台设备的登记会被删除；它下次同步时会重新请求批准。"
    }
  }

  @MainActor private func confirm(_ item: Confirmation) {
    confirmation = nil
    switch item {
    case .confirmDeletions: refresh(.manual, confirmDeletions: true)
    case .reset: change { sync in try await sync.reset() }
    case .trustRemote: trustRemote()
    case .restart: restart()
    case .disable: disable()
    case .remove(let device): change { sync in try await sync.remove(device.device_id) }
    case .reject(let device): deviceAction { sync in try await sync.reject(device.device_id) }
    }
  }

  // MARK: - 请求

  /// 跑一次同步，再读设备列表。同步失败（例如等待批准、凭据不对）时状态照样更新，错误写在状态卡里。
  @MainActor private func refresh(_ trigger: IOSDictionarySync.Trigger, confirmDeletions: Bool = false,
                                  restoreDeletions: Bool = false) {
    let current = generation
    busy = true
    message = nil
    Task { @MainActor in
      defer { if current == generation { busy = false } }
      do {
        let run = try await center.run(trigger, confirmDeletions: confirmDeletions, restoreDeletions: restoreDeletions)
        guard current == generation else { return }
        status = run.status
        if trigger == .manual, run.error == nil { ToastCenter.shared.show("词库已同步") }
        // 还没登记过（同步失败在登记之前）时不去读设备，免得为一个已知的错误再发一轮请求。
        guard run.status.device_id != nil else { return }
        let list = try await center.perform { sync in try await sync.devices() }
        guard current == generation else { return }
        devices = list
      } catch let error as IOSDictionarySyncFailure {
        guard current == generation else { return }
        if status == nil { status = try? await center.perform { sync in try await sync.status() } }
        message = error.localizedDescription
      } catch {
        guard current == generation else { return }
        message = IOSDictionarySyncPolicy.failureMessage("")
      }
    }
  }

  /// 换上一次同步带回的状态，不论是谁发起的。本机的批准状态因此变了而设备列表还是旧的（例如在另一台设备上允许之后，定时的那一次同步先取到了密钥），就再读一次设备，免得「本机正在等待批准」一直留着；本页自己的请求在途时不读，它跑完会自己读。
  @MainActor private func adopt(_ next: IOSDictionarySync.Status) {
    status = next
    guard !busy, let devices, next.device_id != nil, devices.isPending != next.isPending else { return }
    let current = generation
    Task { @MainActor in
      guard let list = try? await center.perform({ sync in try await sync.devices() }), current == generation else { return }
      self.devices = list
    }
  }

  @MainActor private func approve(_ device: BackendAccountClient.ClipboardDevice) {
    deviceAction { sync in try await sync.approve(device.device_id) }
  }

  /// 批准、拒绝：结果就是设备状态。
  @MainActor private func deviceAction(_ body: @escaping @Sendable (IOSDictionarySync) async throws -> BackendAccountClient.ClipboardDevices) {
    perform { list in devices = list } body: { sync in try await body(sync) }
  }

  /// 移除和重置：设备状态加上随后那一次同步。
  @MainActor private func change(_ body: @escaping @Sendable (IOSDictionarySync) async throws -> IOSDictionarySync.DeviceChange) {
    perform { result in
      devices = result.devices
      status = result.run.status
      center.record(result.run)
      if let error = result.run.error { message = IOSDictionarySyncPolicy.failureMessage(error) }
    } body: { sync in try await body(sync) }
  }

  @MainActor private func trustRemote() {
    guard let configuration else { return }
    perform { (run: IOSDictionarySync.Run) in
      status = run.status
      center.record(run)
    } body: { sync in
      try await sync.probe(url: configuration.url, username: configuration.username,
                           password: configuration.savedPassword, trustRemote: true)
      return try await sync.run(.manual)
    }
  }

  @MainActor private func restart() {
    perform { value in
      status = value
      ToastCenter.shared.show("已以本机为准重新开始")
    } body: { sync in try await sync.restart() }
  }

  @MainActor private func perform<T: Sendable>(_ apply: @escaping @MainActor (T) -> Void,
                                               body: @escaping @Sendable (IOSDictionarySync) async throws -> T) {
    let current = generation
    busy = true
    message = nil
    Task { @MainActor in
      defer { if current == generation { busy = false } }
      do {
        let value = try await center.perform(body)
        guard current == generation else { return }
        apply(value)
      } catch let error as IOSDictionarySyncFailure {
        guard current == generation else { return }
        message = error.localizedDescription
      } catch {
        guard current == generation else { return }
        message = IOSDictionarySyncPolicy.failureMessage("")
      }
    }
  }

  /// 停止同步：删掉偏好里的网盘，网盘上的记录、本机的同步状态和密钥串都不动（再配回同一个网盘和账号时还是同一台设备）。
  @MainActor private func disable() {
    let saved = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
      preferences.removeValue(forKey: IOSDictionarySyncPolicy.preference)
    }
    guard saved else { message = "保存失败，请重试"; return }
    center.forget()
    configuration = nil
    status = nil
    devices = nil
    ToastCenter.shared.show("已停止词库同步")
  }

  /// 填写页保存之后：配置换了，按已保存配置同步一次；测试连接不改同步状态。
  @MainActor private func saved() {
    center.forget()
    configuration = DictionarySyncCenter.configuration()
    status = nil
    devices = nil
    if configuration != nil { refresh(.manual) }
  }
}

/// 填写词库同步的 WebDAV 网盘：服务器地址、账号和应用密码。三项按共享规则当场检查（与云剪贴板的网盘相同），不合规时在输入框下说明原因并禁用「测试连接」和「保存」。已经配置过时应用密码框可以留空，表示保留已存的应用密码，已存的值不回显。「测试连接」只测不存；「保存」先测，通过才写进偏好，没通过时填写页和草稿都留着，原因写在下面。
struct DictionarySyncWebDAVEditor: View {
  let onSaved: () -> Void
  private let savedURL: String
  private let savedUsername: String
  private let savedPassword: String
  @Environment(\.dismiss) private var dismiss
  @State private var url: String
  @State private var username: String
  @State private var password = ""
  @State private var probe = Probe.idle
  /// 草稿每改一次加一，旧的「测试连接」结果回来时据此丢弃，不让它说中新的草稿。
  @State private var draft = 0
  @State private var task: Task<Void, Never>?

  enum Probe: Equatable {
    case idle, probing, passed, saving
    case failed(String)
  }

  init(saved: IOSDictionarySyncPolicy.Configuration?, onSaved: @escaping () -> Void) {
    self.onSaved = onSaved
    savedURL = saved?.url ?? ""
    savedUsername = saved?.username ?? ""
    savedPassword = saved?.savedPassword ?? ""
    _url = State(initialValue: savedURL)
    _username = State(initialValue: savedUsername)
  }

  private var trimmedURL: String { url.trimmingCharacters(in: .whitespacesAndNewlines) }
  private var trimmedUsername: String { username.trimmingCharacters(in: .whitespacesAndNewlines) }
  /// 应用密码原样使用，不去掉首尾空白：有的网盘允许它们出现在密码里。
  private var effectivePassword: String { password.isEmpty ? savedPassword : password }
  private var urlIssue: String? { IOSCloudClipboardRelayPolicy.webdavURLIssue(trimmedURL) }
  private var usernameIssue: String? { IOSCloudClipboardRelayPolicy.usernameIssue(trimmedUsername) }
  private var passwordIssue: String? {
    password.isEmpty && !savedPassword.isEmpty ? nil : IOSCloudClipboardRelayPolicy.passwordIssue(password)
  }
  private var valid: Bool { urlIssue == nil && usernameIssue == nil && passwordIssue == nil }
  private var busy: Bool { probe == .probing || probe == .saving }
  private var canProbe: Bool { valid && !busy }
  private var changed: Bool {
    trimmedURL != savedURL || trimmedUsername != savedUsername || effectivePassword != savedPassword
  }
  private var canSave: Bool { canProbe && changed }

  var body: some View {
    NavigationStack {
      Form {
        Section {
          TextField("https://dav.jianguoyun.com/dav/", text: edited($url))
            .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
            .accessibilityIdentifier("dictionarySyncURLField")
        } header: {
          DesignSectionHeader(subject: "服务器地址", info: "坚果云填 https://dav.jianguoyun.com/dav/ 即可，水杉会在里面自动建 msime-dictionary 文件夹，不用自己先建；其他网盘填它给的 WebDAV 地址。必须是 HTTPS，证书要由公网信任的机构签发。可以与云剪贴板用同一个网盘。") { SettingsGroupHeader(title: "服务器地址") }
        } footer: {
          DictionarySyncFieldFooter(issue: url.isEmpty ? nil : urlIssue)
        }
        .designRow()
        Section {
          TextField("账号", text: edited($username))
            .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.emailAddress)
            .accessibilityIdentifier("dictionarySyncUsernameField")
        } header: {
          DesignSectionHeader(subject: "账号", info: "网盘的登录账号，坚果云是登录邮箱。换了地址或账号就是一台新设备，要在其他设备上重新允许。") { SettingsGroupHeader(title: "账号") }
        } footer: {
          DictionarySyncFieldFooter(issue: username.isEmpty ? nil : usernameIssue)
        }
        .designRow()
        Section {
          SecureField(savedPassword.isEmpty ? "应用密码" : "留空以保留已保存的应用密码", text: edited($password))
            .textInputAutocapitalization(.never).autocorrectionDisabled()
            .accessibilityIdentifier("dictionarySyncPasswordField")
        } header: {
          DesignSectionHeader(subject: "应用密码", info: "在网盘的安全设置里为第三方应用生成的密码，不是登录密码。只存在本机的设置里，不会随设置文件导出或账号同步。") { SettingsGroupHeader(title: "应用密码") }
        } footer: {
          DictionarySyncFieldFooter(issue: password.isEmpty ? nil : passwordIssue)
        }
        .designRow()
        Section {
          Button(probe == .probing ? "正在测试连接……" : "测试连接") { start(saving: false) }
            .disabled(!canProbe)
            .accessibilityIdentifier("dictionarySyncProbe")
          if let status {
            Text(status.text).font(.footnote)
              .foregroundStyle(status.failed ? MetasequoiaTheme.danger : MetasequoiaTheme.sub)
              .fixedSize(horizontal: false, vertical: true)
              .accessibilityIdentifier("dictionarySyncProbeStatus")
          }
        } footer: {
          Text(IOSDictionarySyncPolicy.note)
        }
        .designRow()
      }
      .designPage()
      .fittedSheet()
      .navigationTitle("词库同步网盘")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("取消") { close() } }
        ToolbarItem(placement: .confirmationAction) {
          Button("保存") { start(saving: true) }
            .fontWeight(.semibold)
            .disabled(!canSave)
            .accessibilityIdentifier("dictionarySyncSave")
        }
      }
    }
    .tint(MetasequoiaTheme.accent)
    .interactiveDismissDisabled(busy)
    .onDisappear { task?.cancel(); task = nil }
  }

  private var status: (text: String, failed: Bool)? {
    switch probe {
    case .idle, .probing: return nil
    case .passed: return ("连接正常，可以保存。", false)
    case .saving: return ("连接正常，正在保存……", false)
    case .failed(let message): return (message, true)
    }
  }

  /// 改了任何一项就收起上一次测试的结果，免得「连接正常」留在改过的内容下面。
  private func edited(_ binding: Binding<String>) -> Binding<String> {
    Binding(get: { binding.wrappedValue }, set: { value in
      guard value != binding.wrappedValue else { return }
      binding.wrappedValue = value
      draft += 1
      if !busy { probe = .idle }
    })
  }

  /// 跑一次测试连接；`saving` 时通过了就接着保存，否则只把结果写在下面。
  private func start(saving: Bool) {
    guard saving ? canSave : canProbe else { return }
    let address = trimmedURL, account = trimmedUsername, secret = effectivePassword, generation = draft
    probe = .probing
    task = Task { @MainActor in
      var failure: String?
      do {
        try await DictionarySyncCenter.shared.perform { sync in
          try await sync.probe(url: address, username: account, password: secret)
        }
      } catch let error as IOSDictionarySyncFailure {
        failure = IOSDictionarySyncPolicy.probeMessage(error.code)
      } catch {
        failure = IOSDictionarySyncPolicy.probeMessage("")
      }
      guard !Task.isCancelled else { return }
      guard generation == draft else { probe = .idle; return }
      if let failure { probe = .failed(failure); return }
      guard saving else { probe = .passed; return }
      probe = .saving
      let webdav: [String: Any] = ["url": address, "username": account, "password": secret]
      let saved = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
        preferences[IOSDictionarySyncPolicy.preference] = webdav
      }
      guard saved else { probe = .failed("保存失败，请重试"); return }
      ToastCenter.shared.show("已保存，开始同步词库")
      close()
      onSaved()
    }
  }

  /// 关掉填写页之前清空应用密码框：框里还有内容时系统会弹出「保存密码？」，应用密码不该进系统的密码库。
  private func close() {
    task?.cancel()
    password = ""
    dismiss()
  }
}

/// 输入框下面的一段：当场检查的问题（红字）在上，固定的说明在下。
/// 字段下方只留校验结果；字段的说明在分区标题旁的「?」里。
private struct DictionarySyncFieldFooter: View {
  let issue: String?

  var body: some View {
    if let issue {
      Text(issue).foregroundStyle(MetasequoiaTheme.danger).accessibilityIdentifier("dictionarySyncFieldIssue")
    }
  }
}
