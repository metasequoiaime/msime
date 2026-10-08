import SwiftUI
import UIKit

struct CloudClipboardView: View {
  let session: BackendAccountSession
  let client: BackendAccountClient
  @State private var accountID: String?
  @State private var enabled = false
  @State private var loaded = false
  @State private var retention: Int?
  @State private var items: [BackendAccountClient.ClipboardItem] = []
  @State private var search = ""
  @State private var text = ""
  @State private var busy = false
  @State private var message: String?
  @State private var confirmsClear = false
  @State private var addsText = false
  @State private var pending: Task<Void, Never>?

  private static let retentionChoices: [(title: String, value: Int)] = [("1 天", 1), ("7 天", 7), ("30 天", 30), ("一直", 0)]

  private static let isoFractional: ISO8601DateFormatter = {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter
  }()
  private static let iso = ISO8601DateFormatter()
  private static let relative: RelativeDateTimeFormatter = {
    let formatter = RelativeDateTimeFormatter()
    formatter.locale = Locale(identifier: "zh-Hans")
    formatter.unitsStyle = .full
    return formatter
  }()

  private var canUpload: Bool {
    !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && text.utf16.count <= 4000
  }

  /// 置顶的排在前面；两部分各自保持服务端的顺序，最新的在前。
  private var orderedItems: [BackendAccountClient.ClipboardItem] {
    items.filter { $0.pinned == true } + items.filter { $0.pinned != true }
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 24) {
        if loaded {
          settingsCard
          if enabled { recent }
        } else if !busy {
          DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
            Button { run { _ in } } label: {
              HStack {
                VStack(alignment: .leading, spacing: 2) {
                  Text("重试").font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.accent)
                  Text("没能读到云端内容").font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
                }
                Spacer()
                Image(systemName: "arrow.clockwise").foregroundStyle(MetasequoiaTheme.accent).accessibilityHidden(true)
              }
              .padding(.horizontal, 16)
              .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
              .contentShape(Rectangle())
            }
            .buttonStyle(PressFillButtonStyle())
            .accessibilityIdentifier("cloudClipboardRetry")
          }
        }
      }
      .padding(.horizontal, 16).padding(.top, 8).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .searchable(text: $search, prompt: "搜索云端内容")
    .onSubmit(of: .search) { run { _ in } }
    .settingsStatus(busy: busy, message: message)
    .disabled(busy)
    .navigationTitle("云剪贴板")
    .navigationBarTitleDisplayMode(.inline)
    .toolbar {
      if loaded && enabled {
        ToolbarItem(placement: .topBarTrailing) {
          Button { addsText = true } label: { Image(systemName: "plus") }
            .accessibilityLabel("添加内容")
            .accessibilityIdentifier("cloudClipboardAdd")
        }
      }
    }
    .sheet(isPresented: $addsText) { addSheet }
    .task { run { _ in } }
    .onDisappear { pending?.cancel(); items = []; text = "" }
    .confirmationDialog("清空云剪贴板？", isPresented: $confirmsClear, titleVisibility: .visible) {
      Button("确认清空", role: .destructive) {
        run { token in try await client.deleteClipboard(token: token) }
      }
      Button("取消", role: .cancel) { }
    }
  }

  /// 开关，以及服务端返回保留期限时的保留期选项。
  private var settingsCard: some View {
    DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
      Toggle(isOn: Binding(
        get: { enabled },
        set: { on in run { token in try await client.setClipboardEnabled(on, token: token) } }
      )) {
        VStack(alignment: .leading, spacing: 2) {
          Text("云剪贴板").font(.system(size: 16)).foregroundStyle(.primary)
          Text("在你登录的设备之间同步，经 HTTPS 传输，保存在水杉云，关闭即删除")
            .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
      .toggleStyle(.switch)
      .tint(MetasequoiaTheme.switchOn)
      .padding(.horizontal, 16).padding(.vertical, 10)
      .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
      .accessibilityIdentifier("cloudClipboardSwitch")
      if let retention {
        DesignDivider()
        HStack(spacing: 12) {
          Text("保留时长").font(.system(size: 16)).foregroundStyle(.primary)
          Spacer(minLength: 8)
          DesignSegmentedControl(
            items: Self.retentionChoices,
            selection: Binding(
              get: { retention },
              set: { days in
                guard days != retention else { return }
                run { token in try await client.setClipboardRetention(days: days, token: token) }
              }
            ),
            identifierPrefix: "cloudClipboardRetention"
          )
          .frame(maxWidth: 230)
          .disabled(!enabled)
          .opacity(enabled ? 1 : 0.5)
        }
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("保留时长")
      }
    }
  }

  /// 最近：带「清空」的标题，下面是条目或空状态卡片。
  private var recent: some View {
    VStack(alignment: .leading, spacing: 7) {
      HStack {
        Text("最近").font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.groupTitle)
          .accessibilityAddTraits(.isHeader)
        Spacer()
        if !items.isEmpty {
          Button("清空") { confirmsClear = true }
            .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.accent)
            .accessibilityIdentifier("cloudClipboardClear")
        }
      }
      .padding(.horizontal, 16)
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
        if items.isEmpty {
          emptyState
        } else {
          let ordered = orderedItems
          ForEach(Array(ordered.enumerated()), id: \.element.id) { index, item in
            if index > 0 { DesignDivider() }
            row(item)
          }
        }
      }
    }
  }

  private var emptyState: some View {
    VStack(spacing: 6) {
      if search.isEmpty {
        Text("还没有同步内容").font(.system(size: 15)).foregroundStyle(.primary)
        Text("在任一设备上复制文字，这里就会出现").font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
      } else {
        Text("没有匹配「\(search)」的内容").font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.sub)
      }
    }
    .multilineTextAlignment(.center)
    .frame(maxWidth: .infinity)
    .padding(.horizontal, 16).padding(.vertical, 36)
    .accessibilityElement(children: .combine)
  }

  private func row(_ item: BackendAccountClient.ClipboardItem) -> some View {
    let pinned = item.pinned == true
    return HStack(alignment: .top, spacing: 4) {
      Button {
        UIPasteboard.general.string = item.text
        ToastCenter.shared.show("已复制")
      } label: {
        VStack(alignment: .leading, spacing: 4) {
          Text(item.text).font(.system(size: 15)).foregroundStyle(.primary)
            .lineLimit(2).multilineTextAlignment(.leading)
          let line = meta(item)
          if !line.isEmpty {
            Text(line).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
          }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityHint("轻点复制")
      Button {
        run { token in try await client.setClipboardPinned(id: item.id, pinned: !pinned, token: token) }
      } label: {
        Image(systemName: pinned ? "pin.fill" : "pin").font(.system(size: 15))
          .foregroundStyle(pinned ? MetasequoiaTheme.accent : MetasequoiaTheme.sub)
          .frame(width: 30, height: 30)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityLabel(pinned ? "取消置顶" : "置顶")
      Button {
        run { token in try await client.deleteClipboard(id: item.id, token: token) }
      } label: {
        Image(systemName: "trash").font(.system(size: 15))
          .foregroundStyle(MetasequoiaTheme.sub)
          .frame(width: 30, height: 30)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityLabel("删除")
    }
    .padding(.leading, 16).padding(.trailing, 10).padding(.vertical, 12)
    .frame(maxWidth: .infinity, alignment: .leading)
    .accessibilityElement(children: .contain)
  }

  /// '已置顶 · <device> · <relative time>'，未知的部分省略。
  private func meta(_ item: BackendAccountClient.ClipboardItem) -> String {
    var parts: [String] = []
    if item.pinned == true { parts.append("已置顶") }
    if let device = item.device { parts.append(device) }
    if let date = Self.isoFractional.date(from: item.updated_at) ?? Self.iso.date(from: item.updated_at) {
      parts.append(Self.relative.localizedString(for: min(date, Date()), relativeTo: Date()))
    }
    return parts.joined(separator: " · ")
  }

  /// 添加内容：手动上传一段文本的编辑器。
  private var addSheet: some View {
    NavigationStack {
      Form {
        Section {
          TextEditor(text: $text).frame(minHeight: 160).accessibilityIdentifier("cloudClipboardText")
        } footer: {
          VStack(alignment: .leading, spacing: 4) {
            Text("\(text.utf16.count) / 4000")
              .foregroundStyle(text.utf16.count > 4000 ? MetasequoiaTheme.danger : MetasequoiaTheme.sub)
              .monospacedDigit()
            Text("只上传你在这里明确添加的内容，不自动读取系统剪贴板。最多保存 50 条。")
          }
        }
        .designRow()
      }
      .designPage()
      .navigationTitle("添加内容")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button("取消") { addsText = false }
        }
        ToolbarItem(placement: .confirmationAction) {
          Button("上传") {
            let value = text
            addsText = false
            run { token in
              _ = try await client.addClipboard(value, token: token)
              text = ""
            }
          }
          .fontWeight(.semibold)
          .disabled(!canUpload || busy)
          .accessibilityIdentifier("cloudClipboardUpload")
        }
      }
    }
    .tint(MetasequoiaTheme.accent)
    .presentationDetents([.medium, .large])
  }

  @MainActor private func run(_ action: @escaping (String) async throws -> Void) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task { await execute(action) }
  }
  @MainActor private func execute(_ action: (String) async throws -> Void) async {
    defer { busy = false }
    do {
      let identity = try await session.credentials(matchingUserID: accountID)
      try Task.checkCancellation()
      accountID = identity.userID
      try await action(identity.token)
      let page = try await client.clipboard(token: identity.token, search: search)
      _ = try await session.credentials(matchingUserID: identity.userID)
      try Task.checkCancellation()
      enabled = page.enabled; retention = page.retention_days; items = page.items; loaded = true
    } catch is CancellationError { items = []; text = ""; loaded = false }
    catch let error as BackendAccountClient.Failure { items = []; loaded = false; message = error.localizedDescription }
    catch { items = []; loaded = false; message = "连接未完成，请检查网络后重试。" }
  }
}
