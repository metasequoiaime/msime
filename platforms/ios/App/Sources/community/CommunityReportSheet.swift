import SwiftUI

/// The 已下架 mark on the author's own removed work. Never a reason and never a pending state: publishing is post-moderated, so a work is public until a moderator removes it.
struct CommunityRemovedBadge: View {
  var body: some View {
    Text("已下架").font(.system(size: 10, weight: .semibold)).foregroundStyle(.white)
      .padding(.horizontal, 6).padding(.vertical, 2)
      .background(Color.red.opacity(0.85), in: Capsule())
      .accessibilityIdentifier("communityRemovedBadge")
  }
}

/// 举报 on another user's work: one of the fixed reasons and an optional note, sent with the signed-in account or, without one, the device's anonymous account.
struct CommunityReportButton: View {
  /// skins, dictionaries or replies.
  let kind: String
  let itemID: String
  @State private var reporting = false
  var body: some View {
    Button { reporting = true } label: { Label("举报", systemImage: "exclamationmark.bubble") }
      .font(.subheadline).foregroundStyle(.secondary)
      .accessibilityIdentifier("reportCommunityWork")
      .sheet(isPresented: $reporting) { CommunityReportSheet(kind: kind, itemID: itemID) }
  }
}

/// 「屏蔽作者」：放在别人作品详情的「举报」旁边。只用登录账号，没登录时先请用户登录；确认后屏蔽，提示「已屏蔽」，用户点「好」后关掉详情，列表收到 `CommunityBlocks.didChange` 重新读取，这位作者的作品随之消失。在 我的 › 隐私 › 已屏蔽的作者 里取消。
struct CommunityBlockAuthorButton: View {
  /// 与举报相同的类别：skins、plugins、dictionaries、replies 或 phrases。
  let kind: String
  let itemID: String
  let author: String
  @Environment(\.dismiss) private var dismiss
  @State private var confirming = false
  @State private var asksSignIn = false
  @State private var signingIn = false
  @State private var busy = false
  @State private var blocked: String?
  @State private var message: String?
  var body: some View {
    Button { check() } label: { Label("屏蔽作者", systemImage: "person.crop.circle.badge.xmark") }
      .font(.subheadline).foregroundStyle(.secondary).disabled(busy)
      .accessibilityIdentifier("blockCommunityAuthor")
      .confirmationDialog("屏蔽作者「\(author)」？", isPresented: $confirming, titleVisibility: .visible) {
        Button("屏蔽", role: .destructive) { block() }
        Button("取消", role: .cancel) {}
      } message: {
        Text("屏蔽后，社区里不再显示这位作者发布的皮肤、插件、词库和模板。可以在「我的 › 隐私 › 已屏蔽的作者」里取消屏蔽。")
      }
      .alert(CommunityBlocks.signInRequired, isPresented: $asksSignIn) {
        Button("登录") { signingIn = true }
        Button("取消", role: .cancel) {}
      }
      .alert(blocked.map { "已屏蔽「\($0)」" } ?? "", isPresented: Binding(get: { blocked != nil }, set: { if !$0 { blocked = nil } })) {
        Button("好") { dismiss() }
      }
      .alert("屏蔽未完成", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
        Button("好", role: .cancel) {}
      } message: { Text(message ?? "") }
      .sheet(isPresented: $signingIn, onDismiss: { check(prompting: false) }) { AccountLoginSheet() }
  }
  /// 登录了就请用户确认，没登录就请用户先登录。`prompting` 为假时是登录面板刚关掉：登录成功就接着确认，没登录就到此为止，不再追问。
  private func check(prompting: Bool = true) {
    Task { @MainActor in
      let signedIn = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
      if signedIn { confirming = true } else if prompting { asksSignIn = true }
    }
  }
  private func block() {
    busy = true
    Task { @MainActor in
      defer { busy = false }
      do { blocked = try await SkinCommunityAPI.shared.block(kind: kind, itemID: itemID).name }
      catch is CancellationError {}
      catch { message = error.localizedDescription }
    }
  }
}

private struct CommunityReportSheet: View {
  let kind: String
  let itemID: String
  @Environment(\.dismiss) private var dismiss
  @State private var reason = BackendAccountClient.reportReasons[0]
  @State private var detail = ""
  @State private var busy = false
  @State private var message: String?
  @State private var sent = false
  var body: some View {
    NavigationStack {
      Form {
        Section("举报原因") {
          Picker("原因", selection: $reason) {
            ForEach(BackendAccountClient.reportReasons, id: \.self) { Text($0).tag($0) }
          }.pickerStyle(.inline).labelsHidden()
        }
        Section {
          TextField("补充说明（选填）", text: $detail, axis: .vertical).lineLimit(3...6)
            .onChange(of: detail) { _, value in if value.unicodeScalars.count > 1_000 { detail = String(String.UnicodeScalarView(value.unicodeScalars.prefix(1_000))) } }
        } footer: {
          Text("举报会发送作品编号、原因和你填写的说明，由管理员核实处理。")
        }
        Button("提交举报") {
          busy = true
          Task {
            defer { busy = false }
            do {
              try await SkinCommunityAPI.shared.report(kind: kind, itemID: itemID, reason: reason, detail: detail)
              sent = true
            } catch { message = error.localizedDescription }
          }
        }.disabled(busy).accessibilityIdentifier("submitCommunityReport")
        if busy { ProgressView() }
      }.disabled(busy)
        .designPage()
        .fittedSheet()
        .navigationTitle("举报").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() }.disabled(busy) } }
        .alert("已收到举报", isPresented: $sent) { Button("好") { dismiss() } } message: { Text("感谢反馈，我们会尽快核实处理。") }
        .alert("举报未完成", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
          Button("好", role: .cancel) {}
        } message: { Text(message ?? "") }
    }.interactiveDismissDisabled(busy)
  }
}
