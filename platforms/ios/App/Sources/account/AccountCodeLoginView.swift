import AuthenticationServices
import SwiftUI
#if canImport(GoogleSignIn)
import GoogleSignIn
#endif

/// Google Sign-In adds its own cancellation error to the shared account request policy.
private func isLoginCancellation(_ error: Error) -> Bool {
  if AccountRequestPolicy.isCancellation(error) { return true }
  #if canImport(GoogleSignIn)
  if let error = error as? GIDSignInError { return error.code == .canceled }
  #endif
  return false
}

/// 这个构建能不能发起 Google 登录：链接了 GoogleSignIn（只有 CocoaPods workspace 构建才有，见 `Podfile`），并且 Info.plist 里 iOS 与服务端两个 client ID 都不为空。服务端 client ID 决定 ID token 的 audience，后端认的是它，与 Android 用的是同一个。
enum GoogleSignInSupport {
  static var available: Bool {
    #if canImport(GoogleSignIn)
    let configured = { (key: String) in (Bundle.main.object(forInfoDictionaryKey: key) as? String)?.isEmpty == false }
    return configured("GIDClientID") && configured("GIDServerClientID")
    #else
    return false
    #endif
  }
}

/// 登录弹窗共享的登录状态：后端提供哪些登录方式、待用的 Apple challenge、邮箱和手机号的验证码流程，以及刚完成的那次登录用的是哪种方式。挑战和登录都经 Rust（#8201 迁移计划 4b），本机会话由 Rust 存进钥匙串。
@MainActor
final class CodeLoginModel: ObservableObject {
  @Published var user: BackendAccountClient.User?
  @Published var providers: [String: Bool] = [:]
  /// 请求过登录方式列表后即为 true，不论回应是否已经到达。
  @Published private(set) var providersLoaded = false
  /// 下一次「通过 Apple 登录」请求携带的 Apple challenge；正在获取或不提供 Apple 登录时为 nil。
  @Published private(set) var appleChallenge: CommunityChallenge?
  @Published private(set) var preparingApple = false
  /// 上一次登录使用的方式（"Apple"、"Google"、"邮箱"、"手机号"），只在登录成功时设置。
  @Published private(set) var signedInVia: String?
  @Published var busy = false
  @Published var message: String?
  private let api: SkinCommunityAPI
  private let rust: @Sendable () throws -> BackendRustAccount
  /// 登录换了账号时取消上一个账号排队中的词库快照，见 `SkinCommunityAPI.signIn`。
  private let cancelSnapshot: @Sendable (String) throws -> Void
  private var generation = OperationGeneration()

  init(api: SkinCommunityAPI = .shared,
       rust: @escaping @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() },
       cancelSnapshot: @escaping @Sendable (String) throws -> Void = { try DictionarySnapshotQueue().cancelIfPresent(accountID: $0) }) {
    self.api = api; self.rust = rust; self.cancelSnapshot = cancelSnapshot
  }

  /// 让登录面板关闭时已经在路上的请求失效。网络客户端未必会及时响应 Task 的取消，
  /// 所以结果写回前还要用代次再挡一次。
  func invalidate() {
    generation.invalidate()
    busy = false
    appleChallenge = nil
    preparingApple = false
    message = nil
  }

  func loadProviders() async {
    let id = generation.current()
    await perform { id in
      let providers = try await self.rust().providers()
      try self.ensureCurrent(id)
      self.providers = providers
    }
    guard generation.isCurrent(id) else { return }
    providersLoaded = true
  }

  /// 加载登录方式；其中有 Apple 时，同时获取第一个 Apple challenge。
  func prepare() async {
    let id = generation.current()
    await loadProviders()
    guard generation.isCurrent(id) else { return }
    await prepareApple(expected: id)
  }

  func prepareApple() async {
    await prepareApple(expected: generation.current())
  }

  private func prepareApple(expected id: UUID) async {
    guard generation.isCurrent(id), !Task.isCancelled else { return }
    appleChallenge = nil
    guard providers["apple"] == true else { return }
    preparingApple = true
    defer { if generation.isCurrent(id) { preparingApple = false } }
    do {
      let challenge = try await api.challenge()
      try ensureCurrent(id)
      appleChallenge = challenge
    } catch is CancellationError {
    } catch {
      guard generation.isCurrent(id) else { return }
      report(error)
    }
  }

  func requestCode(provider: String, target: String) async -> BackendRustAccount.Challenge? {
    var response: BackendRustAccount.Challenge?
    await perform { id in
      guard self.providers[provider] == true else { throw BackendRustClient.Failure(code: "account_unavailable") }
      let result = try BackendRustAccount.challenge(try await self.rust().account([
        "operation": "request_code", "provider": provider, "target": target]))
      try self.ensureCurrent(id)
      response = result
    }
    return response
  }

  func signInWithCode(challenge: String, code: String, channel: CodeLoginChannel) async {
    await perform { id in
      let user = try await SkinCommunityAPI.signIn(try self.rust(), cancelSnapshot: self.cancelSnapshot) { account in
        try await account.account(["operation": "login", "challenge_id": challenge, "code": code])
      }
      try self.ensureCurrent(id)
      self.user = user
      self.signedInVia = channel.via
    }
  }

  /// 完成「通过 Apple 登录」。凭证的 `state` 必须回传 challenge id，借此把 identity token 和本 App 向后端申请的 nonce 绑在一起。成功以外的任何结果都会重新获取 challenge，因为后端每个 challenge 只接受一次。
  func signInWithApple(_ result: Result<ASAuthorization, Error>) async {
    let id = generation.current()
    guard let challenge = appleChallenge else { return }
    switch result {
    case .success(let authorization):
      guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
            credential.state == challenge.challenge_id,
            let data = credential.identityToken, let token = String(data: data, encoding: .utf8) else {
        guard generation.isCurrent(id) else { return }
        message = "Apple 登录未返回有效凭据，请重试。"
        await prepareApple(expected: id)
        return
      }
      var failed = false
      await perform { id in
        do { try await self.api.login(challenge: challenge.challenge_id, identityToken: token) }
        catch { failed = true; throw error }
        try self.ensureCurrent(id)
        let user = try await self.api.currentUser()
        try self.ensureCurrent(id)
        self.user = user
        if self.user != nil { self.signedInVia = "Apple" }
      }
      if failed, generation.isCurrent(id) { await prepareApple(expected: id) }
    case .failure(let error):
      guard generation.isCurrent(id) else { return }
      if (error as? ASAuthorizationError)?.code != .canceled { message = error.localizedDescription }
      await prepareApple(expected: id)
    }
  }

  /// 完成「通过 Google 登录」：向后端要 challenge 和 nonce，拉起 Google 的账号选择，让 ID token 带上这个 nonce，再用它换会话。GoogleSignIn 自己在钥匙串里留的 Google 会话拿到 token 后立即清掉，登录状态只由后端会话决定，下次登录也会重新让用户选账号。
  func signInWithGoogle() async {
    #if canImport(GoogleSignIn)
    await perform { id in
      let challenge = try BackendRustAccount.challenge(try await self.rust().account(["operation": "google_challenge"]))
      try self.ensureCurrent(id)
      guard let nonce = challenge.nonce, let presenter = Self.presenter() else {
        self.message = "现在无法开始 Google 登录，请稍后再试。"
        return
      }
      let result = try await GIDSignIn.sharedInstance.signIn(withPresenting: presenter, hint: nil,
                                                              additionalScopes: nil, nonce: nonce)
      GIDSignIn.sharedInstance.signOut()
      try self.ensureCurrent(id)
      guard let token = result.user.idToken?.tokenString else {
        self.message = "Google 登录未返回有效凭据，请重试。"
        return
      }
      let user = try await SkinCommunityAPI.signIn(try self.rust(), cancelSnapshot: self.cancelSnapshot) { account in
        try await account.account(["operation": "google_token_login", "challenge_id": challenge.challenge_id, "id_token": token])
      }
      try self.ensureCurrent(id)
      self.user = user
      self.signedInVia = "Google"
    }
    #endif
  }

  /// Google 的登录页要从最上层正在显示的控制器弹出，也就是登录面板自己。
  private static func presenter() -> UIViewController? {
    let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
    var top = scenes.first(where: { $0.activationState == .foregroundActive })?.keyWindow?.rootViewController
    while let presented = top?.presentedViewController { top = presented }
    return top
  }

  private func perform(_ operation: (UUID) async throws -> Void) async {
    guard !busy else { return }
    let id = generation.current()
    busy = true
    message = nil
    defer { if generation.isCurrent(id) { busy = false } }
    do {
      try ensureCurrent(id)
      try await operation(id)
    }
    catch is CancellationError {
    }
    catch {
      guard generation.isCurrent(id) else { return }
      report(error)
    }
  }

  private func ensureCurrent(_ id: UUID) throws {
    try generation.checkCurrent(id)
  }

  /// 取消时不提示；后端或社区拒绝时说明原因；其余情况都视为连接没有完成。
  private func report(_ error: Error) {
    if isLoginCancellation(error) { return }
    switch error {
    case let error as BackendRustClient.Failure: message = error.localizedDescription
    case let error as CommunityFailure: message = error.localizedDescription
    #if canImport(GoogleSignIn)
    case is GIDSignInError: message = "Google 登录没有完成，请重试。"
    #endif
    default: message = "连接未完成，请检查网络后重试。"
    }
  }
}

enum CodeLoginChannel: String, CaseIterable, Identifiable {
  case email, phone
  var id: String { rawValue }
  var title: String { self == .email ? "邮箱登录" : "手机号登录" }
  /// 「已通过 … 登录」提示里的登录方式名称。
  var via: String { self == .email ? "邮箱" : "手机号" }
}

struct CodeLoginView: View {
  @ObservedObject var model: CodeLoginModel
  let channel: CodeLoginChannel
  @Environment(\.dismiss) private var dismiss
  @State private var target = ""
  @State private var code = ""
  @State private var challenge: BackendRustAccount.Challenge?
  @State private var expiresAt = Date.distantPast
  @State private var resendAt = Date.distantPast
  @State private var pending: Task<Void, Never>?

  var body: some View {
    NavigationStack {
      Form {
        Section {
          TextField(channel == .email ? "邮箱地址" : "手机号（含国家区号，如 +86）", text: $target)
            .keyboardType(channel == .email ? .emailAddress : .phonePad)
            .textContentType(channel == .email ? .emailAddress : .telephoneNumber)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .accessibilityIdentifier("backendCodeTarget")
            .onChange(of: target) { challenge = nil; code = "" }
          TimelineView(.periodic(from: .now, by: 1)) { timeline in
            let seconds = VerificationCodePolicy.remainingSeconds(until: resendAt, now: timeline.date)
            Button(seconds == 0 ? "获取验证码" : "\(seconds) 秒后可重新发送") {
              pending = Task {
                let response = await model.requestCode(provider: channel.rawValue,
                  target: target.trimmingCharacters(in: .whitespacesAndNewlines))
                guard !Task.isCancelled, let response else { return }
                challenge = response; code = ""
                expiresAt = Date().addingTimeInterval(TimeInterval(response.expires_in))
                resendAt = Date().addingTimeInterval(60)
              }
            }
            .disabled(seconds > 0 || target.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
          }
          if let challenge {
            TextField("6 位验证码", text: $code)
              .keyboardType(.numberPad)
              .textContentType(.oneTimeCode)
              .accessibilityIdentifier("backendVerificationCode")
            TimelineView(.periodic(from: .now, by: 1)) { timeline in
              let expired = expiresAt <= timeline.date
              Button(expired ? "验证码已过期，请重新获取" : "登录") {
                pending = Task { await model.signInWithCode(challenge: challenge.challenge_id, code: code, channel: channel) }
              }
              .disabled(expired || code.utf8.count != 6 || !code.utf8.allSatisfy { (48...57).contains($0) })
            }
          }
        } footer: {
          Text("验证码只用于本次登录，请勿向他人透露。")
        }
        if model.busy { ProgressView("正在处理…") }
        if let message = model.message { Text(message).foregroundStyle(.secondary) }
      }
      .disabled(model.busy)
      .designPage()
      .fittedSheet()
      .navigationTitle(channel.title)
      .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { ViewRequestTask.cancel(task: $pending); dismiss() } } }
    }
    .onChange(of: model.user?.id) { _, userID in if userID != nil { dismiss() } }
    .onDisappear {
      ViewRequestTask.cancel(task: $pending)
      model.invalidate()
    }
  }
}
