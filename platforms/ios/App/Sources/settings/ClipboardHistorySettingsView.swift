import SwiftUI

/// 键盘与主应用通过共享 ABI 读写同一份本机历史；此页提供搜索、固定和显式开启自动采集。
/// 手动保存使用系统 PasteButton，自动采集只在已开启且键盘可见时读取。
struct ClipboardHistorySettingsView: View {
  var store = ClipboardHistoryStore()
  @Environment(\.horizontalSizeClass) private var sizeClass
  @State private var items: [ClipboardHistoryItem] = []
  @State private var query = ""
  @State private var message: String?
  @State private var confirmsClear = false
  @State private var historyLimit = 50
  @State private var automaticallyCapture = false

  private var visible: [ClipboardHistoryItem] { ClipboardHistoryItem.matching(items, query: query) }

  var body: some View {
    Form {
      Section {
        DesignToggleRow(title: "自动保存复制内容", info: "允许完全访问后，在键盘显示期间保存新复制的文本；隐私模式与凭据输入框不保存。iOS 不提供后台已覆盖的历史，可能需要允许来自其他应用的粘贴。",
                        isOn: Binding(get: { automaticallyCapture }, set: setAutomaticallyCapture))
          .accessibilityIdentifier("clipboardHistoryAutomaticCapture")
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
        DesignSectionHeader(subject: query.isEmpty ? "\(items.count)/\(historyLimit) 条" : "\(visible.count) 条匹配", info: "与键盘工具面板共用本机历史，固定项不会被新记录挤掉。" + (sizeClass == .regular ? "点一条复制到系统剪贴板，右键或长按可固定、删除。" : "点一条复制到系统剪贴板，右滑固定，左滑删除。")) {
          SettingsGroupHeader(title: query.isEmpty ? "\(items.count)/\(historyLimit) 条" : "\(visible.count) 条匹配")
        }
      } footer: {
        if let message { Text(message) }
      }
      .designRow()
    }
    .designPage()
    .searchable(text: $query, prompt: "搜索剪贴板")
    .navigationTitle("剪贴板历史").navigationBarTitleDisplayMode(.inline)
    .confirmationDialog("清空全部剪贴板历史？", isPresented: $confirmsClear, titleVisibility: .visible) {
      Button("清空，包括固定项", role: .destructive) { perform { try store.clear() } }
    }
    .reloadOnAppearAndActive(reload)
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

  private func setAutomaticallyCapture(_ value: Bool) {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences({ $0["clipboard_history"] = value }) else {
      message = SettingsSaveFailure.message
      return
    }
    automaticallyCapture = value
  }

  private func reload() {
    do {
      let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
      automaticallyCapture = preferences?["clipboard_history"] as? Bool == true
      let stored = preferences?["clipboard_history_limit"] as? Int ?? 50
      historyLimit = [50, 100, 200, 500, 1000].contains(stored) ? stored : 50
      items = try store.load()
    } catch { message = error.localizedDescription }
  }
}
