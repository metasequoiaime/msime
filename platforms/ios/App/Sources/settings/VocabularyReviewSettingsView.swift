import SwiftUI
import UniformTypeIdentifiers

/// 背单词：复习会话，作为设置应用里的一个页面。
///
/// 这里不做任何排期。间隔、难易系数、一次遗忘的代价和队列的顺序，都经由 `VocabularyReviewStore` 从 `client-services::vocabulary` 返回；这个视图只显示卡片、报告按下了哪个按钮，并画出交给它的计数。
struct VocabularyReviewSettingsView: View {
  @Environment(\.horizontalSizeClass) private var widthClass
  @State private var status = VocabularyReviewStatus()
  @State private var loaded = false
  @State private var revealed = false
  @State private var busy = false
  @State private var message = ""
  @State private var confirmsReset = false
  @State private var picking = false
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()

  private let store = VocabularyReviewStore()
  /// 卡片下面的「朗读」，页面消失时停下正在读的。
  @State private var speaker = ReadAloudSpeaker()

  /// An iPad shows the card larger; a phone keeps it inside the width it has.
  private var wide: Bool { widthClass == .regular }

  var body: some View {
    Form {
      Section {
        HStack(spacing: wide ? 40 : 24) {
          metric("今日待复习", status.due)
          metric("已完成", status.answeredToday)
          metric("未开始", status.remaining)
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("今日待复习 \(status.due)，已完成 \(status.answeredToday)")
      } footer: {
        Text("复习只在这里进行，不会改变打字时的候选或组词。进度保存在本机，不会上传。")
      }
      .designRow()

      Section {
        Picker("词书", selection: wordbookBinding) {
          Text("未选择").tag("")
          ForEach(status.wordbooks) { book in
            Text(book.pack ? "\(book.name)（插件，\(book.total) 词）" : "\(book.name)（\(book.total) 词）").tag(book.id)
          }
        }
        .accessibilityIdentifier("vocabularyWordbookPicker")
        Stepper(
          "每日新词 \(status.newPerDay)",
          value: newPerDayBinding, in: 0...200, step: 5
        )
        .accessibilityIdentifier("vocabularyNewPerDayStepper")
        Button("导入词表（CSV / TXT）") { picking = true }
          .accessibilityIdentifier("vocabularyImportButton")
        if let book = status.wordbooks.first(where: { $0.id == status.wordbook }), !book.builtin, !book.pack {
          Button("删除这个词表", role: .destructive) { act { try store.removeWordbook(book.id) } }
            .accessibilityIdentifier("vocabularyRemoveButton")
        }
      } header: {
        DesignSectionHeader(subject: "词书", info: "每行一个词，用逗号或制表符分隔：单词,音标,释义 或 单词,释义。释义里有逗号时用英文引号括起来。") { SettingsGroupHeader(title: "词书") }
      }
      .designRow()

      Section {
        if status.needsWordbook {
          Text("先选一本词书。").foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .center)
        } else if let card = status.current {
          cardFace(card)
          // 词头是英文时卡片下面有「朗读」；它不翻开卡片，也不作答。
          if let spoken = ReadAloudText.text(candidate: card.word) {
            Button {
              if let notice = speaker.speak(spoken) { message = notice }
            } label: {
              Label("朗读", systemImage: "speaker.wave.2")
            }
            .buttonStyle(.borderless)
            .frame(maxWidth: .infinity)
            .accessibilityLabel("朗读 \(card.word)")
            .accessibilityIdentifier("vocabularyReadAloudButton")
          }
          HStack(spacing: 12) {
            Button("不认识") { answer(known: false) }
              .buttonStyle(.bordered)
              .frame(maxWidth: .infinity)
              .accessibilityIdentifier("vocabularyUnknownButton")
            Button("认识") { answer(known: true) }
              .buttonStyle(.borderedProminent)
              .frame(maxWidth: .infinity)
              .accessibilityIdentifier("vocabularyKnownButton")
          }
          .disabled(busy)
        } else {
          Text("今天的复习已经完成。").foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .center)
        }
      } footer: {
        Text("答「不认识」的词会在本次复习里再次出现；答「认识」的词按间隔安排到以后的某一天。")
      }
      .designRow()

      Section {
        Button("清空复习进度", role: .destructive) { confirmsReset = true }
          .accessibilityIdentifier("vocabularyResetButton")
      } footer: {
        if !message.isEmpty { Text(message).foregroundStyle(.red) }
      }
      .designRow()
    }
    .designPage()
    .navigationTitle("背单词")
    .navigationBarTitleDisplayMode(.inline)
    .task { request.appear(); if !loaded { reload() } }
    .alert("清空全部复习进度？", isPresented: $confirmsReset) {
      Button("取消", role: .cancel) {}
      Button("清空", role: .destructive) { act { try store.reset() } }
    } message: {
      Text("已经学过的词会从头开始，词表本身会保留。此操作无法撤销。")
    }
    .fileImporter(
      isPresented: $picking,
      allowedContentTypes: [.commaSeparatedText, .plainText, .text],
      allowsMultipleSelection: false
    ) { result in
      guard case .success(let urls) = result, let url = urls.first else { return }
      importWordbook(from: url)
    }
    .onDisappear {
      ViewRequestTask.disappear(request: $request, pending: $pending)
      busy = false
      speaker.stop()
    }
  }

  private func metric(_ title: String, _ value: Int) -> some View {
    VStack(alignment: .leading, spacing: 2) {
      Text("\(value)").font(.system(size: wide ? 34 : 28, weight: .semibold)).monospacedDigit()
        .foregroundStyle(MetasequoiaTheme.accent)
      Text(title).font(.caption).foregroundStyle(.secondary)
    }
  }

  private func cardFace(_ card: VocabularyCard) -> some View {
    Button {
      revealed = true
    } label: {
      VStack(spacing: 10) {
        Text(card.word)
          .font(.system(size: wide ? 40 : 32, weight: .semibold))
          .multilineTextAlignment(.center)
        if !card.phonetic.isEmpty {
          Text(card.phonetic).font(.subheadline).foregroundStyle(.secondary)
        }
        if revealed {
          Text(card.meaning).font(.body).multilineTextAlignment(.center)
        } else {
          Text("点击查看释义").font(.caption).foregroundStyle(.secondary)
        }
      }
      .frame(maxWidth: .infinity, minHeight: wide ? 200 : 160)
      .padding(.vertical, 12)
    }
    .buttonStyle(.plain)
    .accessibilityIdentifier("vocabularyCard")
    .accessibilityLabel(revealed ? "\(card.word)，\(card.meaning)" : "显示 \(card.word) 的释义")
  }

  private var wordbookBinding: Binding<String> {
    Binding(
      get: { status.wordbook },
      set: { id in
        act {
          try store.setSettings(
            wordbook: id, newPerDay: status.newPerDay, sessionLimit: status.sessionLimit)
        }
      })
  }

  private var newPerDayBinding: Binding<Int> {
    Binding(
      get: { status.newPerDay },
      set: { value in
        act {
          try store.setSettings(
            wordbook: status.wordbook, newPerDay: value, sessionLimit: status.sessionLimit)
        }
      })
  }

  private func answer(known: Bool) {
    guard let card = status.current else { return }
    // 词书与卡片取自同一显示快照，异步作答不能重新读取可能已改变的选择。
    let wordbook = status.wordbook
    // 新卡从隐藏释义开始。
    revealed = false
    act { try store.answer(wordbook: wordbook, word: card.word, known: known) }
  }

  private func reload() {
    loaded = true
    act { try store.load() }
  }

  private func importWordbook(from url: URL) {
    guard request.isVisible, !busy else { return }
    let name = String(url.deletingPathExtension().lastPathComponent.prefix(64))
    let store = store
    ViewRequestTask.run(request: $request, busy: $busy, pending: $pending,
                        prepare: { message = "" }) { id in
      try request.checkCurrent(id)
      let outcome = await Task.detached(priority: .userInitiated) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        return Result<VocabularyReviewStatus, Error> {
          guard let data = try? VocabularyReviewStore.readWordbookData(from: url) else {
            throw VocabularyReviewStore.Failure.unreadableWordbook
          }
          // Windows 词表也可能使用带 BOM 的 UTF-16 或 GB18030；不能只尝试 UTF-8。
          let text = String(data: data, encoding: .utf8)
            ?? String(data: data, encoding: .utf16)
            ?? String(decoding: data, as: UTF8.self)
          return try store.importWordbook(name: name.isEmpty ? "导入的词表" : name, text: text)
        }
      }.value
      try request.checkCurrent(id)
      switch outcome {
      case .success(let next): status = next
      case .failure(let error):
        message = (error as? VocabularyReviewStore.Failure)?.errorDescription
          ?? VocabularyReviewStore.Failure.unavailable.errorDescription ?? ""
      }
    }
  }

  /// One in-flight operation, and the whole status replaces the old one.
  ///
  /// Two taps on 认识 in quick succession would otherwise both read the same card and the second
  /// would schedule from a state the first had already replaced.
  private func act(_ operation: @escaping () throws -> VocabularyReviewStatus) {
    ViewRequestTask.run(request: $request, busy: $busy, pending: $pending,
                        prepare: { message = "" }) { id in
      try request.checkCurrent(id)
      // 共享入口会加文件锁并可能读取很大的词书，不能在主线程执行。
      let outcome = await Task.detached { Result { try operation() } }.value
      try request.checkCurrent(id)
      switch outcome {
      case .success(let next): status = next
      case .failure(let error):
        message = (error as? VocabularyReviewStore.Failure)?.errorDescription
          ?? VocabularyReviewStore.Failure.unavailable.errorDescription ?? ""
      }
    }
  }
}
