import SwiftUI
import UIKit

/// 任务取消不是错误。切换标签页会让 `.task` 连同它已经发出的请求一起取消,而 URLSession 抛回来的是 `URLError.cancelled` —— 它的 `localizedDescription` 就是 "cancelled"。把它当错误弹出来,结果就是每次切到「我的」都跳一个写着 cancelled 的框。
///
/// 只判 `CancellationError` 不够:那是 Swift 结构化并发自己抛的那一种,而这里真正会抛的是网络层那一种,两者没有继承关系。
private func isCancellation(_ error: Error) -> Bool {
  if error is CancellationError { return true }
  if let error = error as? URLError { return error.code == .cancelled }
  return false
}

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

/// 设计稿中「我的」子页面的行：16pt 标签，可带次要色的 15pt 值和右箭头，最低 50pt 高，左右内边距 16pt。
private struct ProfileRowLabel: View {
  let title: String
  var value: String? = nil
  var chevron = true

  var body: some View {
    HStack(spacing: 8) {
      Text(title).font(.system(size: 16)).foregroundStyle(.primary)
      Spacer(minLength: 12)
      if let value {
        Text(value).font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.sub)
          .lineLimit(1).truncationMode(.middle)
      }
      if chevron {
        Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold))
          .foregroundStyle(MetasequoiaTheme.sub.opacity(0.55))
          .accessibilityHidden(true)
      }
    }
    .padding(.horizontal, 16)
    .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
    .contentShape(Rectangle())
    .accessibilityElement(children: .combine)
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

  private var displayName: String {
    profile?.user.preferredDisplayName ?? initialUser?.preferredDisplayName ?? "水杉用户"
  }

  /// 邮箱登录身份的地址。只显示看起来像地址的 subject；其他登录方式的 subject 是不透明的 id。
  private var email: String? {
    profile?.identities.first { $0.provider == "email" && Self.looksLikeEmail($0.subject) }?.subject
  }

  /// 已关联的登录方式，按后端列出的顺序，每种只出现一次。
  private var providers: [String] {
    var seen = Set<String>()
    return (profile?.identities ?? []).map(\.provider).filter { seen.insert($0).inserted }
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
              .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
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
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
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
    .task { await load() }
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
        ProfileRowLabel(title: "昵称", value: profile.user.preferredDisplayName)
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("accountNicknameLink")
      DesignDivider()
      Button {
        UIPasteboard.general.string = profile.user.id
        ToastCenter.shared.show("已复制")
      } label: {
        ProfileRowLabel(title: "水杉 ID", value: profile.user.id, chevron: false)
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityHint("轻点复制完整 ID")
      .accessibilityIdentifier("copyAccountID")
      if let email {
        DesignDivider()
        ProfileRowLabel(title: "邮箱", value: email, chevron: false)
      }
    }
  }

  private var loginMethods: some View {
    // 设计稿的页脚承诺可以关联更多登录方式，iOS 还做不到，所以这一组只列出已关联的。
    DesignGroup(title: "登录方式", radius: MetasequoiaTheme.tabCardRadius) {
      ForEach(Array(providers.enumerated()), id: \.element) { index, provider in
        if index > 0 { DesignDivider() }
        ProfileRowLabel(title: providerTitle(provider), value: "已关联", chevron: false)
      }
    }
  }

  /// 「退出所有设备」，以及用保存的登录读不到资料时的「重新登录」。
  private var accountActions: some View {
    DesignGroup(title: "账号操作", radius: MetasequoiaTheme.tabCardRadius) {
      Button { confirmLogoutAll = true } label: {
        ProfileRowLabel(title: "退出所有设备", chevron: false)
      }
      .buttonStyle(PressFillButtonStyle())
      .disabled(busy)
      .accessibilityIdentifier("signOutEverywhere")
      if needsRecovery {
        DesignDivider()
        Button { confirmRelogin = true } label: {
          ProfileRowLabel(title: "重新登录", chevron: false)
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
        Button("重新加载") { Task { await load() } }
          .font(.subheadline.weight(.semibold))
          .foregroundStyle(MetasequoiaTheme.accent)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding(16)
    }
    .accessibilityIdentifier("accountProfileError")
  }

  /// 四个动作都以「这台设备不再登录」收尾,所以走同一条路:调用、回调父视图、退回上一页。失败时留在原地把原因说出来。
  private func perform(_ action: @escaping () async throws -> Void) {
    busy = true
    actionMessage = nil
    Task {
      defer { busy = false }
      do {
        try await action()
        onSignedOut()
        dismiss()
      } catch {
        guard !isCancellation(error) else { return }
        actionMessage = error.localizedDescription
      }
    }
  }

  /// 每次页面出现都重新读取资料，所以从「昵称」返回后会刷新；只有还没有内容可显示时才显示加载指示。
  @MainActor private func load() async {
    loading = profile == nil
    loadMessage = nil
    defer { loading = false }
    do {
      let result = try await SkinCommunityAPI.shared.profile()
      profile = result
      needsRecovery = false
      onSaved(result.user)
    } catch {
      guard !isCancellation(error) else { return }
      needsRecovery = true
      loadMessage = error.localizedDescription
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

  init(profile: CommunityProfile, onSaved: @escaping (CommunityProfile) -> Void) {
    self.profile = profile
    self.onSaved = onSaved
    _name = State(initialValue: profile.user.preferredDisplayName)
  }

  private var normalizedName: String { CommunityProfilePolicy.normalizedName(name) }
  private var validName: Bool { CommunityProfilePolicy.validName(name) }
  private var hasChanges: Bool { normalizedName != profile.user.preferredDisplayName }
  private var nameHint: String {
    if normalizedName.isEmpty { return "取一个喜欢的名字，让大家记住你。" }
    if !validName { return "昵称最多 64 个字符，请勿使用换行或控制字符。" }
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
          Text("\(normalizedName.unicodeScalars.count)/64")
            .monospacedDigit().fixedSize()
            .accessibilityLabel("已输入 \(normalizedName.unicodeScalars.count) 个字符，最多 64 个")
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
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
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
    .onAppear { editing = true }
  }

  private func save() {
    guard !saving else { return }
    guard hasChanges else { dismiss(); return }
    guard validName else { return }
    editing = false
    saving = true
    message = nil
    let submittedName = normalizedName
    Task {
      defer { saving = false }
      do {
        let updated = try await SkinCommunityAPI.shared.updateProfile(name: submittedName)
        onSaved(updated)
        ToastCenter.shared.show("昵称已更新")
        dismiss()
      } catch {
        guard !isCancellation(error) else { return }
        message = error.localizedDescription
      }
    }
  }
}
