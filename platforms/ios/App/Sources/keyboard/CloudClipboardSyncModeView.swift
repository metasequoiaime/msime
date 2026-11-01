import SwiftUI

/// 「我的 → 云剪贴板」页顶部的「云剪贴板同步方式」：水杉账号、用户自建的中继（`docs/cloud-clipboard-relay.md`）或者用户自己的 WebDAV 网盘（`docs/cloud-clipboard-webdav.md`），与 Android 同一页的卡片和共享设置页的 `cloud-clipboard-relay-section.tsx` 做法和说法相同。选了中继或网盘就只走它，不需要登录；出错也不退回水杉账号。
///
/// 地址和凭据在填写页里填好、校验通过后点「保存」才写进共享偏好，不会把填到一半的地址交给自动保存。令牌和应用密码框是空的，不回显已存的值，已经保存过时留空表示沿用。网盘「测试连接」只测不存，「保存」先测、通过才写，不通过时填写页和草稿都留着。中继和网盘只能二选一：保存其中一个就删掉另一个，改回水杉账号两个都删；偏好的校验、凭据清单、不随设置导出和诊断脱敏都在 Rust 里。
struct CloudClipboardSyncModeSection: View {
  /// 偏好里现在的同步方式；nil 是水杉账号。
  let configuration: IOSCloudClipboardRelay.Configuration?
  var disabled = false
  /// 保存或改回水杉账号之后调用，页面据此重读同步方式和内容。
  let onChange: () -> Void
  @State private var picksMode = false
  @State private var editor: Editor?
  @State private var confirmsAccount = false

  enum Mode: Hashable { case account, relay, webdav }
  enum Editor: String, Identifiable {
    case relay, webdav
    var id: String { rawValue }
  }

  private var mode: Mode {
    switch configuration?.backend {
    case .relay: return .relay
    case .webdav: return .webdav
    case nil: return .account
    }
  }

  private static let options: [DesignOption<Mode>] = [
    DesignOption(title: "水杉账号", value: .account, identifier: "cloudClipboardModeAccount"),
    DesignOption(title: "自建中继", value: .relay, identifier: "cloudClipboardModeRelay"),
    DesignOption(title: "WebDAV", value: .webdav, identifier: "cloudClipboardModeWebDAV"),
  ]

  var body: some View {
    VStack(alignment: .leading, spacing: 7) {
      // WebDAV 的存储、配额和回收站说明是解释，收进「?」；「现在经哪里同步」是状态，留在卡片下面。
      DesignSectionHeader(subject: "云剪贴板同步方式", info: configuration?.backend == .webdav ? IOSCloudClipboardRelayPolicy.webdavNote : nil) {
        SettingsGroupHeader(title: "云剪贴板同步方式")
      }
      .padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        Button { picksMode = true } label: {
          DesignNavRowLabel(title: "同步方式",
                            info: "自建中继和 WebDAV 都不需要登录水杉账号：内容仍在本机加密，中继或网盘只保存密文，新设备照样要在已有设备上核对后点「允许」。",
                            value: DesignOption.title(of: mode, in: Self.options))
        }
        .buttonStyle(PressFillButtonStyle())
        .accessibilityIdentifier("cloudClipboardSyncMode")
        if let configuration {
          switch configuration.backend {
          case .relay:
            DesignDivider()
            row("中继地址", Self.shorten(configuration.url), editor: .relay, identifier: "cloudClipboardRelayURL")
            DesignDivider()
            row("中继令牌", configuration.hasSecret ? "已填写" : "未填写", editor: .relay, identifier: "cloudClipboardRelayToken")
          case .webdav:
            DesignDivider()
            row("服务器地址", Self.shorten(configuration.url), editor: .webdav, identifier: "cloudClipboardWebDAVURL")
            DesignDivider()
            row("账号", Self.shorten(configuration.username), editor: .webdav, identifier: "cloudClipboardWebDAVUsername")
            DesignDivider()
            row("应用密码", configuration.hasSecret ? "已填写" : "未填写", editor: .webdav, identifier: "cloudClipboardWebDAVPassword")
          }
        }
      }
      .disabled(disabled)
      if let footer {
        Text(footer).designSubtext()
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, 16)
          .accessibilityIdentifier("cloudClipboardSyncModeFooter")
      }
    }
    .designOptionSheet(isPresented: $picksMode, title: "云剪贴板同步方式", options: Self.options, selected: mode) { picked in
      switch picked {
      case .account: if configuration != nil { confirmsAccount = true }
      case .relay: if mode != .relay { editor = .relay }
      case .webdav: if mode != .webdav { editor = .webdav }
      }
    }
    .sheet(item: $editor) { editor in
      switch editor {
      case .relay: CloudClipboardRelayEditor(saved: configuration, onSaved: onChange)
      case .webdav: CloudClipboardWebDAVEditor(saved: configuration, onSaved: onChange)
      }
    }
    .confirmationDialog("改回水杉账号？", isPresented: $confirmsAccount, titleVisibility: .visible) {
      Button("改回水杉账号", role: .destructive) { switchToAccount() }
      Button("取消", role: .cancel) { }
    } message: {
      Text((configuration?.backend == .webdav
            ? "会清除本机保存的网盘地址、账号和应用密码，网盘上的内容不受影响。"
            : "会清除本机保存的中继地址和令牌，中继上的内容不受影响。") + "之后云剪贴板经水杉账号同步，要登录才能用。")
    }
  }

  private func row(_ title: String, _ value: String, editor: Editor, identifier: String) -> some View {
    Button { self.editor = editor } label: { DesignNavRowLabel(title: title, value: value) }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier(identifier)
  }

  /// 卡片下面的说明：现在经哪里同步、换了什么要重新允许；网盘再加上那一段关于存储、同步时机、配额和凭据的说明。
  private var footer: String? {
    guard let configuration else { return nil }
    switch configuration.backend {
    case .relay:
      return "云剪贴板现在经 \(configuration.url) 同步，中继出错时也不会改用水杉账号。换了地址或令牌后，本机要在中继上重新登记，并在其他设备上重新允许。"
    case .webdav:
      return "云剪贴板现在经 WebDAV 网盘 \(configuration.url) 同步，网盘出错时也不会改用水杉账号。换了地址或账号后，本机要重新登记，并在其他设备上重新允许；换应用密码不用。"
    }
  }

  /// 改回水杉账号：删掉偏好里的中继和网盘，中继或网盘上的内容和本机的中继密钥串都不动。
  private func switchToAccount() {
    let saved = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
      preferences.removeValue(forKey: IOSCloudClipboardRelayPolicy.relayPreference)
      preferences.removeValue(forKey: IOSCloudClipboardRelayPolicy.webdavPreference)
    }
    ToastCenter.shared.show(saved ? "已改回水杉账号" : "保存失败，请重试")
    onChange()
  }

  static func shorten(_ value: String) -> String {
    value.count <= 32 ? value : String(value.prefix(31)) + "…"
  }
}

/// 填写自建中继的地址和令牌。两项按共享规则当场检查（`IOSCloudClipboardRelayPolicy`），不合规时在输入框下说明原因并禁用「保存」。已经配置过中继时令牌框可以留空，表示保留已存的令牌。保存时删掉 WebDAV 网盘。
struct CloudClipboardRelayEditor: View {
  let onSaved: () -> Void
  private let savedURL: String
  private let savedToken: String
  /// 现在走的是网盘：保存中继就是换掉它，地址没改也算改动。
  private let replacesWebDAV: Bool
  @Environment(\.dismiss) private var dismiss
  @State private var url: String
  @State private var token = ""
  @State private var failure: String?

  init(saved: IOSCloudClipboardRelay.Configuration?, onSaved: @escaping () -> Void) {
    self.onSaved = onSaved
    let relay = saved?.backend == .relay ? saved : nil
    savedURL = relay?.url ?? ""
    savedToken = relay?.savedToken ?? ""
    replacesWebDAV = saved?.backend == .webdav
    _url = State(initialValue: savedURL)
  }

  private var trimmedURL: String { url.trimmingCharacters(in: .whitespacesAndNewlines) }
  private var trimmedToken: String { token.trimmingCharacters(in: .whitespacesAndNewlines) }
  private var urlIssue: String? { IOSCloudClipboardRelayPolicy.relayURLIssue(trimmedURL) }
  private var tokenIssue: String? {
    trimmedToken.isEmpty && !savedToken.isEmpty ? nil : IOSCloudClipboardRelayPolicy.tokenIssue(trimmedToken)
  }
  private var changed: Bool {
    trimmedURL != savedURL || (!trimmedToken.isEmpty && trimmedToken != savedToken) || replacesWebDAV
  }
  private var canSave: Bool { changed && urlIssue == nil && tokenIssue == nil }

  var body: some View {
    NavigationStack {
      Form {
        Section {
          TextField("https://relay.example.com/msime", text: $url)
            .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
            .accessibilityIdentifier("cloudClipboardRelayURLField")
        } header: {
          DesignSectionHeader(subject: "中继地址", info: "例如 https://relay.example.com/msime。必须是 HTTPS，证书要由公网信任的机构签发（自签证书和私有 CA 连不上）；HTTP 只用于本机测试（127.0.0.1）。") { SettingsGroupHeader(title: "中继地址") }
        } footer: {
          CloudClipboardFieldFooter(issue: url.isEmpty ? nil : urlIssue)
        }
        .designRow()
        Section {
          SecureField(savedToken.isEmpty ? "中继令牌" : "留空以保留已保存的令牌", text: $token)
            .textInputAutocapitalization(.never).autocorrectionDisabled()
            .accessibilityIdentifier("cloudClipboardRelayTokenField")
        } header: {
          DesignSectionHeader(subject: "中继令牌", info: "由中继管理员生成，每个令牌对应中继上一份独立的云剪贴板。令牌只存在本机的设置里，不会随设置文件导出或账号同步。") { SettingsGroupHeader(title: "中继令牌") }
        } footer: {
          CloudClipboardFieldFooter(issue: token.isEmpty ? nil : tokenIssue)
        }
        .designRow()
        if let failure {
          Section { Text(failure).foregroundStyle(MetasequoiaTheme.danger) }.designRow()
        }
      }
      .designPage()
      .fittedSheet()
      .navigationTitle("自建中继")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("取消") { close() } }
        ToolbarItem(placement: .confirmationAction) {
          Button("保存") { save() }
            .fontWeight(.semibold)
            .disabled(!canSave)
            .accessibilityIdentifier("cloudClipboardRelaySave")
        }
      }
    }
    .tint(MetasequoiaTheme.accent)
  }

  private func save() {
    guard canSave else { return }
    let relay: [String: Any] = ["url": trimmedURL, "token": trimmedToken.isEmpty ? savedToken : trimmedToken]
    let saved = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
      preferences.removeValue(forKey: IOSCloudClipboardRelayPolicy.webdavPreference)
      preferences[IOSCloudClipboardRelayPolicy.relayPreference] = relay
    }
    guard saved else { failure = "保存失败，请重试"; return }
    ToastCenter.shared.show("已保存，云剪贴板改走自建中继")
    close()
    onSaved()
  }

  /// 关掉填写页之前清空令牌框：框里还有内容时系统会弹出「保存密码？」，令牌不该进系统的密码库。
  private func close() {
    token = ""
    dismiss()
  }
}

/// 填写 WebDAV 网盘的服务器地址、账号和应用密码。三项按共享规则当场检查，不合规时在输入框下说明原因并禁用「测试连接」和「保存」。已经配置过网盘时应用密码框可以留空，表示保留已存的应用密码。「测试连接」只测不存；「保存」先测，通过才写进偏好（并删掉中继），没通过时填写页留着，原因写在下面。
struct CloudClipboardWebDAVEditor: View {
  let onSaved: () -> Void
  private let savedURL: String
  private let savedUsername: String
  private let savedPassword: String
  /// 现在走的是中继：保存网盘就是换掉它。
  private let replacesRelay: Bool
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

  init(saved: IOSCloudClipboardRelay.Configuration?, onSaved: @escaping () -> Void) {
    self.onSaved = onSaved
    let webdav = saved?.backend == .webdav ? saved : nil
    savedURL = webdav?.url ?? ""
    savedUsername = webdav?.username ?? ""
    savedPassword = webdav?.savedPassword ?? ""
    replacesRelay = saved?.backend == .relay
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
    trimmedURL != savedURL || trimmedUsername != savedUsername || effectivePassword != savedPassword || replacesRelay
  }
  private var canSave: Bool { canProbe && changed }

  var body: some View {
    NavigationStack {
      Form {
        Section {
          TextField("https://dav.jianguoyun.com/dav/", text: edited($url))
            .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
            .accessibilityIdentifier("cloudClipboardWebDAVURLField")
        } header: {
          DesignSectionHeader(subject: "服务器地址", info: "坚果云填 https://dav.jianguoyun.com/dav/ 即可，水杉会在里面自动建 msime-clipboard 文件夹，不用自己先建；其他网盘填它给的 WebDAV 地址。必须是 HTTPS，证书要由公网信任的机构签发。") { SettingsGroupHeader(title: "服务器地址") }
        } footer: {
          CloudClipboardFieldFooter(issue: url.isEmpty ? nil : urlIssue)
        }
        .designRow()
        Section {
          TextField("账号", text: edited($username))
            .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.emailAddress)
            .accessibilityIdentifier("cloudClipboardWebDAVUsernameField")
        } header: {
          DesignSectionHeader(subject: "账号", info: "网盘的登录账号，坚果云是登录邮箱。") { SettingsGroupHeader(title: "账号") }
        } footer: {
          CloudClipboardFieldFooter(issue: username.isEmpty ? nil : usernameIssue)
        }
        .designRow()
        Section {
          SecureField(savedPassword.isEmpty ? "应用密码" : "留空以保留已保存的应用密码", text: edited($password))
            .textInputAutocapitalization(.never).autocorrectionDisabled()
            .accessibilityIdentifier("cloudClipboardWebDAVPasswordField")
        } header: {
          DesignSectionHeader(subject: "应用密码", info: "在网盘的安全设置里为第三方应用生成的密码，不是登录密码。只存在本机的设置里，不会随设置文件导出或账号同步。") { SettingsGroupHeader(title: "应用密码") }
        } footer: {
          CloudClipboardFieldFooter(issue: password.isEmpty ? nil : passwordIssue)
        }
        .designRow()
        Section {
          Button(probe == .probing ? "正在测试连接……" : "测试连接") { start(saving: false) }
            .disabled(!canProbe)
            .accessibilityIdentifier("cloudClipboardWebDAVProbe")
          if let status {
            Text(status.text).font(.footnote)
              .foregroundStyle(status.failed ? MetasequoiaTheme.danger : MetasequoiaTheme.sub)
              .fixedSize(horizontal: false, vertical: true)
              .accessibilityIdentifier("cloudClipboardWebDAVStatus")
          }
        } header: {
          DesignSectionHeader(info: IOSCloudClipboardRelayPolicy.webdavNote)
        }
        .designRow()
      }
      .designPage()
      .fittedSheet()
      .navigationTitle("WebDAV 网盘")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("取消") { close() } }
        ToolbarItem(placement: .confirmationAction) {
          Button("保存") { start(saving: true) }
            .fontWeight(.semibold)
            .disabled(!canSave)
            .accessibilityIdentifier("cloudClipboardWebDAVSave")
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
        try await IOSCloudClipboardRelay.probeWebDAV(url: address, username: account, password: secret)
      } catch let error as IOSCloudClipboardRelayFailure {
        failure = IOSCloudClipboardRelayPolicy.probeMessage(error.code)
      } catch {
        failure = IOSCloudClipboardRelayPolicy.probeMessage("")
      }
      guard !Task.isCancelled else { return }
      guard generation == draft else { probe = .idle; return }
      if let failure { probe = .failed(failure); return }
      guard saving else { probe = .passed; return }
      probe = .saving
      let webdav: [String: Any] = ["url": address, "username": account, "password": secret]
      let saved = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
        preferences.removeValue(forKey: IOSCloudClipboardRelayPolicy.relayPreference)
        preferences[IOSCloudClipboardRelayPolicy.webdavPreference] = webdav
      }
      guard saved else { probe = .failed("保存失败，请重试"); return }
      ToastCenter.shared.show("已保存，云剪贴板改走 WebDAV 网盘")
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
private struct CloudClipboardFieldFooter: View {
  let issue: String?

  var body: some View {
    if let issue {
      Text(issue).foregroundStyle(MetasequoiaTheme.danger).accessibilityIdentifier("cloudClipboardFieldIssue")
    }
  }
}
