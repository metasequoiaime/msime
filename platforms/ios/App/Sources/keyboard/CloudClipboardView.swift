import SwiftUI
import UIKit

/// 云剪贴板页。最上面是「云剪贴板同步方式」（`CloudClipboardSyncModeSection`）：水杉账号、自建中继或 WebDAV 网盘，选了后两者就只走它，不需要登录。
///
/// 水杉账号经 Rust 的 `msime_client_account_cloud_clipboard`（#8201 迁移计划 4c）：端到端加密、令牌和密钥串（与键盘共用的钥匙串项 `cloud-clipboard-keystore`）都在 Rust 里，键盘扩展仍用 Swift 的传输和同一份密钥串，所以两边是同一台设备。中继和网盘经 `IOSCloudClipboardRelay`：驱动在 Swift 里跑，密钥串在 App Group 访问组的钥匙串项 `cloud-clipboard-relay-keystore` 里，键盘读写同一项。页面只按第一次读到的那一方绑定（账号的 user.id，或中继、网盘的绑定）：C 上拿不到会话代次，同一个账号的令牌轮换和重新登录不作废页面，换号、退出、换中继、改同步方式作废。
struct CloudClipboardView: View {
  /// 默认是 App 启动时注册的那一份；测试和预览传别的。
  var rust: @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() }
  @State private var accountID: String?
  @State private var enabled = false
  @State private var loaded = false
  @State private var retention: Int?
  @State private var items: [BackendAccountClient.ClipboardItem] = []
  @State private var search = ""
  @State private var text = ""
  @State private var busy = false
  @State private var message: String?
  @State private var confirmsClear = false
  @State private var addsText = false
  @State private var pending: Task<Void, Never>?
  /// 让离开页面后仍在取消中的旧请求不能释放新页面的忙碌状态或覆盖新数据。
  @State private var request = ViewRequestGeneration()
  /// 列表带回的端到端加密状态；`pending` 时本机还在等待其他设备批准。
  @State private var e2eStatus: BackendAccountClient.ClipboardE2EStatus?
  /// 账号下登记的设备，云剪贴板开着时与列表一起刷新。
  @State private var devices: BackendAccountClient.ClipboardDevices?
  @State private var deviceConfirmation: DeviceConfirmation?
  /// 下面的开关、列表和设备在哪：经中继时是 `.relay` 或 `.webdav`，经水杉账号时为 nil。
  @State private var relay: IOSCloudClipboardRelay.Backend?
  /// 偏好里的同步方式；nil 是水杉账号。
  @State private var syncConfiguration: IOSCloudClipboardRelay.Configuration?
  /// 同步方式读到了没有；偏好读不了时不画那张卡。
  @State private var syncModeRead = false
  /// 走水杉账号而没有登录。
  @State private var signedOut = false
  @State private var showsLogin = false

  private enum DeviceConfirmation: Equatable {
    case reset
    case remove(BackendAccountClient.ClipboardDevice)
    case reject(BackendAccountClient.ClipboardDevice)
  }

  /// 本机还在等待批准：看不到条目，也不能上传。
  private var isPending: Bool { enabled && e2eStatus?.status == "pending" }
  private var code: String? { devices?.code ?? e2eStatus?.code }
  private var awaitingApproval: [BackendAccountClient.ClipboardDevice] { isPending ? [] : devices?.awaitingApproval ?? [] }

  private static let retentionChoices: [(title: String, value: Int)] = [("1 天", 1), ("7 天", 7), ("30 天", 30), ("一直", 0)]

  private static let relative: RelativeDateTimeFormatter = {
    let formatter = RelativeDateTimeFormatter()
    formatter.locale = Locale(identifier: "zh-Hans")
    formatter.unitsStyle = .full
    return formatter
  }()

  private var canUpload: Bool {
    !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && text.utf16.count <= 4000
  }

  /// 置顶的排在前面；两部分各自保持服务端的顺序，最新的在前。
  private var orderedItems: [BackendAccountClient.ClipboardItem] {
    items.filter { $0.pinned == true } + items.filter { $0.pinned != true }
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 24) {
        if syncModeRead {
          CloudClipboardSyncModeSection(configuration: syncConfiguration, disabled: busy) { syncModeChanged() }
        }
        if loaded {
          settingsCard
          if enabled {
            if isPending {
              pendingCard
            } else {
              if !awaitingApproval.isEmpty { approvals }
              recent
            }
            deviceList
          }
        } else if signedOut {
          signedOutCard
        } else if !busy {
          DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
            Button { run { _ in nil } } label: {
              HStack {
                VStack(alignment: .leading, spacing: 2) {
                  Text("重试").font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.accent)
                  Text("没能读到云端内容").font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
                }
                Spacer()
                Image(systemName: "arrow.clockwise").foregroundStyle(MetasequoiaTheme.accent).accessibilityHidden(true)
              }
              .padding(.horizontal, 16)
              .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
              .contentShape(Rectangle())
            }
            .buttonStyle(PressFillButtonStyle())
            .accessibilityIdentifier("cloudClipboardRetry")
          }
        }
      }
      .padding(.horizontal, 16).padding(.top, 8).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .designCanvasBackground()
    .searchable(text: $search, prompt: "搜索云端内容")
    .onSubmit(of: .search) { run { _ in nil } }
    .settingsStatus(busy: busy, message: message)
    .disabled(busy)
    .navigationTitle("云剪贴板")
    .navigationBarTitleDisplayMode(.inline)
    .toolbar {
      if loaded && enabled && !isPending {
        ToolbarItem(placement: .topBarTrailing) {
          Button { addsText = true } label: { Image(systemName: "plus") }
            .accessibilityLabel("添加内容")
            .accessibilityIdentifier("cloudClipboardAdd")
        }
      }
    }
    .sheet(isPresented: $addsText) { addSheet }
    .task { run { _ in nil } }
    // `.task` 可能先于下面的 onAppear 运行，那时页面还不算可见，`run` 什么也不做；所以出现时还没有内容就在这里再读一次（`run` 在已有请求时不重复发）。
    .trackViewRequest(request: $request, pending: $pending, onAppear: {
      readSyncMode()
      if !loaded { run { _ in nil } }
    }, onDisappear: {
      busy = false
      items = []; text = ""; devices = nil; e2eStatus = nil
    })
    .confirmationDialog("清空云剪贴板？", isPresented: $confirmsClear, titleVisibility: .visible) {
      Button("确认清空", role: .destructive) {
        run { request in _ = try await request(["operation": "clear"]); return nil }
      }
      Button("取消", role: .cancel) { }
    }
    .confirmationDialog(confirmationTitle, isPresented: Binding(
      get: { deviceConfirmation != nil }, set: { if !$0 { deviceConfirmation = nil } }), titleVisibility: .visible) {
      Button(confirmationButton, role: .destructive) {
        switch deviceConfirmation {
        case .reset: change(.reset)
        case .remove(let device): change(.remove(deviceID: device.device_id))
        case .reject(let device): change(.reject(deviceID: device.device_id))
        case nil: break
        }
        deviceConfirmation = nil
      }
      Button("取消", role: .cancel) { deviceConfirmation = nil }
    } message: { Text(confirmationMessage) }
  }

  /// 走水杉账号而没有登录：说明登录后能做什么，也可以在上面改用中继或网盘。
  private var signedOutCard: some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      VStack(alignment: .leading, spacing: 10) {
        Text("登录水杉账号后，复制的文字可以在你的设备之间同步，内容在本机加密，水杉云只保存密文。不想登录的话，可以在上面把同步方式改成自建中继或 WebDAV 网盘。")
          .designSubtext()
          .fixedSize(horizontal: false, vertical: true)
        Button("登录水杉账号") { showsLogin = true }
          .font(.system(size: 15))
          .accessibilityIdentifier("cloudClipboardSignIn")
      }
      .padding(16)
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .sheet(isPresented: $showsLogin, onDismiss: { run { _ in nil } }) { AccountLoginSheet() }
  }

  /// 本机等待批准：显示核对码，说明去哪里批准；所有设备都用不了时可以重置。
  private var pendingCard: some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      VStack(alignment: .leading, spacing: 10) {
        Text("本机正在等待批准").font(.system(size: 16, weight: .semibold)).foregroundStyle(.primary)
        if let code {
          Text(code).font(.system(size: 24, weight: .semibold, design: .monospaced)).foregroundStyle(.primary)
            .textSelection(.enabled)
            .accessibilityLabel("核对码 \(code)")
            .accessibilityIdentifier("cloudClipboardOwnCode")
        }
        Text("在一台已经能使用云剪贴板的设备上打开云剪贴板，确认那里显示的核对码与上面相同，再点「允许」。批准后点「刷新」即可读取和上传。")
          .designSubtext()
          .fixedSize(horizontal: false, vertical: true)
        HStack {
          Button("刷新") { run { _ in nil } }
            .accessibilityIdentifier("cloudClipboardPendingRefresh")
          Spacer()
          Button("重置云剪贴板", role: .destructive) { deviceConfirmation = .reset }
            .accessibilityIdentifier("cloudClipboardReset")
        }
        .font(.system(size: 15))
      }
      .padding(16)
      .frame(maxWidth: .infinity, alignment: .leading)
    }
  }

  /// 等待本机批准的其他设备。
  private var approvals: some View {
    VStack(alignment: .leading, spacing: 7) {
      SettingsGroupHeader(title: "等待批准的设备").padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        ForEach(Array(awaitingApproval.enumerated()), id: \.element.id) { index, device in
          if index > 0 { DesignDivider() }
          VStack(alignment: .leading, spacing: 8) {
            Text(Self.name(device)).font(.system(size: 15)).foregroundStyle(.primary)
            Text(device.code).font(.system(size: 17, weight: .semibold, design: .monospaced)).foregroundStyle(.primary)
            Text("只在那台设备上显示的核对码与这里相同时才点「允许」。")
              .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
            HStack(spacing: 20) {
              Spacer()
              Button("拒绝") { deviceConfirmation = .reject(device) }
                .foregroundStyle(MetasequoiaTheme.danger)
              Button("允许") { change(.approve(deviceID: device.device_id)) }
                .fontWeight(.semibold)
                .foregroundStyle(MetasequoiaTheme.accent)
                .accessibilityIdentifier("cloudClipboardApprove")
            }
            .font(.system(size: 15))
          }
          .padding(16)
          .frame(maxWidth: .infinity, alignment: .leading)
        }
      }
    }
  }

  /// 账号下的设备。移除一台已批准的设备会换钥，其余设备都要重新批准。
  @ViewBuilder private var deviceList: some View {
    if let list = devices?.devices, !list.isEmpty {
      VStack(alignment: .leading, spacing: 7) {
        SettingsGroupHeader(title: "设备").padding(.horizontal, 16)
        DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
          ForEach(Array(list.enumerated()), id: \.element.id) { index, device in
            if index > 0 { DesignDivider() }
            HStack(spacing: 12) {
              VStack(alignment: .leading, spacing: 2) {
                Text(Self.name(device) + (device.current ? "（本机）" : "")).font(.system(size: 15)).foregroundStyle(.primary)
                Text(device.approved ? "已批准" : "等待批准").font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
              }
              Spacer(minLength: 8)
              if !device.current && !isPending {
                Button(device.approved ? "移除" : "拒绝") {
                  deviceConfirmation = device.approved ? .remove(device) : .reject(device)
                }
                .font(.system(size: 15))
                .foregroundStyle(MetasequoiaTheme.danger)
              }
            }
            .padding(.horizontal, 16)
            .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
          }
        }
        if !isPending {
          Button("重置云剪贴板") { deviceConfirmation = .reset }
            .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.danger)
            .padding(.horizontal, 16)
        }
      }
    }
  }

  private static func name(_ device: BackendAccountClient.ClipboardDevice) -> String {
    device.name.isEmpty ? "未命名设备" : device.name
  }

  private var confirmationTitle: String {
    switch deviceConfirmation {
    case .reset: return "重置云剪贴板？"
    case .remove(let device): return "移除「\(Self.name(device))」？"
    case .reject(let device): return "拒绝「\(Self.name(device))」？"
    case nil: return ""
    }
  }
  private var confirmationButton: String {
    switch deviceConfirmation {
    case .reset: return "重置"
    case .remove: return "移除"
    case .reject, nil: return "拒绝"
    }
  }
  private var confirmationMessage: String {
    switch deviceConfirmation {
    case .reset: return "重置会换一把新的加密密钥并删除云端的全部条目。其他所有设备都要重新批准才能继续使用云剪贴板。只在已获批准的设备都用不了时这样做。"
    case .remove: return "移除后这台设备不能再读取云剪贴板。为此会换一把新的加密密钥并删除云端的全部条目，其他设备都要重新批准。"
    case .reject: return "拒绝后这台设备的登记会被删除；它下次打开云剪贴板时会重新请求批准。"
    case nil: return ""
    }
  }

  /// 开关的说明，随内容存在哪里而变。
  private var switchSubtitle: String {
    switch relay {
    case .relay: return "在你批准的设备之间经自建中继同步，内容在本机加密，中继只保存密文；关闭即删除中继上的内容和设备登记"
    case .webdav: return "在你批准的设备之间经 WebDAV 网盘同步，内容在本机加密，网盘只保存密文；关闭即删除网盘上的内容和设备登记"
    case nil: return "在你登录并批准的设备之间同步，内容在本机加密，水杉云只保存密文；关闭即删除云端内容和设备登记"
    }
  }

  /// 读偏好里的同步方式；读不了时不画同步方式那张卡（内容那边会报同样的错）。读一个本机的小文件。
  @MainActor private func readSyncMode() {
    do {
      syncConfiguration = try IOSCloudClipboardRelay.currentConfiguration()
      syncModeRead = true
    } catch {
      syncModeRead = false
    }
  }

  /// 同步方式保存或改回之后：页面不再属于原来那一方，从头读。
  @MainActor private func syncModeChanged() {
    readSyncMode()
    accountID = nil
    items = []; devices = nil; e2eStatus = nil; loaded = false; signedOut = false
    run { _ in nil }
  }

  /// 开关，以及服务端返回保留期限时的保留期选项。
  private var settingsCard: some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      Toggle(isOn: Binding(
        get: { enabled },
        set: { on in run { request in _ = try await request(["operation": "set_enabled", "enabled": on]); return nil } }
      )) {
        VStack(alignment: .leading, spacing: 2) {
          Text("云剪贴板").font(.system(size: 16)).foregroundStyle(.primary)
          Text(switchSubtitle)
            .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
      .toggleStyle(.switch)
      .tint(MetasequoiaTheme.switchOn)
      .padding(.horizontal, 16).padding(.vertical, 10)
      .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
      .accessibilityIdentifier("cloudClipboardSwitch")
      if let retention {
        DesignDivider()
        HStack(spacing: 12) {
          Text("保留时长").font(.system(size: 16)).foregroundStyle(.primary)
          Spacer(minLength: 8)
          DesignSegmentedControl(
            items: Self.retentionChoices,
            selection: Binding(
              get: { retention },
              set: { days in
                guard days != retention else { return }
                run { request in _ = try await request(["operation": "set_retention", "days": days]); return nil }
              }
            ),
            identifierPrefix: "cloudClipboardRetention"
          )
          .frame(maxWidth: 230)
          .disabled(!enabled)
          .opacity(enabled ? 1 : 0.5)
        }
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("保留时长")
      }
    }
  }

  /// 最近：带「清空」的标题，下面是条目或空状态卡片。
  private var recent: some View {
    VStack(alignment: .leading, spacing: 7) {
      HStack {
        SettingsGroupHeader(title: "最近")
        Spacer()
        if !items.isEmpty {
          Button("清空") { confirmsClear = true }
            .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.accent)
            .accessibilityIdentifier("cloudClipboardClear")
        }
      }
      .padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        if items.isEmpty {
          emptyState
        } else {
          let ordered = orderedItems
          ForEach(Array(ordered.enumerated()), id: \.element.id) { index, item in
            if index > 0 { DesignDivider() }
            row(item)
          }
        }
      }
    }
  }

  private var emptyState: some View {
    VStack(spacing: 6) {
      if search.isEmpty {
        Text("还没有同步内容").font(.system(size: 15)).foregroundStyle(.primary)
        Text("在任一设备上复制文字，这里就会出现").designSubtext()
      } else {
        Text("没有匹配「\(search)」的内容").font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.sub)
      }
    }
    .multilineTextAlignment(.center)
    .frame(maxWidth: .infinity)
    .padding(.horizontal, 16).padding(.vertical, 36)
    .accessibilityElement(children: .combine)
  }

  private func row(_ item: BackendAccountClient.ClipboardItem) -> some View {
    let pinned = item.pinned == true
    return HStack(alignment: .top, spacing: 4) {
      Button {
        UIPasteboard.general.string = item.text
        ToastCenter.shared.show("已复制")
      } label: {
        VStack(alignment: .leading, spacing: 4) {
          Text(item.text).font(.system(size: 15)).foregroundStyle(.primary)
            .lineLimit(2).multilineTextAlignment(.leading)
          let line = meta(item)
          if !line.isEmpty {
            Text(line).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
          }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityHint("轻点复制")
      Button {
        run { request in _ = try await request(["operation": "pin", "id": item.id, "pinned": !pinned]); return nil }
      } label: {
        Image(systemName: pinned ? "pin.fill" : "pin").font(.system(size: 15))
          .foregroundStyle(pinned ? MetasequoiaTheme.accent : MetasequoiaTheme.sub)
          .frame(width: 30, height: 30)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityLabel(pinned ? "取消置顶" : "置顶")
      Button {
        run { request in _ = try await request(["operation": "delete", "id": item.id]); return nil }
      } label: {
        Image(systemName: "trash").font(.system(size: 15))
          .foregroundStyle(MetasequoiaTheme.sub)
          .frame(width: 30, height: 30)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityLabel("删除")
    }
    .padding(.leading, 16).padding(.trailing, 10).padding(.vertical, 12)
    .frame(maxWidth: .infinity, alignment: .leading)
    .accessibilityElement(children: .contain)
  }

  /// '已置顶 · <device> · <relative time>'，未知的部分省略。
  private func meta(_ item: BackendAccountClient.ClipboardItem) -> String {
    var parts: [String] = []
    if item.pinned == true { parts.append("已置顶") }
    if let device = item.device { parts.append(device) }
    if let date = CloudClipboardTimePolicy.date(item.updated_at) {
      parts.append(CloudClipboardTimePolicy.relative(date, formatter: Self.relative))
    }
    return parts.joined(separator: " · ")
  }

  /// 添加内容：手动上传一段文本的编辑器。
  private var addSheet: some View {
    NavigationStack {
      Form {
        Section {
          TextEditor(text: $text).frame(minHeight: 160).accessibilityIdentifier("cloudClipboardText")
        } footer: {
          VStack(alignment: .leading, spacing: 4) {
            Text("\(text.utf16.count) / 4000")
              .foregroundStyle(text.utf16.count > 4000 ? MetasequoiaTheme.danger : MetasequoiaTheme.sub)
              .monospacedDigit()
            Text("只上传你在这里明确添加的内容，不自动读取系统剪贴板。内容在本机加密后上传，最多保存 50 条。")
          }
        }
        .designRow()
      }
      .designPage()
      .fittedSheet()
      .navigationTitle("添加内容")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button("取消") { addsText = false }
        }
        ToolbarItem(placement: .confirmationAction) {
          Button("上传") {
            let value = text
            addsText = false
            run { request in
              _ = try await request(["operation": "add", "text": value])
              text = ""
              return nil
            }
          }
          .fontWeight(.semibold)
          .disabled(!canUpload || busy)
          .accessibilityIdentifier("cloudClipboardUpload")
        }
      }
    }
    .tint(MetasequoiaTheme.accent)
  }

  /// 批准、拒绝、移除一台设备或重置云剪贴板，结果就是设备状态。
  @MainActor private func change(_ operation: BackendAccountClient.ClipboardE2EOperation) {
    guard let action = IOSCloudClipboard.action(operation) else { return }
    run { request in try BackendRustAccount.decode(BackendAccountClient.ClipboardDevices.self, try await request(action)) }
  }

  /// 执行一个操作，然后重新读列表（`IOSCloudClipboard.refresh`）。
  @MainActor private func run(_ action: @escaping @MainActor @Sendable (IOSCloudClipboard.Request) async throws -> BackendAccountClient.ClipboardDevices?) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message,
                        pending: $pending) { id in await execute(action, requestID: id) }
  }
  @MainActor private func execute(_ action: @escaping @MainActor @Sendable (IOSCloudClipboard.Request) async throws -> BackendAccountClient.ClipboardDevices?,
                                  requestID id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      let clipboard = IOSCloudClipboard(account: try rust(), deviceName: IOSCloudClipboardDriver.deviceName(UIDevice.current))
      let result = try await clipboard.refresh(boundTo: accountID, search: search) { request in try await action(request) }
      try request.checkCurrent(id)
      accountID = result.accountID
      let page = result.page
      enabled = page.enabled; retention = page.retention_days; items = page.items; e2eStatus = page.e2e
      devices = result.devices
      relay = result.relay
      loaded = true; signedOut = false
    } catch is CancellationError {
      guard request.isCurrent(id) else { return }
      items = []; text = ""; loaded = false; devices = nil; e2eStatus = nil
    }
    catch is IOSCloudClipboard.SignedOut {
      guard request.isCurrent(id) else { return }
      items = []; text = ""; loaded = false; devices = nil; e2eStatus = nil; relay = nil; signedOut = true
    }
    catch let error as IOSCloudClipboardRelayFailure {
      guard request.isCurrent(id) else { return }
      // 本机等待批准时上传失败不清空页面，只提示去批准。
      if error.code != "clipboard_device_pending" { items = []; loaded = false }
      message = error.localizedDescription
    }
    catch let error as BackendRustClient.Failure {
      guard request.isCurrent(id) else { return }
      // 本机等待批准时上传失败不清空页面，只提示去批准。
      if error.code != "clipboard_device_pending" { items = []; loaded = false }
      message = error.localizedDescription
    }
    catch {
      guard request.isCurrent(id) else { return }
      items = []; loaded = false; message = "连接未完成，请检查网络后重试。"
    }
  }
}
