import AppKit
import SwiftUI

@MainActor
final class MacClipboardModel: ObservableObject {
  @Published var enabled = false
  @Published var loaded = false
  @Published var items: [BackendAccountClient.ClipboardItem] = []
  @Published var text = ""
  @Published var search = ""
  @Published var busy = false
  @Published var message: String?
  /// 列表带回的端到端加密状态；`pending` 时本机还在等待其他设备批准。
  @Published var e2e: BackendAccountClient.ClipboardE2EStatus?
  /// 账号下登记的设备，云剪贴板开着时与列表一起刷新。
  @Published var devices: BackendAccountClient.ClipboardDevices?
  private let accountID: String
  /// 云剪贴板经 Rust 的 `msime_client_account_cloud_clipboard`（#8201 迁移计划 4c）：端到端加密、密钥串（`account-session.json` 旁边的 `cloud-clipboard-keystore`）和令牌都在 Rust 里。nil 是宿主没有注册会话存储，每个操作都报 `rust_client_unavailable`。
  private let rust: BackendRustAccount?
  /// 本机登记端到端加密时给其他设备看的名字。
  private let deviceName: String
  private var pending: Task<Void, Never>?
  init(accountID: String, rust: BackendRustAccount? = .shared, deviceName: String = MacCloudClipboardDriver.deviceName) {
    self.accountID = accountID; self.rust = rust; self.deviceName = deviceName
  }

  /// 本机还在等待批准：看不到条目，也不能上传。
  var isPending: Bool { enabled && e2e?.status == "pending" }
  /// 本机的核对码，列表和设备状态里都有。
  var code: String? { devices?.code ?? e2e?.code }
  /// 等待本机批准的其他设备。
  var awaitingApproval: [BackendAccountClient.ClipboardDevice] { isPending ? [] : devices?.awaitingApproval ?? [] }

  private func request(_ action: [String: Any]) async throws -> Data {
    guard let rust else { throw BackendRustClient.Failure.unavailable }
    return try await rust.cloudClipboard(action, deviceName: deviceName)
  }
  /// 打开窗口时的账号还登录着才继续，退出或换了号就丢掉结果。C 上拿不到会话代次，只按 user.id 绑定：同一个账号的令牌轮换和重新登录不作废（迁移差异 M29）。
  private func requireAccount() async throws {
    guard let rust else { throw BackendRustClient.Failure.unavailable }
    guard try await rust.status()?.id == accountID else { throw CancellationError() }
    try Task.checkCancellation()
  }
  /// 执行一个操作，然后重新读列表；云剪贴板开着时再读一次设备状态（操作本身已经带回设备状态时不再读）。
  func run(_ action: @escaping @MainActor () async throws -> BackendAccountClient.ClipboardDevices?) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do {
        try await requireAccount()
        var status = try await action()
        let page = try BackendRustAccount.decode(BackendAccountClient.ClipboardPage.self,
                                                 try await request(["operation": "list", "search": search]))
        if page.enabled && status == nil {
          status = try BackendRustAccount.decode(BackendAccountClient.ClipboardDevices.self,
                                                 try await request(["operation": "devices"]))
        }
        try await requireAccount()
        enabled = page.enabled; items = page.items; self.e2e = page.e2e
        devices = page.enabled ? status : nil
        loaded = true
      } catch is CancellationError { clear() }
      catch {
        let keepsState = (error as? BackendRustClient.Failure)?.code == "clipboard_device_pending"
        if !keepsState { items = []; loaded = false }
        if !Task.isCancelled { message = error.localizedDescription }
      }
    }
  }
  func refresh() { run { nil } }
  func setEnabled(_ value: Bool) { run { _ = try await self.request(["operation": "set_enabled", "enabled": value]); return nil } }
  func upload() {
    let value = text
    run {
      _ = try await self.request(["operation": "add", "text": value])
      try await self.requireAccount()
      self.text = ""
      return nil
    }
  }
  /// 删除一条；`id` 为 nil 时清空云端全部条目，开关不变。
  func delete(id: String? = nil) {
    run {
      _ = try await self.request(id.map { ["operation": "delete", "id": $0] } ?? ["operation": "clear"])
      return nil
    }
  }
  /// 批准、拒绝、移除一台设备或重置云剪贴板，结果就是设备状态。
  func change(_ operation: BackendAccountClient.ClipboardE2EOperation) {
    let action: [String: Any]
    switch operation {
    case .approve(let id): action = ["operation": "approve", "device_id": id]
    case .reject(let id): action = ["operation": "reject", "device_id": id]
    case .remove(let id): action = ["operation": "remove", "device_id": id]
    case .reset: action = ["operation": "reset"]
    case .status: action = ["operation": "devices"]
    case .list, .add: return
    }
    run { try BackendRustAccount.decode(BackendAccountClient.ClipboardDevices.self, try await self.request(action)) }
  }
  func copy(_ item: BackendAccountClient.ClipboardItem) {
    run {
      NSPasteboard.general.clearContents()
      NSPasteboard.general.setString(item.text, forType: .string)
      return nil
    }
  }
  private func clear() { items = []; text = ""; loaded = false; e2e = nil; devices = nil }
  func close() { pending?.cancel(); clear(); search = "" }
}

struct MacCloudClipboardView: View {
  @StateObject private var model: MacClipboardModel
  @Environment(\.dismiss) private var dismiss
  @State private var confirmation: Confirmation?
  private enum Confirmation: Equatable {
    case enable, disable, clear, reset
    case remove(BackendAccountClient.ClipboardDevice)
    case reject(BackendAccountClient.ClipboardDevice)
  }
  init(accountID: String) { _model = StateObject(wrappedValue: MacClipboardModel(accountID: accountID)) }
  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack {
        Text("云剪贴板").font(.title2)
        Spacer()
        Button("关闭") { model.close(); dismiss() }
      }
      Text("文字在本机加密后才上传，服务端只保存密文。只上传你明确添加的内容，不自动读取系统剪贴板。最多保存 50 条；关闭云剪贴板会删除云端历史和所有设备的登记。")
        .font(.footnote).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
      if model.loaded {
        Button(model.enabled ? "关闭云剪贴板" : "开启云剪贴板") { confirmation = model.enabled ? .disable : .enable }
          .disabled(model.busy)
      }
      ScrollView {
        VStack(alignment: .leading, spacing: 12) {
          if model.enabled && model.loaded {
            if model.isPending { pendingCard } else { approvals; clipboard }
            deviceList
          }
          if !model.loaded && !model.busy { Button("加载云剪贴板") { model.refresh() } }
        }.frame(maxWidth: .infinity, alignment: .leading)
      }
      if model.busy { ProgressView() }
      if let message = model.message { Text(message).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true) }
    }
    .padding(20).frame(width: 560, height: 560)
    .onAppear { model.refresh() }
    .onDisappear { model.close() }
    .alert(title, isPresented: Binding(
      get: { confirmation != nil }, set: { if !$0 { confirmation = nil } })) {
      Button("取消", role: .cancel) { confirmation = nil }
      Button(confirmButton, role: confirmation == .enable ? nil : .destructive) {
        switch confirmation {
        case .enable: model.setEnabled(true)
        case .disable: model.setEnabled(false)
        case .clear: model.delete()
        case .reset: model.change(.reset)
        case .remove(let device): model.change(.remove(deviceID: device.device_id))
        case .reject(let device): model.change(.reject(deviceID: device.device_id))
        case nil: break
        }
        confirmation = nil
      }
    } message: { Text(confirmationMessage) }
  }

  /// 本机等待批准：显示核对码，说明去哪里批准；所有设备都用不了时可以重置。
  private var pendingCard: some View {
    VStack(alignment: .leading, spacing: 8) {
      Text("这台 Mac 正在等待批准").font(.headline)
      if let code = model.code {
        Text(code).font(.system(size: 22, weight: .semibold, design: .monospaced)).textSelection(.enabled)
          .accessibilityLabel("核对码 \(code)")
      }
      Text("在一台已经能使用云剪贴板的设备上打开云剪贴板，确认那里显示的核对码与上面相同，再点「允许」。批准后点「刷新」即可读取和上传。")
        .font(.footnote).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
      HStack {
        Button("刷新") { model.refresh() }.disabled(model.busy)
        Spacer()
        Button("重置云剪贴板…", role: .destructive) { confirmation = .reset }.disabled(model.busy)
      }
    }
    .padding(12).background(RoundedRectangle(cornerRadius: 8).fill(Color.secondary.opacity(0.08)))
  }

  /// 等待本机批准的其他设备。
  @ViewBuilder private var approvals: some View {
    let waiting = model.awaitingApproval
    if !waiting.isEmpty {
      VStack(alignment: .leading, spacing: 8) {
        Text("有设备请求使用云剪贴板").font(.headline)
        Text("只在那台设备上显示的核对码与这里相同时才点「允许」。").font(.footnote).foregroundStyle(.secondary)
        ForEach(waiting) { device in
          HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 2) {
              Text(Self.name(device))
              Text(device.code).font(.system(.body, design: .monospaced)).textSelection(.enabled)
            }
            Spacer()
            Button("拒绝") { confirmation = .reject(device) }
            Button("允许") { model.change(.approve(deviceID: device.device_id)) }.keyboardShortcut(.defaultAction)
          }.disabled(model.busy)
        }
      }
      .padding(12).background(RoundedRectangle(cornerRadius: 8).fill(Color.accentColor.opacity(0.08)))
    }
  }

  @ViewBuilder private var clipboard: some View {
    TextEditor(text: $model.text).frame(height: 80).border(Color.secondary.opacity(0.3))
    HStack {
      Text("\(model.text.utf16.count) / 4000").font(.caption)
      Spacer()
      Button("上传这段文字") { model.upload() }
        .disabled(model.busy || model.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.text.utf16.count > 4000)
    }
    HStack {
      TextField("搜索云端历史", text: $model.search).onSubmit { model.refresh() }
      Button("搜索 / 刷新") { model.refresh() }.disabled(model.busy)
    }
    if let skipped = model.e2e?.undecryptable, skipped > 0 {
      Text("有 \(skipped) 条内容用旧密钥加密，这台设备解不开，已略去。").font(.footnote).foregroundStyle(.secondary)
    }
    ForEach(model.items) { item in
      VStack(alignment: .leading, spacing: 6) {
        Text(item.text).textSelection(.enabled)
        let meta = Self.meta(item)
        if !meta.isEmpty { Text(meta).font(.caption).foregroundStyle(.secondary) }
        HStack {
          Button("复制到本机") { model.copy(item) }
          Button("删除", role: .destructive) { model.delete(id: item.id) }
        }.disabled(model.busy)
      }.padding(.vertical, 4)
      Divider()
    }
    Button("清空云端历史", role: .destructive) { confirmation = .clear }.disabled(model.busy || model.items.isEmpty)
  }

  /// 账号下的设备。移除一台已批准的设备会换钥，其余设备都要重新批准。
  @ViewBuilder private var deviceList: some View {
    if let devices = model.devices?.devices, !devices.isEmpty {
      VStack(alignment: .leading, spacing: 6) {
        Text("设备").font(.headline)
        ForEach(devices) { device in
          HStack {
            VStack(alignment: .leading, spacing: 2) {
              Text(Self.name(device) + (device.current ? "（本机）" : ""))
              Text(device.approved ? "已批准" : "等待批准").font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            if !device.current && !model.isPending {
              if device.approved {
                Button("移除", role: .destructive) { confirmation = .remove(device) }
              } else {
                Button("拒绝") { confirmation = .reject(device) }
              }
            }
          }.disabled(model.busy)
        }
        if !model.isPending {
          Button("重置云剪贴板…", role: .destructive) { confirmation = .reset }.disabled(model.busy)
        }
      }
    }
  }

  private static func name(_ device: BackendAccountClient.ClipboardDevice) -> String {
    device.name.isEmpty ? "未命名设备" : device.name
  }

  private static func meta(_ item: BackendAccountClient.ClipboardItem) -> String {
    var parts: [String] = []
    if item.pinned == true { parts.append("已置顶") }
    if let device = item.device { parts.append(device) }
    return parts.joined(separator: " · ")
  }

  private var title: String {
    switch confirmation {
    case .enable: return "开启云剪贴板？"
    case .disable, .clear: return "删除云端历史？"
    case .reset: return "重置云剪贴板？"
    case .remove(let device): return "移除「\(Self.name(device))」？"
    case .reject(let device): return "拒绝「\(Self.name(device))」？"
    case nil: return ""
    }
  }
  private var confirmButton: String {
    switch confirmation {
    case .reset: return "重置"
    case .remove: return "移除"
    case .reject: return "拒绝"
    default: return "确认"
    }
  }
  private var confirmationMessage: String {
    switch confirmation {
    case .enable: return "开启后可以手动上传文字，内容在本机加密后保存，在已登录并获准的设备上解密查看。"
    case .disable: return "关闭后云端历史、加密密钥和所有设备的登记都会删除，不可恢复；再开启时其他设备需要重新批准。已复制到其他应用的内容不会被删除。"
    case .clear: return "云端历史删除后不可恢复，已复制到其他应用的内容不会被删除。"
    case .reset: return "重置会换一把新的加密密钥并删除云端的全部条目。其他所有设备都要重新批准才能继续使用云剪贴板。只在已获批准的设备都用不了时这样做。"
    case .remove: return "移除后这台设备不能再读取云剪贴板。为此会换一把新的加密密钥并删除云端的全部条目，其他设备都要重新批准。"
    case .reject: return "拒绝后这台设备的登记会被删除；它下次打开云剪贴板时会重新请求批准。"
    case nil: return ""
    }
  }
}
