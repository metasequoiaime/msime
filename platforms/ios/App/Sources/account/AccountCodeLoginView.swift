import AuthenticationServices
import SwiftUI

/// 任务取消不是错误：关闭弹窗或离开标签页会取消正在进行的请求，URLSession 回的是 `URLError.cancelled`，它的描述就是 "cancelled"。只判 `CancellationError` 不够，因为网络层抛的是另一种。
private func isLoginCancellation(_ error: Error) -> Bool {
  if error is CancellationError { return true }
  if let error = error as? URLError { return error.code == .cancelled }
  return false
}

/// 登录弹窗共享的登录状态：后端提供哪些登录方式、待用的 Apple challenge、邮箱和手机号的验证码流程，以及刚完成的那次登录用的是哪种方式。
@MainActor
final class CodeLoginModel: ObservableObject {
  @Published var user: BackendAccountClient.User?
  @Published var providers: [String: Bool] = [:]
  /// 请求过登录方式列表后即为 true，不论回应是否已经到达。
  @Published private(set) var providersLoaded = false
  /// 下一次「通过 Apple 登录」请求携带的 Apple challenge；正在获取或不提供 Apple 登录时为 nil。
  @Published private(set) var appleChallenge: CommunityChallenge?
  @Published private(set) var preparingApple = false
  /// 上一次登录使用的方式（"Apple"、"邮箱"、"手机号"），只在登录成功时设置。
  @Published private(set) var signedInVia: String?
  @Published var busy = false
  @Published var message: String?
  let client = BackendAccountClient()
  let session = BackendAccountSession.shared
  private let api = SkinCommunityAPI.shared

  func loadProviders() async {
    await perform { self.providers = try await self.client.providers() }
    providersLoaded = true
  }

  /// 加载登录方式；其中有 Apple 时，同时获取第一个 Apple challenge。
  func prepare() async {
    await loadProviders()
    await prepareApple()
  }

  func prepareApple() async {
    appleChallenge = nil
    guard providers["apple"] == true else { return }
    preparingApple = true
    defer { preparingApple = false }
    do { appleChallenge = try await api.challenge() }
    catch { report(error) }
  }

  func requestCode(provider: String, target: String) async -> BackendAccountClient.Challenge? {
    var response: BackendAccountClient.Challenge?
    await perform {
      guard self.providers[provider] == true else { throw BackendAccountClient.Failure(status: 503) }
      response = try await self.client.challenge(provider: provider, target: target)
    }
    return response
  }

  func signInWithCode(challenge: String, code: String, channel: CodeLoginChannel) async {
    await perform {
      try await self.session.signIn(challenge: challenge, credential: code)
      self.user = try await self.session.user()
      if self.user != nil { self.signedInVia = channel.via }
    }
  }

  /// 完成「通过 Apple 登录」。凭证的 `state` 必须回传 challenge id，借此把 identity token 和本 App 向后端申请的 nonce 绑在一起。成功以外的任何结果都会重新获取 challenge，因为后端每个 challenge 只接受一次。
  func signInWithApple(_ result: Result<ASAuthorization, Error>) async {
    guard let challenge = appleChallenge else { return }
    switch result {
    case .success(let authorization):
      guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
            credential.state == challenge.challenge_id,
            let data = credential.identityToken, let token = String(data: data, encoding: .utf8) else {
        message = "Apple 登录未返回有效凭据，请重试。"
        await prepareApple()
        return
      }
      var failed = false
      await perform {
        do { try await self.api.login(challenge: challenge.challenge_id, identityToken: token) }
        catch { failed = true; throw error }
        self.user = try await self.api.currentUser()
        if self.user != nil { self.signedInVia = "Apple" }
      }
      if failed { await prepareApple() }
    case .failure(let error):
      if (error as? ASAuthorizationError)?.code != .canceled { message = error.localizedDescription }
      await prepareApple()
    }
  }

  private func perform(_ operation: () async throws -> Void) async {
    guard !busy else { return }
    busy = true; message = nil
    defer { busy = false }
    do { try await operation() }
    catch { report(error) }
  }

  /// 取消时不提示；后端或社区拒绝时说明原因；其余情况都视为连接没有完成。
  private func report(_ error: Error) {
    if isLoginCancellation(error) { return }
    switch error {
    case let error as BackendAccountClient.Failure: message = error.localizedDescription
    case let error as CommunityFailure: message = error.localizedDescription
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
  @State private var challenge: BackendAccountClient.Challenge?
  @State private var expiresAt = Date.distantPast
  @State private var resendAt = Date.distantPast
  @State private var pending: Task<Void, Never>?

  var body: some View {
    NavigationView {
      Form {
        Section {
          TextField(channel == .email ? "邮箱地址" : "手机号（含国家区号，如 +86）", text: $target)
            .keyboardType(channel == .email ? .emailAddress : .phonePad)
            .textContentType(channel == .email ? .emailAddress : .telephoneNumber)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .accessibilityIdentifier("backendCodeTarget")
            .onChange(of: target) { _ in challenge = nil; code = "" }
          TimelineView(.periodic(from: .now, by: 1)) { timeline in
            let seconds = max(0, Int(ceil(resendAt.timeIntervalSince(timeline.date))))
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
      .navigationTitle(channel.title)
      .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { pending?.cancel(); dismiss() } } }
    }
    .onChange(of: model.user?.id) { userID in if userID != nil { dismiss() } }
    .onDisappear { pending?.cancel() }
  }
}
