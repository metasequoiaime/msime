import SwiftUI

/// The keyboard's local clipboard history, searchable here the way the Windows clipboard page is.
///
/// A keyboard extension cannot type into a search field of its own, so the search Windows puts above its clipboard list lives in the App instead. Both read and write the same App Group file through the shared clipboard ABI, so pins, deletions and saves made here show up the next time the keyboard's clipboard page opens, and the other way round. The App saves the system clipboard through `PasteButton`, which reads it without the paste permission prompt.
struct ClipboardHistorySettingsView: View {
  var store = ClipboardHistoryStore()
  @Environment(\.horizontalSizeClass) private var sizeClass
  @State private var items: [ClipboardHistoryItem] = []
  @State private var query = ""
  @State private var message: String?
  @State private var confirmsClear = false
  @State private var historyLimit = 50
  /// 「剪贴板删除数据验证」（#10408）：只在本设备，存在 App Group（`ClipboardDeleteAuthenticationStore.shared`），键盘删除前读它。
  var deleteAuthenticationStore = ClipboardDeleteAuthenticationStore.shared
  @State private var deleteAuthentication = false
  @State private var authenticating = false
  @State private var authenticator = ClipboardDeleteAuthenticator()

  private var visible: [ClipboardHistoryItem] { ClipboardHistoryItem.matching(items, query: query) }

  var body: some View {
    Form {
      Section {
        Picker("历史记录上限", selection: $historyLimit) {
          ForEach([50, 100, 200, 500, 1000], id: \.self) { limit in
            Text("\(limit) 条").tag(limit)
          }
        }
        .onChange(of: historyLimit) { _, value in
          guard MetasequoiaInputSessionBridge.updateSharedPreferences({ $0["clipboard_history_limit"] = value }) else {
            return
          }
        }
        PasteButton(payloadType: String.self) { strings in
          let text = strings.first ?? ""
          DispatchQueue.main.async { perform("已保存") { try store.add(text) } }
        }
        .accessibilityIdentifier("clipboardHistorySavePaste")
      } footer: {
        Text("键盘工具面板里的“剪贴板历史”保存的内容也在这里。记录只保存在本机，固定的不会被新记录挤掉。")
      }
      .designRow()

      Section {
        Toggle("剪贴板删除数据验证", isOn: Binding(get: { deleteAuthentication }, set: changeDeleteAuthentication))
          .disabled(authenticating)
          .accessibilityIdentifier("clipboardDeleteAuthentication")
      } footer: {
        Text("开启后，在键盘剪贴板或本页删除、清空记录都要先通过面容 ID、触控 ID 或锁屏密码验证；打开和关闭本开关也要验证。只对这台设备生效。")
      }
      .designRow()

      Section {
        ForEach(visible) { item in
          Button { copy(item) } label: { row(item) }
            .buttonStyle(.plain)
            .swipeActions(edge: .trailing) {
              Button("删除", role: .destructive) { deleteAfterAuthentication { try store.remove(text: item.text) } }
            }
            .swipeActions(edge: .leading) {
              Button(item.pinned ? "取消固定" : "固定") { perform { try store.setPinned(!item.pinned, text: item.text) } }
                .tint(.orange)
            }
            .contextMenu {
              Button { copy(item) } label: { Label("复制", systemImage: "doc.on.doc") }
              Button { perform { try store.setPinned(!item.pinned, text: item.text) } } label: {
                Label(item.pinned ? "取消固定" : "固定", systemImage: item.pinned ? "pin.slash" : "pin")
              }
              Button(role: .destructive) { deleteAfterAuthentication { try store.remove(text: item.text) } } label: {
                Label("删除", systemImage: "trash")
              }
            }
        }
        if visible.isEmpty {
          Text(query.isEmpty ? "暂无历史" : "没有匹配「\(query)」的记录").foregroundStyle(.secondary)
            .accessibilityIdentifier("clipboardHistoryEmpty")
        }
        if !items.isEmpty {
          SettingsActionRow(title: "清空历史", symbol: "trash.fill", destructive: true) { confirmsClear = true }
            .accessibilityIdentifier("clipboardHistoryClear")
        }
      } header: {
        SettingsGroupHeader(title: query.isEmpty ? "\(items.count)/\(historyLimit) 条" : "\(visible.count) 条匹配")
      } footer: {
        if let message { Text(message) } else { Text(sizeClass == .regular ? "点一条复制到系统剪贴板，右键或长按可固定、删除。" : "点一条复制到系统剪贴板，右滑固定，左滑删除。") }
      }
      .designRow()
    }
    .designPage()
    .searchable(text: $query, prompt: "搜索剪贴板")
    .navigationTitle("剪贴板历史").navigationBarTitleDisplayMode(.inline)
    .confirmationDialog("清空全部剪贴板历史？", isPresented: $confirmsClear, titleVisibility: .visible) {
      Button("清空，包括固定项", role: .destructive) { deleteAfterAuthentication { try store.clear() } }
    }
    .reloadOnAppearAndActive(reload)
    // 离开页面时结束进行中的认证；迟到的结果被丢掉、回调不再执行，所以开关在这里放开。
    .onDisappear {
      authenticator.cancel()
      authenticating = false
    }
  }

  /// iPad 展示较长的摘要和完整日期，手机缩短每条记录以容纳更多历史。
  private func row(_ item: ClipboardHistoryItem) -> some View {
    HStack(alignment: .top, spacing: 12) {
      VStack(alignment: .leading, spacing: 4) {
        Text(item.text).lineLimit(sizeClass == .regular ? 6 : 3).foregroundStyle(.primary)
        Text(item.date.formatted(date: sizeClass == .regular ? .long : .abbreviated, time: .shortened))
          .font(.caption).foregroundStyle(.secondary)
      }
      Spacer(minLength: 8)
      if item.pinned { Image(systemName: "pin.fill").font(.caption).foregroundStyle(.orange).accessibilityLabel("已固定") }
    }
    .contentShape(Rectangle())
  }

  private func copy(_ item: ClipboardHistoryItem) {
    UIPasteboard.general.string = item.text
    message = "已复制到系统剪贴板"
  }

  private func perform(_ done: String? = nil, _ action: () throws -> Void) {
    do {
      try action()
      message = done
    } catch {
      message = error.localizedDescription
    }
    reload()
  }

  /// 切换「剪贴板删除数据验证」：打开和关闭都先认证，第一次打开也一样；设备没设锁屏密码时拒绝。认证取消或失败时不保存，开关停在原值。
  private func changeDeleteAuthentication(_ next: Bool) {
    switch ClipboardDeleteAuthenticationPolicy.decide(
      enabled: deleteAuthentication, next: next, authenticationAvailable: authenticator.available())
    {
    case .unchanged:
      return
    case .unavailable:
      message = ClipboardDeleteAuthenticationPolicy.unavailableMessage
    case .authenticate:
      authenticating = true
      authenticator.authenticate(reason: next ? "打开剪贴板删除数据验证" : "关闭剪贴板删除数据验证") { outcome in
        authenticating = false
        guard outcome == .authenticated else {
          message = ClipboardDeleteAuthenticationPolicy.refusal(outcome, next: next)
          return
        }
        deleteAuthenticationStore.save(next)
        deleteAuthentication = deleteAuthenticationStore.enabled
        message = nil
      }
    }
  }

  /// 本页的删除和清空与键盘一样受「剪贴板删除数据验证」约束，否则键盘里拦住的删除在这里绕得过去。
  private func deleteAfterAuthentication(_ action: @escaping () throws -> Void) {
    switch ClipboardDeleteAuthenticationPolicy.gate(
      enabled: deleteAuthenticationStore.enabled, authenticationAvailable: authenticator.available())
    {
    case .delete:
      perform(nil, action)
    case .unavailable:
      message = ClipboardDeleteAuthenticationPolicy.unavailableMessage
    case .authenticate:
      authenticator.authenticate(reason: "删除剪贴板历史") { outcome in
        switch outcome {
        case .authenticated: perform(nil, action)
        case .unavailable: message = ClipboardDeleteAuthenticationPolicy.unavailableMessage
        case .failed: message = "未通过系统认证，剪贴板记录没有删除"
        }
      }
    }
  }

  private func reload() {
    deleteAuthentication = deleteAuthenticationStore.enabled
    do {
      let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
      let stored = preferences?["clipboard_history_limit"] as? Int ?? 50
      historyLimit = [50, 100, 200, 500, 1000].contains(stored) ? stored : 50
      items = try store.load()
    } catch { message = error.localizedDescription }
  }
}
