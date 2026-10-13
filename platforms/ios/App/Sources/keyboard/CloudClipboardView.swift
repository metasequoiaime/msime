import SwiftUI
import UIKit

struct CloudClipboardView: View {
  let session: BackendAccountSession
  let client: BackendAccountClient
  /// 端到端加密驱动和与键盘共用的密钥串。
  var e2e: BackendClipboardE2E? = IOSCloudClipboardDriver.e2e
  @State private var accountID: String?
  @State private var sessionID: UUID?
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
    .task { request.appear(); run { _ in nil } }
    .onDisappear {
      ViewRequestTask.disappear(request: $request, pending: $pending)
      busy = false
      items = []; text = ""; devices = nil; e2eStatus = nil
    }
    .confirmationDialog("清空云剪贴板？", isPresented: $confirmsClear, titleVisibility: .visible) {
      Button("确认清空", role: .destructive) {
        run { token in try await client.deleteClipboard(token: token); return nil }
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

  /// 开关，以及服务端返回保留期限时的保留期选项。
  private var settingsCard: some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      Toggle(isOn: Binding(
        get: { enabled },
        set: { on in run { token in try await client.setClipboardEnabled(on, token: token); return nil } }
      )) {
        VStack(alignment: .leading, spacing: 2) {
          Text("云剪贴板").font(.system(size: 16)).foregroundStyle(.primary)
          Text("在你登录并批准的设备之间同步，内容在本机加密，水杉云只保存密文；关闭即删除云端内容和设备登记")
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
                run { token in try await client.setClipboardRetention(days: days, token: token); return nil }
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
        run { token in try await client.setClipboardPinned(id: item.id, pinned: !pinned, token: token); return nil }
      } label: {
        Image(systemName: pinned ? "pin.fill" : "pin").font(.system(size: 15))
          .foregroundStyle(pinned ? MetasequoiaTheme.accent : MetasequoiaTheme.sub)
          .frame(width: 30, height: 30)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityLabel(pinned ? "取消置顶" : "置顶")
      Button {
        run { token in try await client.deleteClipboard(id: item.id, token: token); return nil }
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
      parts.append(Self.relative.localizedString(for: min(date, Date()), relativeTo: Date()))
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
            run { token in
              _ = try await client.addClipboard(value, token: token, e2e: e2e)
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
    .presentationDetents([.medium, .large])
  }

  /// 批准、拒绝、移除一台设备或重置云剪贴板。
  @MainActor private func change(_ operation: BackendAccountClient.ClipboardE2EOperation) {
    run { [client, e2e] token in try await client.clipboardDevices(operation, token: token, e2e: e2e) }
  }

  /// 执行一个操作，然后重新读列表；云剪贴板开着时再读一次设备状态（操作本身已经带回设备状态时不再读）。
  @MainActor private func run(_ action: @escaping @MainActor @Sendable (String) async throws -> BackendAccountClient.ClipboardDevices?) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message,
                        pending: $pending) { id in await execute(action, requestID: id) }
  }
  @MainActor private func execute(_ action: @MainActor @Sendable (String) async throws -> BackendAccountClient.ClipboardDevices?, requestID id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      let identity = try await session.credentials(matchingUserID: accountID, matchingSessionID: sessionID)
      try request.checkCurrent(id)
      accountID = identity.userID; sessionID = identity.sessionID
      var status = try await session.authenticated(matchingUserID: identity.userID,
                                                   matchingSessionID: identity.sessionID) { token in
        try await action(token)
      }.value
      try request.checkCurrent(id)
      let page = try await client.clipboard(search: search, e2e: e2e, session: session,
                                            matchingUserID: identity.userID,
                                            matchingSessionID: identity.sessionID)
      try request.checkCurrent(id)
      if page.enabled && status == nil {
        status = try await session.authenticated(matchingUserID: identity.userID,
                                                 matchingSessionID: identity.sessionID) { [client, e2e] token in
          try await client.clipboardDevices(token: token, e2e: e2e)
        }.value
      }
      try request.checkCurrent(id)
      try await session.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
      try request.checkCurrent(id)
      enabled = page.enabled; retention = page.retention_days; items = page.items; e2eStatus = page.e2e
      devices = page.enabled ? status : nil
      loaded = true
    } catch is CancellationError {
      guard request.isCurrent(id) else { return }
      items = []; text = ""; loaded = false; devices = nil; e2eStatus = nil
    }
    catch let error as BackendAccountClient.Failure {
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
