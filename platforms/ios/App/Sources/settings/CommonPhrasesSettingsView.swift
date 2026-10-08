import SwiftUI

/// 常用语，对应 Android 的 `PhrasesPage`：列出键盘 常用语 面板中那些不带编码的短语，每条都可删除，并提供 添加常用语 操作。它通过 `CommonPhrasesBridge` 读写 App Group 里同一份 `CommonPhrases.json`，所以键盘下次打开面板时就会显示改动。每次调用存储都要拿文件锁并读写磁盘，所以放在主线程之外执行。
struct CommonPhrasesSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var phrases: [CommonPhrasesBridge.Phrase]?
  @State private var loadFailed = false
  @State private var adding = false
  @State private var deleting: CommonPhrasesBridge.Phrase?
  @State private var busy = false

  var body: some View {
    List {
      Section {
        if let phrases {
          if phrases.isEmpty {
            note("还没有常用语。添加以后，在键盘的「常用语」面板里点一下就能发送。")
          }
          ForEach(Array(phrases.enumerated()), id: \.element.id) { index, phrase in
            Text(phrase.text).font(.system(size: 17)).foregroundStyle(.primary).lineLimit(3)
              .frame(maxWidth: .infinity, minHeight: 34, alignment: .leading)
              .accessibilityIdentifier("commonPhrase-\(index)")
              .swipeActions {
                Button("删除", role: .destructive) { deleting = phrase }
              }
              .designRow()
          }
        } else {
          note(loadFailed ? "常用语读取失败，请稍后重试。" : "正在读取常用语…")
        }
        Button { adding = true } label: {
          HStack(spacing: 8) {
            Image(systemName: "plus").font(.system(size: 16, weight: .semibold)).accessibilityHidden(true)
            Text("添加常用语").font(.system(size: 17))
            Spacer(minLength: 0)
          }
          .foregroundStyle(MetasequoiaTheme.accent)
          .frame(minHeight: 34)
          .contentShape(Rectangle())
        }
        .disabled(busy || phrases == nil)
        .accessibilityIdentifier("addCommonPhrase")
        .designRow()
      } footer: {
        Text("在键盘的「常用语」面板中，点一下即可发送。滑动一条可删除。")
      }
    }
    .designPage()
    .navigationTitle("常用语")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in if phase == .active { reload() } }
    .sheet(isPresented: $adding) {
      CommonPhraseEditor { text in run(success: "已添加") { try CommonPhrasesBridge.add(text) } }
    }
    .alert("删除这条常用语？", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
           presenting: deleting) { phrase in
      Button("取消", role: .cancel) {}
      Button("删除", role: .destructive) { run(success: "已删除") { try CommonPhrasesBridge.remove(id: phrase.id) } }
    } message: { phrase in
      Text(phrase.text)
    }
  }

  private func note(_ text: String) -> some View {
    Text(text).font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding(.vertical, 8)
      .designRow()
  }

  private func reload() {
    Task {
      let loaded = await Task.detached(priority: .userInitiated) { Result { try CommonPhrasesBridge.load() } }.value
      switch loaded {
      case .success(let list):
        phrases = list
        loadFailed = false
      case .failure:
        // 已经加载的列表保留不动；刷新失败只替换加载提示。
        if phrases == nil { loadFailed = true }
      }
    }
  }

  /// 在主线程之外执行一次写入，并显示它返回的已存储列表，或说明存储拒绝的原因。写入被拒绝时会重新加载，这样在别处被删除的短语（`common_phrases_not_found`）也会从列表中消失。
  private func run(success: String, _ operation: @escaping @Sendable () throws -> [CommonPhrasesBridge.Phrase]) {
    guard !busy else { return }
    busy = true
    Task {
      let result = await Task.detached(priority: .userInitiated) { Result { try operation() } }.value
      busy = false
      switch result {
      case .success(let list):
        phrases = list
        loadFailed = false
        ToastCenter.shared.show(success)
      case .failure(let error):
        ToastCenter.shared.show(CommonPhrasesBridge.message(for: error))
        reload()
      }
    }
  }
}

/// 添加常用语 弹窗：一个多行输入框，最多 1000 个字符，与存储允许的上限一致。
private struct CommonPhraseEditor: View {
  let save: (String) -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var text = ""
  @FocusState private var focused: Bool

  private static let limit = 1000
  private var trimmed: String { text.trimmingCharacters(in: .whitespacesAndNewlines) }
  /// 存储按 UTF-16 单元计数，与 Android 的 `validText` 一致。
  private var valid: Bool { !trimmed.isEmpty && text.utf16.count <= Self.limit }

  var body: some View {
    NavigationStack {
      Form {
        Section {
          TextField("输入常用语", text: $text, axis: .vertical)
            .lineLimit(3...6)
            .focused($focused)
            .accessibilityIdentifier("commonPhraseText")
        } footer: {
          Text(text.utf16.count > Self.limit ? "最多 1000 字，已超出 \(text.utf16.count - Self.limit) 字。" : "可以换行，最多 1000 字。")
        }
      }
      .navigationTitle("添加常用语")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() } }
        ToolbarItem(placement: .confirmationAction) {
          Button("添加") {
            save(text)
            dismiss()
          }
          .disabled(!valid)
          .accessibilityIdentifier("saveCommonPhrase")
        }
      }
      .onAppear { focused = true }
    }
    .tint(MetasequoiaTheme.accent)
  }
}
