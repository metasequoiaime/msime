import AppKit
import SwiftUI

@MainActor
final class MacAccountModel: NSObject, ObservableObject {
  @Published var user: BackendAccountClient.User?
  @Published var anonymous = false
  @Published var providers: [String: Bool] = [:]
  @Published var message: String?
  @Published var busy = false
  @Published var name = ""
  @Published var target = ""
  @Published var code = ""
  @Published var channel = "email"
  @Published var challenge: BackendAccountClient.Challenge?
  @Published var expiresAt = Date.distantPast
  @Published var resendAt = Date.distantPast
  private let client: BackendAccountClient
  private let account: BackendAccountSession
  private let anonymousAccount: BackendAccountSession
  private let closeAccountWindows: @MainActor () -> Void
  private let discardAnonymous: () -> Void
  private var pending: Task<Void, Never>?

  init(client: BackendAccountClient = BackendAccountClient(), account: BackendAccountSession = .shared,
       anonymousAccount: BackendAccountSession = BackendAccountSession(storage: BackendAnonymousAccount.sessionStorage()),
       closeAccountWindows: @escaping @MainActor () -> Void = { BackendWindowBridge.shared.closeAll() },
       discardAnonymous: @escaping () -> Void = BackendAnonymousAccount.discard) {
    self.client = client; self.account = account; self.anonymousAccount = anonymousAccount
    self.closeAccountWindows = closeAccountWindows
    self.discardAnonymous = discardAnonymous
    super.init()
  }

  func perform(_ action: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do { try await action(); try Task.checkCancellation() }
      catch is CancellationError { }
      catch { if !Task.isCancelled { message = error.localizedDescription } }
    }
  }
  func load() {
    perform {
      if let user = try await self.account.user() {
        self.user = user
        self.anonymous = false
      } else {
        self.user = try await self.anonymousAccount.user()
        self.anonymous = self.user != nil
      }
      self.name = self.user?.preferredDisplayName ?? ""
      let providers = try await self.client.providers()
      try Task.checkCancellation()
      self.providers = providers
    }
  }
  private var currentSession: BackendAccountSession { anonymous ? anonymousAccount : account }
  func requestCode() {
    guard resendAt <= Date() else { return }
    perform {
      guard self.providers[self.channel] == true else { throw BackendAccountClient.Failure(status: 503) }
      let response = try await self.client.challenge(provider: self.channel, target: self.target.trimmingCharacters(in: .whitespacesAndNewlines))
      try Task.checkCancellation()
      self.challenge = response
      self.expiresAt = Date().addingTimeInterval(TimeInterval(response.expires_in))
      self.resendAt = Date().addingTimeInterval(60)
      self.code = ""
    }
  }
  func codeLogin() {
    guard let challenge, expiresAt > Date(), code.utf8.count == 6, code.utf8.allSatisfy({ (48...57).contains($0) }) else { return }
    perform {
      let replacingAnonymous = self.anonymous
      try await self.account.signIn(challenge: challenge.challenge_id, credential: self.code)
      if replacingAnonymous {
        try await self.anonymousAccount.forget()
        self.discardAnonymous()
      }
      let user = try await self.account.user()
      try Task.checkCancellation()
      self.user = user; self.name = self.user?.preferredDisplayName ?? ""; self.anonymous = false
      self.challenge = nil; self.code = ""; self.target = ""
    }
  }
  func rename() {
    let value = name.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !value.isEmpty, value.unicodeScalars.count <= 64,
          !value.unicodeScalars.contains(where: { $0.properties.generalCategory == .control }) else {
      message = "昵称需为 1–64 个字符，不能包含换行或控制字符。"; return
    }
    perform {
      let identity = try await self.currentSession.credentials()
      try await self.client.rename(value, token: identity.token)
      let profile = try await self.client.profile(token: identity.token)
      try await self.currentSession.updateUser(profile.user, matching: identity.token)
      try Task.checkCancellation()
      self.user = profile.user; self.name = profile.user.preferredDisplayName
    }
  }
  func logout(all: Bool = false, delete: Bool = false) {
    perform {
      let session = self.currentSession
      if delete {
        let identity = try await session.credentials()
        try await self.client.deleteAccount(token: identity.token)
        self.closeAccountWindows()
        try await session.forget()
        if self.anonymous { self.discardAnonymous() }
      } else {
        // Hide private views before awaiting logout, including offline failures.
        self.closeAccountWindows()
        do { try await session.logout(all: all) }
        catch { self.user = try await session.user(); throw error }
      }
      self.user = nil; self.name = ""; self.anonymous = false
    }
  }
  func close() { pending?.cancel(); code = ""; target = ""; challenge = nil }
}

private struct SettingsCard<Content: View>: View {
  var title: String?
  @ViewBuilder var content: Content
  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      if let title {
        Text(title).font(.system(size: 11, weight: .semibold)).foregroundStyle(.secondary).padding(.leading, 2)
      }
      VStack(spacing: 0) { content }
        .padding(.horizontal, 16)
        .background(RoundedRectangle(cornerRadius: 12).fill(Color(nsColor: .controlBackgroundColor)))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color(nsColor: .separatorColor)))
    }
  }
}

private struct SettingsRow<Trailing: View>: View {
  let title: String
  var subtitle: String?
  var destructive = false
  @ViewBuilder var trailing: Trailing
  var body: some View {
    HStack(spacing: 12) {
      VStack(alignment: .leading, spacing: 1) {
        Text(title).font(.system(size: 15)).foregroundStyle(destructive ? Color.red : Color.primary)
        if let subtitle { Text(subtitle).font(.caption).foregroundStyle(.secondary) }
      }
      Spacer(minLength: 12)
      trailing
    }
    .frame(minHeight: subtitle == nil ? 48 : 56)
  }
}

private struct DisclosureRow: View {
  let title: String
  let subtitle: String
  let action: () -> Void
  @State private var hovering = false
  var body: some View {
    Button(action: action) {
      HStack(spacing: 12) {
        VStack(alignment: .leading, spacing: 1) {
          Text(title).font(.system(size: 15)).foregroundStyle(.primary)
          Text(subtitle).font(.caption).foregroundStyle(.secondary)
        }
        Spacer(minLength: 12)
        Image(systemName: "chevron.right").font(.system(size: 12, weight: .semibold)).foregroundStyle(.tertiary)
      }
      .frame(minHeight: 56).contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .background(hovering ? Color(nsColor: .selectedContentBackgroundColor).opacity(0.12) : .clear)
    .onHover { hovering = $0 }
  }
}

private struct CardDivider: View {
  var body: some View { Divider().overlay(Color(nsColor: .separatorColor)) }
}

private struct MacAccountView: View {
  @ObservedObject var model: MacAccountModel
  @State private var deleting = false
  @State private var clipboard = false
  @State private var settings = false
  @State private var dictionary = false
  @State private var snapshot = false
  @State private var resources = false
  private var monogram: String { String((model.user?.preferredDisplayName ?? "").prefix(1)).uppercased() }
  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 20) {
        if let user = model.user { identity(user) }
        if model.anonymous { localAccountNotice }
        if model.user == nil {
          Text("当前没有可用账号。输入法会在启动时尝试连接本机账号，请检查网络后重启输入法。")
            .font(.callout).foregroundStyle(.secondary)
          signInCard
        }
        if model.user != nil { cloudServices; accountActions }
        if model.busy { ProgressView().controlSize(.small) }
        if let message = model.message { Text(message).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true) }
      }
      .padding(.vertical, 4)
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .padding(.horizontal, 24).frame(width: 560, height: 680).disabled(model.busy)
    .sheet(isPresented: $resources) {
      if let user = model.user { BackendCommunityResourcesView(accountID: user.id).frame(width: 650, height: 650) }
    }
    .sheet(isPresented: $snapshot) {
      if let user = model.user { MacCloudSnapshotView(accountID: user.id) }
    }
    .sheet(isPresented: $dictionary) {
      if let user = model.user { MacCloudDictionaryView(accountID: user.id) }
    }
    .sheet(isPresented: $settings) {
      if let user = model.user { MacCloudSettingsView(accountID: user.id) }
    }
    .sheet(isPresented: $clipboard) {
      if let user = model.user { MacCloudClipboardView(accountID: user.id) }
    }
    .alert("注销账号？", isPresented: $deleting) {
      Button("取消", role: .cancel) { }
      Button("确认注销", role: .destructive) { model.logout(delete: true) }
    } message: { Text("将删除账号及其云端数据，此操作不可撤销。") }
  }

  private func identity(_ user: BackendAccountClient.User) -> some View {
    SettingsCard {
      HStack(spacing: 14) {
        Text(monogram).font(.system(size: 22, weight: .medium)).foregroundStyle(.white)
          .frame(width: 52, height: 52).background(Circle().fill(Color.accentColor))
        VStack(alignment: .leading, spacing: 3) {
          Text(user.preferredDisplayName).font(.system(size: 17, weight: .semibold))
          if model.anonymous, let anonymous = BackendAnonymousAccount.stored() {
            Text(anonymous.subject).font(.system(size: 12, design: .monospaced)).foregroundStyle(.secondary).textSelection(.enabled)
          }
        }
        Spacer(minLength: 12)
        if model.anonymous {
          Text("本机账号").font(.system(size: 11, weight: .medium)).padding(.horizontal, 8).padding(.vertical, 3)
            .background(Capsule().fill(Color.orange.opacity(0.16))).foregroundStyle(.orange)
        }
      }.frame(minHeight: 72)
      CardDivider()
      SettingsRow(title: "昵称") {
        HStack(spacing: 8) {
          TextField("", text: $model.name).textFieldStyle(.roundedBorder).frame(width: 200)
          Button("保存") { model.rename() }
            .disabled(model.name.trimmingCharacters(in: .whitespacesAndNewlines) == user.preferredDisplayName)
        }
      }
    }
  }

  private var localAccountNotice: some View {
    SettingsCard(title: "本机账号") {
      HStack(alignment: .top, spacing: 10) {
        Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange).font(.system(size: 13))
        Text("这个账号是安装时自动创建的，凭据只在本机。清除输入法数据或更换设备后无法找回它和它的云端词库。")
          .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
      }.padding(.vertical, 12)
    }
  }

  private var signInCard: some View {
    SettingsCard(title: "登录") {
      Picker("验证码登录", selection: $model.channel) {
        Text("邮箱").tag("email"); Text("手机号").tag("phone")
      }.onChange(of: model.channel) { _ in model.challenge = nil; model.code = "" }
      TextField(model.channel == "email" ? "邮箱地址" : "手机号（含国家区号，如 +86）", text: $model.target)
        .onChange(of: model.target) { _ in model.challenge = nil; model.code = "" }
      TimelineView(.periodic(from: .now, by: 1)) { timeline in
        let remaining = max(0, Int(ceil(model.resendAt.timeIntervalSince(timeline.date))))
        Button(remaining == 0 ? "获取验证码" : "\(remaining) 秒后可重发") { model.requestCode() }
          .disabled(remaining > 0 || model.providers[model.channel] != true || model.target.isEmpty)
        if model.challenge != nil {
          TextField("6 位验证码", text: $model.code)
          Button("登录") { model.codeLogin() }
            .disabled(model.expiresAt <= timeline.date || model.code.utf8.count != 6 || !model.code.utf8.allSatisfy { (48...57).contains($0) })
        }
      }
      if model.providers[model.channel] != true { Text("此登录方式尚未启用。").foregroundStyle(.secondary) }
    }
  }

  private var cloudServices: some View {
    SettingsCard(title: "云服务") {
      DisclosureRow(title: "云剪贴板", subtitle: "在设备之间同步复制的内容") { clipboard = true }
      CardDivider()
      DisclosureRow(title: "桌面设置同步", subtitle: "把这台机器的偏好上传或取回") { settings = true }
      CardDivider()
      DisclosureRow(title: "词包与回复模板", subtitle: "社区分享的词库与常用语") { resources = true }
      CardDivider()
      DisclosureRow(title: "云词库", subtitle: "自造词与调频记录") { dictionary = true }
      CardDivider()
      DisclosureRow(title: "云词库同步与备份", subtitle: "手动备份与按版本回滚") { snapshot = true }
    }
  }

  private var accountActions: some View {
    SettingsCard(title: "账号") {
      SettingsRow(title: "退出登录", subtitle: "只退出这台设备") { Button("退出") { model.logout() } }
      CardDivider()
      SettingsRow(title: "退出所有设备", subtitle: "使其他设备上的登录立即失效") { Button("全部退出") { model.logout(all: true) } }
      CardDivider()
      SettingsRow(title: "注销账号", subtitle: "删除账号及其云端数据，不可撤销", destructive: true) {
        Button("注销") { deleting = true }
      }
    }
  }
}

@MainActor @objc(MSIMEBackendAccountWindow)
final class BackendAccountWindow: NSWindowController, NSWindowDelegate {
  @objc static let shared = BackendAccountWindow()
  private let model = MacAccountModel()
  private init() {
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 560, height: 680), styleMask: [.titled, .closable], backing: .buffered, defer: false)
    super.init(window: window)
    window.title = "水杉账号"; window.isReleasedWhenClosed = false; window.delegate = self
    window.contentView = NSHostingView(rootView: MacAccountView(model: model))
    window.center()
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
  @objc func showAccount() { showWindow(nil); presentBackendWindow(window); model.load() }
  @objc func showCloudClipboard() {
    Task { @MainActor in
      await BackendClipboardEntry.open(account: .shared,
        present: { BackendWindowBridge.shared.showClipboard(forAccountID: $0) },
        signIn: { self.showAccount() })
    }
  }
  func windowWillClose(_ notification: Notification) { model.close() }
}

@_cdecl("MSIMEShowBackendAccount")
public func showBackendAccount() {
  Task { @MainActor in BackendAccountWindow.shared.showAccount() }
}

// The native preferences window can host the same SwiftUI surface inline. This
// keeps the login and account actions in one place while giving Apple users a
// platform-appropriate page instead of stacking a second window over settings.
@MainActor
private final class AccountPane {
  static let shared = AccountPane()
  let model = MacAccountModel()
  lazy var hosting: NSHostingView<MacAccountView> = {
    let view = NSHostingView(rootView: MacAccountView(model: model))
    view.translatesAutoresizingMaskIntoConstraints = false
    return view
  }()
}

@_cdecl("MSIMEAccountPaneView")
@MainActor
public func accountPaneView() -> NSView {
  MainActor.assumeIsolated { AccountPane.shared.hosting }
}

@_cdecl("MSIMEAccountPaneAttach")
@MainActor
public func accountPaneAttach(_ window: NSWindow?) {
  MainActor.assumeIsolated {
    _ = window
    AccountPane.shared.model.load()
  }
}

@_cdecl("MSIMEAccountPaneClose")
@MainActor
public func accountPaneClose() {
  MainActor.assumeIsolated { AccountPane.shared.model.close() }
}
