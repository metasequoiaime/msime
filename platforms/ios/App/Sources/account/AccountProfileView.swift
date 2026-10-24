import SwiftUI
import UIKit

/// 后端在 `identities` 中报告的登录方式的显示名称。
private func providerTitle(_ provider: String) -> String {
  switch provider {
  case "apple": return "Apple"
  case "google": return "Google"
  case "email": return "邮箱"
  case "phone", "sms": return "手机号"
  default: return provider
  }
}

/// 单独占一张卡片、居中的 16pt 红色行：「退出登录」和「注销账号」。
private struct ProfileDangerRow: View {
  let title: String
  let identifier: String
  let action: () -> Void

  var body: some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      Button(action: action) {
        Text(title).font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.danger)
          .frame(maxWidth: .infinity, minHeight: 50)
          .contentShape(Rectangle())
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier(identifier)
    }
  }
}

/// 个人资料页，从「我的」页的账号卡片推入：头像区、账号（昵称、水杉 ID、邮箱）、后端报告的登录方式，以及账号操作。上传头像、云端数据和关联账号需要 iOS 还没有的后端客户端，所以这里不提供。
struct AccountProfileEditor: View {
  var initialUser: CommunityUser? = nil
  var onSaved: (CommunityUser) -> Void
  var onSignedOut: () -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var profile: CommunityProfile?
  @State private var loading = true
  @State private var busy = false
  @State private var loadMessage: String?
  @State private var actionMessage: String?
  @State private var needsRecovery = false
  @State private var confirmSignOut = false
  @State private var confirmLogoutAll = false
  @State private var confirmRelogin = false
  @State private var confirmDeleteAccount = false
  @State private var operation: Task<Void, Never>?
  @State private var loadTask: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()

  private var displayName: String {
    profile?.user.preferredDisplayName ?? initialUser?.preferredDisplayName ?? "水杉用户"
  }

  /// 账号的邮箱。Rust 的资料只带 Google 身份的验证邮箱，邮箱登录的地址不在里面；只显示看起来像地址的值。
  private var email: String? {
    profile?.user.email.flatMap { Self.looksLikeEmail($0) ? $0 : nil }
  }

  /// 已关联的登录方式，按 Rust 列出的顺序，每种只出现一次。
  private var providers: [String] {
    var seen = Set<String>()
    return (profile?.providers ?? []).filter { seen.insert($0).inserted }
  }

  private static func looksLikeEmail(_ value: String) -> Bool {
    guard let at = value.firstIndex(of: "@"), at != value.startIndex else { return false }
    let domain = value[value.index(after: at)...]
    return domain.contains(".") && !domain.hasPrefix(".") && !domain.hasSuffix(".") && !value.contains(where: \.isWhitespace)
  }

  var body: some View {
    ScrollView {
      VStack(spacing: 24) {
        header
        if loading {
          ProgressView("正在加载资料")
            .font(.subheadline).frame(maxWidth: .infinity).padding(.vertical, 28)
        } else if let profile {
          accountGroup(profile)
          if !providers.isEmpty { loginMethods }
          accountActions
          ProfileDangerRow(title: "退出登录", identifier: "signOutAccount") { confirmSignOut = true }
            .disabled(busy)
          VStack(alignment: .leading, spacing: 7) {
            ProfileDangerRow(title: "注销账号", identifier: "deleteAccount") { confirmDeleteAccount = true }
              .disabled(busy)
            Text("注销后，已发布的皮肤、评分和其他云端账号数据会立即删除，无法撤销")
              .designSubtext()
              .fixedSize(horizontal: false, vertical: true)
              .padding(.horizontal, 16)
          }
        } else {
          if loadMessage != nil { loadFailure }
          accountActions
          ProfileDangerRow(title: "退出登录", identifier: "signOutAccount") { confirmSignOut = true }
            .disabled(busy)
        }
      }
      .padding(.horizontal, 16).padding(.top, 8).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .designCanvasBackground()
    .navigationTitle("个人资料")
    .navigationBarTitleDisplayMode(.inline)
    .overlay { if busy { ProgressView().controlSize(.large) } }
    .confirmationDialog("退出登录后，设置同步、云词库、云剪贴板和发布作品都需要重新登录才能使用。", isPresented: $confirmSignOut, titleVisibility: .visible) {
      Button("退出登录", role: .destructive) { perform { try await SkinCommunityAPI.shared.logout() } }
    }
    .confirmationDialog("退出所有设备后，所有设备都需要重新登录。", isPresented: $confirmLogoutAll, titleVisibility: .visible) {
      Button("退出所有设备", role: .destructive) { perform { try await SkinCommunityAPI.shared.logout(all: true) } }
    }
    .confirmationDialog("清掉本机的登录状态后需要重新登录一次。", isPresented: $confirmRelogin, titleVisibility: .visible) {
      Button("重新登录") { perform { try await SkinCommunityAPI.shared.clearExpiredLogin() } }
    }
    .confirmationDialog("注销账号将删除已发布皮肤、评分及其他云端账号数据，无法撤销。", isPresented: $confirmDeleteAccount, titleVisibility: .visible) {
      Button("注销账号", role: .destructive) { perform { try await SkinCommunityAPI.shared.logout(deleteAccount: true) } }
    }
    .alert("没有完成", isPresented: Binding(get: { actionMessage != nil }, set: { if !$0 { actionMessage = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(actionMessage ?? "") }
    .tint(MetasequoiaTheme.accent)
    .trackViewRequest(request: $request, pending: $operation, onAppear: {
      startLoad()
    }, onDisappear: {
      ViewRequestTask.cancel(task: $loadTask)
      busy = false
    })
  }

  /// 84pt 的强调色圆盘显示名字的首字，下面是名字、有邮箱时的邮箱，以及账号用哪种方式登录。
  private var header: some View {
    VStack(spacing: 0) {
      Text(String(displayName.prefix(1)))
        .font(.system(size: 34, weight: .bold))
        .foregroundStyle(MetasequoiaTheme.onAccent)
        .frame(width: 84, height: 84)
        .background(MetasequoiaTheme.accent, in: Circle())
        .accessibilityHidden(true)
      Text(displayName).font(.system(size: 22, weight: .bold))
        .multilineTextAlignment(.center).lineLimit(2)
        .padding(.top, 10)
      if let email {
        Text(email).font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
          .lineLimit(1).truncationMode(.middle)
          .padding(.top, 2)
      }
      if let provider = providers.first {
        Text("通过 \(providerTitle(provider)) 登录")
          .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.accent)
          .padding(.horizontal, 10).padding(.vertical, 3)
          .background(MetasequoiaTheme.accentSoft, in: Capsule())
          .padding(.top, 8)
      }
    }
    .frame(maxWidth: .infinity).padding(.top, 8)
    .accessibilityElement(children: .combine)
    .accessibilityIdentifier("accountProfilePreview")
  }

  private func accountGroup(_ profile: CommunityProfile) -> some View {
    DesignGroup(title: "账号", radius: MetasequoiaTheme.tabCardRadius) {
      NavigationLink {
        AccountNicknameEditor(profile: profile) { updated in
          self.profile = updated
          onSaved(updated.user)
        }
      } label: {
        SettingsNavigationLabel(title: "昵称", value: profile.user.preferredDisplayName,
          valueTruncationMode: .middle)
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("accountNicknameLink")
      DesignDivider()
      Button {
        UIPasteboard.general.string = profile.user.id
        ToastCenter.shared.show("已复制")
      } label: {
        SettingsNavigationLabel(title: "水杉 ID", value: profile.user.id, showsChevron: false,
          valueTruncationMode: .middle)
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityHint("轻点复制完整 ID")
      .accessibilityIdentifier("copyAccountID")
      if let email {
        DesignDivider()
        SettingsNavigationLabel(title: "邮箱", value: email, showsChevron: false,
          valueTruncationMode: .middle)
      }
    }
  }

  private var loginMethods: some View {
    // 设计稿的页脚承诺可以关联更多登录方式，iOS 还做不到，所以这一组只列出已关联的。
    DesignGroup(title: "登录方式", radius: MetasequoiaTheme.tabCardRadius) {
      ForEach(Array(providers.enumerated()), id: \.element) { index, provider in
        if index > 0 { DesignDivider() }
        SettingsNavigationLabel(title: providerTitle(provider), value: "已关联", showsChevron: false,
          valueTruncationMode: .middle)
      }
    }
  }

  /// 「退出所有设备」，以及用保存的登录读不到资料时的「重新登录」。
  private var accountActions: some View {
    DesignGroup(title: "账号操作", radius: MetasequoiaTheme.tabCardRadius) {
      Button { confirmLogoutAll = true } label: {
        SettingsNavigationLabel(title: "退出所有设备", showsChevron: false)
      }
      .buttonStyle(PressFillButtonStyle())
      .disabled(busy)
      .accessibilityIdentifier("signOutEverywhere")
      if needsRecovery {
        DesignDivider()
        Button { confirmRelogin = true } label: {
          SettingsNavigationLabel(title: "重新登录", showsChevron: false)
        }
        .buttonStyle(PressFillButtonStyle())
        .disabled(busy)
        .accessibilityIdentifier("clearExpiredLogin")
      }
    }
  }

  private var loadFailure: some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      VStack(alignment: .leading, spacing: 12) {
        Label(loadMessage ?? "", systemImage: "exclamationmark.circle")
          .font(.subheadline).foregroundStyle(MetasequoiaTheme.sub)
          .fixedSize(horizontal: false, vertical: true)
        Button("重新加载") { startLoad() }
          .font(.subheadline.weight(.semibold))
          .foregroundStyle(MetasequoiaTheme.accent)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding(16)
    }
    .accessibilityIdentifier("accountProfileError")
  }

  /// 四个动作都以「这台设备不再登录」收尾,所以走同一条路:调用、回调父视图、退回上一页。失败时留在原地把原因说出来；只有本机已经退出（Rust 退出时先清本机会话，断网时服务端没撤销成）时照样退回，再用一条提示说明原因。
  private func perform(_ action: @escaping () async throws -> Void) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $actionMessage,
                        pending: $operation, isCancellation: AccountRequestPolicy.isCancellation) { id in
      do {
        try await action()
      } catch {
        guard !AccountRequestPolicy.isCancellation(error),
              (try? await SkinCommunityAPI.shared.currentUser()) == .some(nil) else { throw error }
        try request.checkCurrent(id)
        onSignedOut()
        dismiss()
        ToastCenter.shared.show("已在本机退出。" + error.localizedDescription)
        return
      }
      try request.checkCurrent(id)
      onSignedOut()
      dismiss()
    }
  }

  /// 每次页面出现都重新读取资料，所以从「昵称」返回后会刷新；只有还没有内容可显示时才显示加载指示。
  @MainActor private func startLoad() {
    ViewRequestTask.startLoad(request: $request, pending: $loadTask) { id in await load(id: id) }
  }

  @MainActor private func load(id: UUID) async {
    guard request.isCurrent(id) else { return }
    loading = profile == nil
    loadMessage = nil
    defer { if request.isCurrent(id) { loading = false } }
    do {
      let result = try await SkinCommunityAPI.shared.profile()
      try request.checkCurrent(id)
      profile = result
      needsRecovery = false
      onSaved(result.user)
    } catch is CancellationError {
      return
    } catch {
      guard request.isCurrent(id), !AccountRequestPolicy.isCancellation(error) else { return }
      needsRecovery = true; loadMessage = error.localizedDescription
    }
  }
}

/// 「昵称」页：按 CommunityProfilePolicy 校验的昵称输入框，点「完成」保存。有未保存的修改时离开会先询问。
private struct AccountNicknameEditor: View {
  let profile: CommunityProfile
  let onSaved: (CommunityProfile) -> Void
  @Environment(\.dismiss) private var dismiss
  @FocusState private var editing: Bool
  @State private var name: String
  @State private var saving = false
  @State private var message: String?
  @State private var confirmDiscard = false
  @State private var operation: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()

  init(profile: CommunityProfile, onSaved: @escaping (CommunityProfile) -> Void) {
    self.profile = profile
    self.onSaved = onSaved
    _name = State(initialValue: profile.user.preferredDisplayName)
  }

  private var namePresentation: CommunityProfilePolicy.NamePresentation? { try? CommunityProfilePolicy.present(name) }
  private var normalizedName: String { namePresentation?.normalized ?? name }
  private var validName: Bool { namePresentation?.valid ?? false }
  private var hasChanges: Bool { normalizedName != profile.user.preferredDisplayName }
  private var nameHint: String {
    guard let presentation = namePresentation else { return "昵称无法校验，请缩短后重试。" }
    if normalizedName.isEmpty { return "取一个喜欢的名字，让大家记住你。" }
    if !validName { return "昵称最多 \(presentation.maximumCharacters) 个字符，请勿使用换行或控制字符。" }
    return "昵称会显示在社区作品中，已发布的作品也会同步更新。"
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 7) {
        DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
          HStack(spacing: 8) {
            TextField("设置你的昵称", text: $name)
              .font(.system(size: 16))
              .textContentType(.nickname).submitLabel(.done)
              .focused($editing).onSubmit(save)
              .disabled(saving)
              .accessibilityIdentifier("accountNicknameField")
            if !name.isEmpty {
              Button { name = ""; editing = true } label: {
                Image(systemName: "xmark.circle.fill").foregroundStyle(.tertiary)
                  .frame(width: 44, height: 44)
              }
              .buttonStyle(.plain).accessibilityLabel("清空昵称").disabled(saving)
            }
          }
          .padding(.leading, 16).padding(.trailing, 4)
          .frame(minHeight: 50)
        }
        HStack(alignment: .top, spacing: 16) {
          Text(nameHint).fixedSize(horizontal: false, vertical: true)
          Spacer(minLength: 0)
          if let presentation = namePresentation {
            Text("\(normalizedName.unicodeScalars.count)/\(presentation.maximumCharacters)")
              .monospacedDigit().fixedSize()
              .accessibilityLabel("已输入 \(normalizedName.unicodeScalars.count) 个字符，最多 \(presentation.maximumCharacters) 个")
          }
        }
        .font(.system(size: 13))
        .foregroundStyle(validName || normalizedName.isEmpty ? MetasequoiaTheme.sub : MetasequoiaTheme.danger)
        .padding(.horizontal, 16)
        if let message {
          Label(message, systemImage: "exclamationmark.circle")
            .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.danger)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 16).padding(.top, 4)
            .accessibilityIdentifier("accountProfileSaveError")
        }
      }
      .padding(.horizontal, 16).padding(.top, 20).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .designCanvasBackground()
    .navigationTitle("昵称")
    .navigationBarTitleDisplayMode(.inline)
    // 系统自带的返回按钮会直接返回而不询问，输入了一半的昵称会随之丢失。
    .navigationBarBackButtonHidden(true)
    .toolbar {
      ToolbarItem(placement: .navigationBarLeading) {
        Button {
          editing = false
          if hasChanges { confirmDiscard = true } else { dismiss() }
        } label: {
          Image(systemName: "chevron.backward").font(.body.weight(.semibold))
        }
        .accessibilityLabel("返回").disabled(saving)
      }
      ToolbarItem(placement: .navigationBarTrailing) {
        if saving {
          ProgressView()
        } else {
          Button("完成", action: save)
            .fontWeight(.semibold)
            .disabled(hasChanges && !validName)
            .accessibilityIdentifier("saveAccountProfile")
        }
      }
    }
    .confirmationDialog("要放弃这次修改吗？", isPresented: $confirmDiscard, titleVisibility: .visible) {
      Button("放弃修改", role: .destructive) { dismiss() }
      Button("继续编辑", role: .cancel) {}
    }
    .tint(MetasequoiaTheme.accent)
    .trackViewRequest(request: $request, pending: $operation, onAppear: {
      editing = true
    }, onDisappear: {
      saving = false
    })
  }

  private func save() {
    guard request.isVisible, !saving else { return }
    guard hasChanges else { dismiss(); return }
    guard validName else { return }
    editing = false
    let submittedName = normalizedName
    ViewRequestTask.run(request: $request, busy: $saving, message: $message,
                        pending: $operation, isCancellation: AccountRequestPolicy.isCancellation) { id in
      let updated = try await SkinCommunityAPI.shared.updateProfile(name: submittedName)
      try request.checkCurrent(id)
      onSaved(updated)
      ToastCenter.shared.show("昵称已更新")
      dismiss()
    }
  }
}
