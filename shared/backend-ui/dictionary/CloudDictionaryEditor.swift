import SwiftUI

struct CloudDictionaryEditor: View {
  let kind: BackendAccountClient.DictionaryKind
  let isEditing: Bool
  let save: (BackendAccountClient.DictionaryValue) async throws -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var code: String
  @State private var word: String
  @State private var weight: String
  @State private var message: String?
  @State private var busy = false
  @State private var pending: Task<Void, Never>?
  init(kind: BackendAccountClient.DictionaryKind, value: BackendAccountClient.DictionaryValue?, save: @escaping (BackendAccountClient.DictionaryValue) async throws -> Void) {
    self.kind = kind; self.isEditing = value != nil; self.save = save
    _code = State(initialValue: value?.code ?? "")
    _word = State(initialValue: value?.word ?? "")
    _weight = State(initialValue: String(value?.weight ?? 100_000))
  }
  private var title: String { isEditing ? "编辑云端词条" : "添加云端词条" }
  private var fields: some View {
    Form {
      Section(kind.title) {
        TextField("词条内容", text: $word)
        TextField("完整编码", text: $code).backendCodeInput()
        TextField("权重", text: $weight).backendNumberInput()
      }
      Text("点击上传将把此词条保存到当前账号。版本冲突或重复词条时需返回刷新，再重新确认。")
        .font(.footnote).foregroundStyle(.secondary)
      if let message { Text(message).foregroundStyle(.red) }
      if busy { ProgressView() }
    }.disabled(busy)
  }
  private var cancelButton: some View { Button("取消") { pending?.cancel(); dismiss() } }
  private var uploadButton: some View {
    Button("上传") {
      guard let number = Int64(weight), number >= 0, !word.isEmpty else { message = "请填写词条和有效的非负权重。"; return }
      busy = true; message = nil
      pending = Task {
        defer { busy = false }
        do { try await save(.init(code: code, word: word, weight: number)); try Task.checkCancellation(); dismiss() }
        catch is CancellationError { }
        catch { message = error.localizedDescription }
      }
    }.disabled(busy)
  }
  var body: some View {
    Group {
      #if os(macOS)
      VStack(alignment: .leading, spacing: 16) {
        Text(title).font(.headline)
        fields
        HStack { cancelButton; Spacer(); uploadButton }
      }.padding(20).frame(width: 440, height: 320)
      #else
      NavigationStack {
        fields.fittedSheet().navigationTitle(title).toolbar {
          ToolbarItem(placement: .cancellationAction) { cancelButton }
          ToolbarItem(placement: .confirmationAction) { uploadButton }
        }
      }
      #endif
    }
    .interactiveDismissDisabled(busy)
    .onDisappear { pending?.cancel() }
  }
}

