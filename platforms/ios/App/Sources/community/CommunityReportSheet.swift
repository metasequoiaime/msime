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
    NavigationView {
      Form {
        Section("举报原因") {
          Picker("原因", selection: $reason) {
            ForEach(BackendAccountClient.reportReasons, id: \.self) { Text($0).tag($0) }
          }.pickerStyle(.inline).labelsHidden()
        }
        Section {
          TextField("补充说明（选填）", text: $detail, axis: .vertical).lineLimit(3...6)
            .onChange(of: detail) { if $0.unicodeScalars.count > 1_000 { detail = String(String.UnicodeScalarView($0.unicodeScalars.prefix(1_000))) } }
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
        .navigationTitle("举报").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() }.disabled(busy) } }
        .alert("已收到举报", isPresented: $sent) { Button("好") { dismiss() } } message: { Text("感谢反馈，我们会尽快核实处理。") }
        .alert("举报未完成", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
          Button("好", role: .cancel) {}
        } message: { Text(message ?? "") }
    }.interactiveDismissDisabled(busy)
  }
}
