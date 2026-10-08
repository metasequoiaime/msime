import SwiftUI

@MainActor
struct AISkinGenerationView: View {
  let useDesign: (CustomKeyboardSkin) -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var proposals: [AISkinProposal] = []
  @State private var saved: [UUID: UUID] = [:]
  @State private var publishing: SavedKeyboardSkin?
  @State private var login = false
  @State private var busy = false
  @State private var completed = 0
  @State private var message: String?
  @State private var request: Task<Void, Never>?
  var body: some View {
    NavigationView {
      ScrollView {
        VStack(alignment: .leading, spacing: 18) {
          VStack(alignment: .leading, spacing: 8) {
            Text("下一张，会是什么风格？").font(.title2.bold())
            Text("一次抽出三张原创皮肤，遇到喜欢的就留下。")
              .font(.subheadline).foregroundStyle(.secondary)
          }
          if proposals.isEmpty || busy { mysteryCards }
          Button { generate() } label: {
            Label(proposals.isEmpty ? "抽三张皮肤" : "再抽三张", systemImage: "sparkles")
              .font(.headline).frame(maxWidth: .infinity).padding(.vertical, 8)
          }.buttonStyle(.borderedProminent).disabled(busy)
            .accessibilityIdentifier("generateAISkins")
          Text("AI 随机搭配插画、键帽造型与材质。抽到的皮肤可以继续编辑、保存或分享。")
            .font(.caption).foregroundStyle(.secondary)
          if busy { HStack { ProgressView(); Text("主题插画已完成 \(completed)/3，可能需要几分钟…"); Button("取消") { request?.cancel() } } }
          if let message { Text(message).font(.callout).foregroundStyle(.secondary) }
          ForEach(proposals) { proposal in
            VStack(alignment: .leading, spacing: 12) {
              Text(proposal.name).font(.headline)
              Text(proposal.description).font(.caption).foregroundStyle(.secondary)
              CommunityDesignPreview(design: proposal.design).frame(height: 210)
              HStack {
                Button("使用并继续编辑") { useDesign(proposal.design); dismiss() }
                Button(saved[proposal.id] == nil ? "保存" : "已保存") { _ = save(proposal) }
                  .disabled(saved[proposal.id] != nil).accessibilityIdentifier("saveAISkin_" + proposal.name)
                Button("发布到社区") { if let value = save(proposal) { publishing = value } }.accessibilityIdentifier("publishAISkin_" + proposal.name)
              }.buttonStyle(.bordered)
            }.padding().background(Color(uiColor:.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius:16))
          }
        }.padding()
      }.navigationTitle("AI 皮肤抽卡").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement:.cancellationAction) { Button("完成") { dismiss() } } }
    }
    .onDisappear { request?.cancel() }
    .sheet(item: $publishing) { item in CommunityPublishView(onPublished: { publishing = nil }, selectedSkinID: item.id) }
    .sheet(isPresented: $login) { AccountLoginSheet() }
  }
  private var mysteryCards: some View {
    HStack(spacing: 12) {
      ForEach(0..<3) { index in
        VStack(spacing: 12) {
          Text("MSIME").font(.caption2.weight(.semibold)).tracking(2)
          Spacer(minLength: 0)
          Image(systemName: ["leaf.fill", "moon.stars.fill", "sparkles"][index])
            .font(.system(size: 30, weight: .light))
          Spacer(minLength: 0)
          Text("等待揭晓").font(.caption)
        }.foregroundStyle(.white.opacity(0.95)).padding(14)
          .frame(maxWidth: .infinity).frame(height: 150)
          // 三张卡片原先中间那张是紫的、两侧是绿的,而它们代表的是同一件还没发生的事。差别交给旋转和错位,颜色统一走品牌绿。
          .background(LinearGradient(colors: [Color(red: 0.29, green: 0.52, blue: 0.44), Color(red: 0.10, green: 0.29, blue: 0.24)],
                                     startPoint: .topLeading, endPoint: .bottomTrailing), in: RoundedRectangle(cornerRadius: 16))
          .overlay(RoundedRectangle(cornerRadius: 12).stroke(.white.opacity(0.25), lineWidth: 1).padding(5))
          .rotationEffect(.degrees(Double(index - 1) * 5))
          .offset(y: index == 1 ? -5 : 5)
      }
    }.padding(.vertical, 12).accessibilityHidden(true)
  }
  private func generate() {
    guard !busy else { return }; busy = true; message = nil; completed = 0
    let description = AISkinService.drawPrompt()
    request = Task {
      defer { busy = false; request = nil }
      do {
        let values: [AISkinProposal]
        #if DEBUG && targetEnvironment(simulator)
        if ProcessInfo.processInfo.arguments.contains("-aiSkinPreview") {
          try await Task.sleep(nanoseconds: ProcessInfo.processInfo.arguments.contains("-skinGenerationSlowFixture") ? 3_600_000_000_000 : 100_000_000)
          values = CustomKeyboardSkin.templates.prefix(3).enumerated().map {
            AISkinProposal(name: "AI 测试 \($0.offset + 1)", description: "仅用于界面自动化的合成设计", design: $0.element.1)
          }
        } else { values = try await AISkinService.generate(description) { completed = $0 } }
        #else
        values = try await AISkinService.generate(description) { completed = $0 }
        #endif
        try Task.checkCancellation(); proposals = values
      } catch is CancellationError { }
      catch let failure as BackendAccountClient.Failure where failure.status == 401 {
        message = "请先登录，再来抽取皮肤。"; login = true
      } catch { if !Task.isCancelled { message = error.localizedDescription } }
    }
  }
  private func save(_ proposal: AISkinProposal) -> SavedKeyboardSkin? {
    var library = CustomSkinLibrary.designs
    if let id = saved[proposal.id], let value = library.first(where: { $0.id == id }) { return value }
    guard library.count < 12 else { message = "最多保存 12 套皮肤，请先在「我的皮肤」删除不需要的设计。"; return nil }
    var name = proposal.name
    var suffix = 2
    while library.contains(where: { $0.name == name }) {
      name = String(proposal.name.prefix(26)) + " \(suffix)"; suffix += 1
    }
    let value = SavedKeyboardSkin(name:name, design:proposal.design)
    library.append(value)
    guard CustomSkinLibrary.save(library) else { message = "保存失败，请检查设备空间后重试。"; return nil }
    saved[proposal.id] = value.id; message = "已保存到「我的皮肤」。"
    return value
  }
}
