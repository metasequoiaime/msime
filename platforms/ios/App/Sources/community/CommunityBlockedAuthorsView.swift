import SwiftUI

/// 我的 › 隐私 › 已屏蔽的作者：登录账号屏蔽的作者，最近的在前，每行是作者名、屏蔽日期和「取消屏蔽」。只在登录后从隐私页推入；取消屏蔽后发 `CommunityBlocks.didChange`，社区列表重新读取，这位作者的作品回到列表里。
struct CommunityBlockedAuthorsView: View {
  @State private var blocks: [CommunityBlock]?
  @State private var failed = false
  @State private var removing: Set<String> = []
  @State private var message: String?

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        if let blocks, !blocks.isEmpty {
          DesignGroup(footer: "取消屏蔽后，这位作者的作品会重新出现在社区里。", radius: MetasequoiaTheme.tabCardRadius) {
            ForEach(Array(blocks.enumerated()), id: \.element.id) { index, block in
              if index > 0 { DesignDivider() }
              row(block)
            }
          }
        } else {
          Text(status)
            .designSubtext()
            .multilineTextAlignment(.center)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity)
            .padding(.horizontal, 20)
            .padding(.top, 40)
            .accessibilityIdentifier("blockedAuthorsStatus")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .designCanvasBackground()
    .navigationTitle("已屏蔽的作者").navigationBarTitleDisplayMode(.inline)
    .task { await load() }
    .refreshable { await load() }
    .alert("取消屏蔽未完成", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }

  private var status: String {
    if failed { return "暂时无法读取已屏蔽的作者，请下拉重试。" }
    if blocks == nil { return "正在读取…" }
    return "没有屏蔽任何作者。在社区作品详情里点「屏蔽作者」，就不会再看到这位作者的作品。"
  }

  private func row(_ block: CommunityBlock) -> some View {
    HStack(spacing: 12) {
      VStack(alignment: .leading, spacing: 2) {
        Text(block.name).font(.system(size: 17)).foregroundStyle(.primary)
        if !block.day.isEmpty { Text(block.day).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub) }
      }
      Spacer(minLength: 8)
      Button("取消屏蔽") { unblock(block) }
        .font(.system(size: 15))
        .disabled(removing.contains(block.id))
        .accessibilityIdentifier("unblockCommunityAuthor")
    }
    .padding(.vertical, 8)
    .padding(.horizontal, 20)
    .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
  }

  @MainActor private func load() async {
    do {
      blocks = try await SkinCommunityAPI.shared.blocks()
      failed = false
    } catch is CancellationError {
    } catch {
      // 读过的列表留着；只有从没读到过时才换成失败提示。
      if blocks == nil { failed = true }
    }
  }

  private func unblock(_ block: CommunityBlock) {
    removing.insert(block.id)
    Task { @MainActor in
      defer { removing.remove(block.id) }
      do {
        try await SkinCommunityAPI.shared.unblock(block.id)
        blocks?.removeAll { $0.id == block.id }
      } catch is CancellationError {
      } catch { message = error.localizedDescription }
    }
  }
}
