import SwiftUI

@MainActor
final class ReplyKeyboardModel: ObservableObject {
  @Published var text = ""
  @Published var polish = false { didSet { if oldValue != polish { resetResults() } } }
  @Published var replies: [String] = []
  @Published var status = "粘贴 TA 的话，再选择回复方式"
  @Published var busy = false
  @Published var style = "高情商"
  private var operation: Task<Void, Never>?
  private var generation = UUID()
  private let communityReplies: () -> [CommunityResource]
  var insertResult: ((String) -> Bool)?

  init(communityReplies: @escaping () -> [CommunityResource] = { CommunityLibrary.replies }) {
    self.communityReplies = communityReplies
  }

  func setText(_ value: String) {
    resetResults()
    guard value.count <= 10_000 else { status = "每次最多粘贴一万字"; return }
    text = value
    status = value.isEmpty ? "剪贴板里没有文字" : "选择下方风格生成，内容仅在点击风格时发送"
  }
  func resetResults() {
    operation?.cancel(); operation = nil; generation = UUID()
    busy = false; replies = []; insertResult = nil
  }
  func invalidateContext() {
    resetResults()
    status = "输入位置已变化，请重新选择回复方式"
  }
  func generate(style: String, request: @escaping @MainActor (String, String) async throws -> String,
                insert: @escaping (String) -> Bool) {
    guard !busy else { return }
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
      status = "先点粘贴，放入 TA 的话"; return
    }
    if self.style != style { replies = [] }
    self.style = style
    let template = style.hasPrefix("community:") ? communityReplies().first { "community:\($0.id)" == style } : nil
    if style.hasPrefix("community:") && template == nil { status = "模板已移除，请重新选择"; return }
    busy = true; status = "正在生成 · \(template?.name ?? style)"
    let id = UUID(); generation = id
    let basePrompt = polish
      ? "请以\(style)的语气润色用户文字，保持原意，不编造事实或承诺。只输出一条简短自然的成稿，不加标题、解释或引号。"
      : "用户内容是对方发来的话，请代拟一条\(style)风格的回复。尊重对方且有边界，不编造事实、关系或承诺。只输出一条简短自然、可以直接发送的回复，不加标题、解释或引号。"
    let prompt = template.map { item in
      "\(polish ? "润色用户原文，保持原意。" : "用户内容是对方发来的话，请代拟回复。")\n\(item.content.prompt ?? "")\n只输出可直接使用的一条回复，不编造事实或承诺。"
    } ?? basePrompt
    let source = text
    operation = Task { @MainActor in
      do {
        let result = try await request(source, prompt)
        try Task.checkCancellation()
        guard generation == id else { return }
        guard !result.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, result.count <= 10_000 else {
          throw ServiceFailure(message: "回复为空或过长，请重试")
        }
        if !replies.contains(result) { replies.insert(result, at: 0); replies = Array(replies.prefix(3)) }
        insertResult = insert
        status = "点选回复插入输入框"
      } catch {
        guard generation == id, !Task.isCancelled else { return }
        status = error.localizedDescription
      }
      if generation == id { busy = false }
    }
  }
  func use(_ reply: String) {
    guard replies.contains(reply), insertResult?(reply) == true else {
      invalidateContext(); return
    }
    resetResults(); status = "已插入，请在聊天应用中确认发送"
  }
}

struct ReplyKeyboardView: View {
  @ObservedObject var model: ReplyKeyboardModel
  let paste: () -> Void
  let generate: (String) -> Void
  private let styles = ["😁 专属回复", "🥰 暖心关怀", "📣 捧场王", "😍 恋人", "🌪 幽默风趣", "👔 成熟稳重", "💬 土味情话", "🤩 高情商", "🙌 委婉拒绝"]
  private var skin: KeyboardTheme { KeyboardTheme.current }
  private var radius: CGFloat { CGFloat(skin.cornerRadius) }
  private var keyGap: CGFloat { CGFloat(KeyboardLayoutPreference.keySpacing) }
  private var rowGap: CGFloat { CGFloat(KeyboardLayoutPreference.rowSpacing) }
  private var keySurface: Color { Color(uiColor: skin.keyBackground) }

  var body: some View {
    VStack(spacing: rowGap) {
      HStack(spacing: keyGap) {
        Picker("操作", selection: $model.polish) {
          Text("帮你回").tag(false)
          Text("帮润色").tag(true)
        }.pickerStyle(.segmented).frame(maxWidth: 200).accessibilityIdentifier("replyMode")
        Spacer(minLength: 0)
        Menu {
          if CommunityLibrary.replies.isEmpty { Text("在 App 社区收藏并添加回复模板") }
          ForEach(CommunityLibrary.replies) { item in
            Button(item.name) { generate("community:\(item.id)") }
          }
        } label: { Image(systemName: "bookmark") }
          .accessibilityLabel("回复模板").accessibilityIdentifier("replyTemplates").disabled(model.busy)
      }.frame(height: 36)
      HStack(spacing: keyGap) {
        Button(action: paste) {
          Text(model.text.isEmpty ? "+ 粘贴 TA 的话帮你回" : model.text)
            .lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
        }.accessibilityIdentifier("replySource")
        Button("粘贴", action: paste).accessibilityIdentifier("replyPaste")
          .padding(.horizontal, 10).foregroundStyle(.white).background(Color.accentColor, in: RoundedRectangle(cornerRadius: radius))
      }.frame(height: 38).padding(.horizontal, 10)
        .background(keySurface, in: RoundedRectangle(cornerRadius: radius))
      HStack(alignment: .top, spacing: keyGap) {
        if model.replies.isEmpty {
          VStack(spacing: rowGap) {
            ForEach(0..<3) { row in
              HStack(spacing: keyGap) {
                ForEach(0..<3) { column in
                  let label = styles[row * 3 + column]
                  Button { generate(String(label.dropFirst(2))) } label: {
                    Text(label).font(.system(size: 14, weight: .medium)).minimumScaleFactor(0.7).lineLimit(1)
                      .frame(maxWidth: .infinity, maxHeight: .infinity)
                  }.disabled(model.busy).accessibilityIdentifier("replyStyle_\(row * 3 + column)")
                    .background(keySurface, in: RoundedRectangle(cornerRadius: radius))
                }
              }
            }
          }
        } else {
          ScrollView {
            VStack(spacing: keyGap) {
              ForEach(model.replies, id: \.self) { reply in
                Button { model.use(reply) } label: {
                  Text(reply).font(.system(size: 15)).multilineTextAlignment(.leading)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(10)
                }.accessibilityIdentifier("replyCandidate")
                  .background(keySurface, in: RoundedRectangle(cornerRadius: radius))
              }
            }
          }.accessibilityIdentifier("replyCandidates").disablingScrollEdgeEffects()
        }
        VStack(spacing: rowGap) {
          Button { if !model.text.isEmpty { model.setText(String(model.text.dropLast())) } } label: {
            Image(systemName: "delete.left")
          }.accessibilityLabel("删除源文字").accessibilityIdentifier("replyDelete")
          Button("清空") { model.setText("") }.accessibilityIdentifier("replyClear")
          if model.busy {
            Button("取消") { model.resetResults(); model.status = "已取消" }
          } else if model.replies.isEmpty {
            Button("生成") { generate(model.style) }.accessibilityIdentifier("replyGenerate")
          } else {
            Button("换一句") { generate(model.style) }.accessibilityIdentifier("replyRegenerate")
          }
        }.frame(width: 60)
          .buttonStyle(ReplyActionStyle(background: keySurface.opacity(0.7), radius: radius))
      }.frame(maxHeight: .infinity)
      HStack(spacing: 4) {
        if model.busy { ProgressView().scaleEffect(0.7) }
        Text(model.status).font(.system(size: 11)).lineLimit(1).accessibilityIdentifier("replyStatus")
        Spacer(minLength: 0)
        if !model.replies.isEmpty { Button("选风格") { model.resetResults() }.font(.caption) }
      }.frame(height: 18)
    }
    .padding(.horizontal, 6).padding(.vertical, 5)
    .foregroundStyle(Color(uiColor: skin.keyForeground))
    .tint(Color(uiColor: skin.accent))
    .background(Color(uiColor: skin.background))
    .buttonStyle(.plain)
  }
}

private struct ReplyActionStyle: ButtonStyle {
  let background: Color
  let radius: CGFloat
  func makeBody(configuration: Configuration) -> some View {
    configuration.label.font(.system(size: 14, weight: .medium))
      .frame(maxWidth: .infinity, maxHeight: .infinity)
      .background(background.opacity(configuration.isPressed ? 0.6 : 1), in: RoundedRectangle(cornerRadius: radius))
  }
}
