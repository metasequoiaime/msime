import SwiftUI

/// The keyboard's local clipboard history, searchable here the way the Windows clipboard page is.
///
/// A keyboard extension cannot type into a search field of its own, so the search Windows puts above its clipboard list lives in the App instead. Both read and write the same App Group file through the shared clipboard ABI, so pins, deletions and saves made here show up the next time the keyboard's clipboard page opens, and the other way round. The App saves the system clipboard through `PasteButton`, which reads it without the paste permission prompt.
struct ClipboardHistorySettingsView: View {
  var store = ClipboardHistoryStore()
  @Environment(\.scenePhase) private var scenePhase
  @Environment(\.horizontalSizeClass) private var sizeClass
  @State private var items: [ClipboardHistoryItem] = []
  @State private var query = ""
  @State private var message: String?
  @State private var confirmsClear = false

  private var visible: [ClipboardHistoryItem] { ClipboardHistoryItem.matching(items, query: query) }

  var body: some View {
    Form {
      Section {
        PasteButton(payloadType: String.self) { strings in
          let text = strings.first ?? ""
          DispatchQueue.main.async { perform("已保存") { try store.add(text) } }
        }
        .accessibilityIdentifier("clipboardHistorySavePaste")
      } footer: {
        Text("键盘工具面板里的“剪贴板历史”保存的内容也在这里。记录只保存在本机，最多 50 条，固定的不会被新记录挤掉。")
      }
      .designRow()

      Section {
        ForEach(visible) { item in
          Button { copy(item) } label: { row(item) }
            .buttonStyle(.plain)
            .swipeActions(edge: .trailing) {
              Button("删除", role: .destructive) { perform { try store.remove(text: item.text) } }
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
              Button(role: .destructive) { perform { try store.remove(text: item.text) } } label: {
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
        SettingsGroupHeader(title: query.isEmpty ? "\(items.count)/\(ClipboardHistoryStore.limit) 条" : "\(visible.count) 条匹配")
      } footer: {
        if let message { Text(message) } else { Text(sizeClass == .regular ? "点一条复制到系统剪贴板，右键或长按可固定、删除。" : "点一条复制到系统剪贴板，右滑固定，左滑删除。") }
      }
      .designRow()
    }
    .designPage()
    .searchable(text: $query, prompt: "搜索剪贴板")
    .navigationTitle("剪贴板历史").navigationBarTitleDisplayMode(.inline)
    .confirmationDialog("清空全部剪贴板历史？", isPresented: $confirmsClear, titleVisibility: .visible) {
      Button("清空，包括固定项", role: .destructive) { perform { try store.clear() } }
    }
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
  }

  /// iPad has the width for a longer excerpt and the full time; the phone keeps rows short so more of the history fits on screen.
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

  private func reload() {
    do { items = try store.load() } catch { message = error.localizedDescription }
  }
}
