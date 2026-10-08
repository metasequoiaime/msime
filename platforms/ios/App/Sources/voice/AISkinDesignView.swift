import SwiftUI
import UIKit

/// AI 设计皮肤页的状态。它是共享的、不归页面所有，因为一次生成要几分钟，用户离开页面再回来时它必须还在：任务继续跑，回来时结果已经在等着。只有「取消」能停下它。
@MainActor
final class AISkinDesignModel: ObservableObject {
  static let shared = AISkinDesignModel()

  /// 服务端接受去掉首尾空白后 1–500 个字符。
  static let maximumPromptLength = 500

  @Published var prompt = ""
  @Published var nineKey = InputSchemePreference.scheme == .nineKey
  @Published private(set) var proposals: [AISkinProposal] = []
  @Published var chosen = 0
  @Published private(set) var busy = false
  @Published private(set) var completed = 0
  @Published var message: String?
  @Published var loginNeeded = false

  private var request: Task<Void, Never>?
  private var generation = UUID()
  /// 方案 id 到它在我的皮肤里保存时所用 id 的映射，这样重试「使用此皮肤」不会再存一份。
  private var saved: [UUID: UUID] = [:]

  var selected: AISkinProposal? {
    proposals.isEmpty ? nil : proposals[min(max(chosen, 0), proposals.count - 1)]
  }

  var trimmedPrompt: String { prompt.trimmingCharacters(in: .whitespacesAndNewlines) }

  private var fixture: Bool {
    #if DEBUG && targetEnvironment(simulator)
    ProcessInfo.processInfo.arguments.contains("-aiSkinPreview")
    #else
    false
    #endif
  }

  func generate() {
    let text = trimmedPrompt
    guard !busy, (1...Self.maximumPromptLength).contains(text.count) else { return }
    busy = true; completed = 0; message = nil
    let id = UUID(); generation = id
    request = Task {
      defer { if generation == id { busy = false; request = nil } }
      do {
        let values: [AISkinProposal]
        if fixture {
          try await Task.sleep(nanoseconds: ProcessInfo.processInfo.arguments.contains("-skinGenerationSlowFixture") ? 3_600_000_000_000 : 100_000_000)
          values = CustomKeyboardSkin.templates.prefix(3).enumerated().map {
            AISkinProposal(name: "AI 测试 \($0.offset + 1)", description: "仅用于界面自动化的合成设计", design: $0.element.1)
          }
        } else {
          guard try await BackendAccountSession.shared.user() != nil else {
            if generation == id { loginNeeded = true }
            return
          }
          values = try await AISkinService.generate(text) { count in
            if self.generation == id { self.completed = count }
          }
        }
        try Task.checkCancellation()
        guard generation == id else { return }
        proposals = values; chosen = 0; saved.removeAll()
      } catch is CancellationError {
      } catch let failure as BackendAccountClient.Failure where failure.status == 401 {
        guard generation == id else { return }
        message = "请先登录，再来设计皮肤。"; loginNeeded = true
      } catch {
        // 429 和 503 直接用后端客户端自带的文案（操作过于频繁… / 此服务暂不可用…）。
        if generation == id && !Task.isCancelled { message = error.localizedDescription }
      }
    }
  }

  func cancel() {
    generation = UUID()
    request?.cancel(); request = nil
    busy = false; completed = 0
  }

  /// 把选中的方案存进我的皮肤并把键盘切过去，与皮肤编辑器的「使用皮肤」做法一致。返回保存时用的名字；失败时返回 nil 并设置 `message`。
  func useSelected() -> String? {
    guard !busy, let proposal = selected else { return nil }
    let design = proposal.design.normalized
    var library = CustomSkinLibrary.designs
    let skin: SavedKeyboardSkin
    if let id = saved[proposal.id], let existing = library.first(where: { $0.id == id }) {
      skin = existing
    } else {
      guard library.count < 12 else {
        message = "最多保存 12 套皮肤，请先在「我的皮肤」删除不需要的设计。"
        return nil
      }
      var name = proposal.name
      var suffix = 2
      while library.contains(where: { $0.name == name }) {
        name = String(proposal.name.prefix(26)) + " \(suffix)"; suffix += 1
      }
      let value = SavedKeyboardSkin(name: name, design: design)
      library.append(value)
      guard CustomSkinLibrary.save(library) else {
        message = "保存失败，请检查设备空间后重试。"
        return nil
      }
      saved[proposal.id] = value.id
      skin = value
    }
    CustomKeyboardSkinStore.save(design)
    guard GlobalThemePreference.apply(design) else {
      message = "已保存到「我的皮肤」，但没能切换到这套皮肤，请稍后重试。"
      return nil
    }
    // 换装达人按保存时的 id 计入这次设计，与 Android 的 `AiSkinPage` 一致。
    TypingStatisticsExtras.recordSkin(skin.id.uuidString)
    // 这之后页面会关闭；下次进来从空白设计开始。
    proposals = []; chosen = 0; saved.removeAll(); prompt = ""; message = nil
    return skin.name
  }
}

/// AI 设计皮肤：描述一种感觉，让 AI 设计三套键盘皮肤，在实时键盘上预览后选一套使用。从皮肤页的虚线图块推入。
struct AISkinDesignView: View {
  @StateObject private var model = AISkinDesignModel.shared
  @State private var currentTheme: KeyboardTheme?
  @FocusState private var editing: Bool
  @Environment(\.dismiss) private var dismiss
  @Environment(\.colorScheme) private var colorScheme

  private static let suggestions = ["秋天的银杏", "深夜霓虹", "宋代青瓷", "樱花与和纸", "雨后竹林", "复古终端"]

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 24) {
        previewCard
        describeGroup
        if let message = model.message {
          Text(message)
            .font(.system(size: 13))
            .foregroundStyle(MetasequoiaTheme.sub)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 20)
            .accessibilityIdentifier("aiSkinDesignMessage")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 8)
      .padding(.bottom, 16)
    }
    .scrollDismissesKeyboard(.interactively)
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("AI 设计皮肤")
    .navigationBarTitleDisplayMode(.inline)
    .safeAreaInset(edge: .bottom, spacing: 0) { actionBar }
    .onAppear {
      currentTheme = KeyboardTheme.resolve(document: MetasequoiaInputSessionBridge.loadSharedPreferences())
    }
    .onChange(of: model.prompt) { _, value in
      if value.count > AISkinDesignModel.maximumPromptLength {
        model.prompt = String(value.prefix(AISkinDesignModel.maximumPromptLength))
      }
    }
    .sheet(isPresented: $model.loginNeeded) { AccountLoginSheet() }
  }

  // MARK: - 预览卡片

  /// 选中的方案；还没有方案时是键盘当前的皮肤。
  private var previewTheme: KeyboardTheme {
    if let proposal = model.selected { return .designed(proposal.design) }
    return currentTheme ?? .system
  }

  /// 固定了明暗模式的皮肤按该模式预览；其他皮肤跟随页面。
  private var previewScheme: ColorScheme {
    switch previewTheme.appearance {
    case .dark: .dark
    case .light: .light
    default: colorScheme
    }
  }

  private var subtitle: String {
    if model.busy {
      return model.completed > 0 ? "正在绘制背景插画 \(model.completed)/3…" : "正在根据描述生成…"
    }
    // iOS 皮肤不带音效和动画包，所以这里承诺的只有配色、按键和背景图。
    return model.selected?.description ?? "写下描述，AI 生成配色、键帽和背景插画"
  }

  private var previewCard: some View {
    VStack(alignment: .leading, spacing: 14) {
      HStack(alignment: .center, spacing: 10) {
        VStack(alignment: .leading, spacing: 2) {
          Text(model.selected?.name ?? "未命名皮肤")
            .font(.system(size: 17, weight: .bold))
            .lineLimit(1)
            .accessibilityIdentifier("aiSkinDesignTitle")
          Text(subtitle)
            .font(.system(size: 13))
            .foregroundStyle(MetasequoiaTheme.sub)
            .lineLimit(2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        DesignSegmentedControl(items: [(title: "26 键", value: false), (title: "9 键", value: true)],
                               selection: $model.nineKey, style: .capsule, identifierPrefix: "aiSkinDesignLayout")
          .frame(width: 112)
      }
      KeyboardSkinPreview(skin: previewTheme, nineKey: model.nineKey)
        .id(model.selected?.id)
        .environment(\.colorScheme, previewScheme)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .opacity(model.busy ? 0.45 : 1)
        .overlay { if model.busy { busyPill } }
        .animation(.easeInOut(duration: 0.3), value: model.busy)
      swatches
      if model.proposals.count > 1 {
        DesignSegmentedControl(items: model.proposals.enumerated().map { (title: $0.element.name, value: $0.offset) },
                               selection: $model.chosen, style: .capsule, identifierPrefix: "aiSkinDesignProposal")
          .disabled(model.busy)
      }
    }
    .padding(18)
    .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
  }

  private var swatches: some View {
    let theme = previewTheme
    let colors = [theme.background, theme.keyBackground, theme.functionKeyBackground, theme.keyForeground, theme.actionBackground]
    return HStack(spacing: 8) {
      Text("配色").font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
      HStack(spacing: 6) {
        ForEach(colors.indices, id: \.self) { index in
          Circle()
            .fill(Color(uiColor: colors[index]))
            .overlay(Circle().strokeBorder(Color.black.opacity(0.18), lineWidth: 0.5))
            .frame(width: 22, height: 22)
        }
      }
      .environment(\.colorScheme, previewScheme)
      .accessibilityHidden(true)
      Spacer(minLength: 0)
    }
  }

  private var busyPill: some View {
    HStack(spacing: 10) {
      ProgressView().tint(MetasequoiaTheme.accent)
      Text("正在设计…").font(.system(size: 14))
      Button("取消") { model.cancel() }
        .font(.system(size: 14, weight: .semibold))
        .foregroundStyle(MetasequoiaTheme.accent)
        .accessibilityIdentifier("cancelAISkinDesign")
    }
    .padding(.horizontal, 16)
    .frame(height: 40)
    .background(MetasequoiaTheme.surface, in: Capsule())
    .shadow(color: .black.opacity(0.15), radius: 8, y: 4)
  }

  // MARK: - 描述

  private var describeGroup: some View {
    VStack(alignment: .leading, spacing: 7) {
      Text("描述").font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.groupTitle)
        .padding(.horizontal, 20)
        .accessibilityAddTraits(.isHeader)
      VStack(alignment: .leading, spacing: 0) {
        ZStack(alignment: .topLeading) {
          if model.prompt.isEmpty {
            Text("想要什么感觉？比如：秋天傍晚的水杉林")
              .font(.system(size: 16))
              .foregroundStyle(MetasequoiaTheme.sub)
              .padding(.horizontal, 14)
              .padding(.top, 12)
              .allowsHitTesting(false)
              .accessibilityHidden(true)
          }
          TextEditor(text: $model.prompt)
            .font(.system(size: 16))
            .scrollContentBackground(.hidden)
            .focused($editing)
            .frame(minHeight: 96)
            // `TextEditor` 的文字水平缩进约 5pt、垂直约 8pt；这里让它和占位文字对齐。
            .padding(.horizontal, 9)
            .padding(.top, 4)
            .accessibilityLabel("描述")
            .accessibilityIdentifier("aiSkinDesignPrompt")
        }
        .disabled(model.busy)
        ScrollView(.horizontal) {
          HStack(spacing: 6) {
            ForEach(Self.suggestions, id: \.self) { suggestion in chip(suggestion) }
          }
          .padding(.horizontal, 14)
          .padding(.top, 4)
          .padding(.bottom, 12)
        }
        .scrollIndicators(.hidden)
      }
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
  }

  private func chip(_ suggestion: String) -> some View {
    let on = model.trimmedPrompt == suggestion
    return Button {
      model.prompt = suggestion
    } label: {
      Text(suggestion)
        .font(.system(size: 13, weight: on ? .semibold : .regular))
        .foregroundStyle(on ? MetasequoiaTheme.onAccent : Color.primary)
        .lineLimit(1)
        .padding(.horizontal, 11)
        .frame(height: 28)
        .background(on ? MetasequoiaTheme.accent : MetasequoiaTheme.segBg, in: Capsule())
    }
    .buttonStyle(.plain)
    .disabled(model.busy)
    .accessibilityLabel("建议描述 \(suggestion)")
    .accessibilityAddTraits(on ? .isSelected : [])
  }

  // MARK: - 底栏

  private var actionBar: some View {
    let hasPrompt = !model.trimmedPrompt.isEmpty
    return VStack(spacing: 0) {
      DesignDivider(leading: 0)
      HStack(spacing: 10) {
        if model.proposals.isEmpty {
          actionButton("✦ 生成皮肤", fill: hasPrompt ? MetasequoiaTheme.accent : MetasequoiaTheme.segBg,
                       text: hasPrompt ? MetasequoiaTheme.onAccent : MetasequoiaTheme.sub, identifier: "generateAISkinDesign") {
            generate()
          }
          .disabled(!hasPrompt || model.busy)
          .opacity(model.busy ? 0.45 : 1)
        } else {
          actionButton("✦ 重新生成", fill: MetasequoiaTheme.accentSoft, text: MetasequoiaTheme.accent, identifier: "regenerateAISkinDesign") {
            generate()
          }
          .disabled(!hasPrompt || model.busy)
          .opacity(!hasPrompt || model.busy ? 0.45 : 1)
          actionButton("使用此皮肤", fill: MetasequoiaTheme.accent, text: MetasequoiaTheme.onAccent, identifier: "useAISkinDesign") {
            use()
          }
          .disabled(model.busy)
          .opacity(model.busy ? 0.45 : 1)
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 10)
      .padding(.bottom, 10)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea(edges: .bottom))
  }

  private func actionButton(_ title: String, fill: Color, text: Color, identifier: String,
                            action: @escaping () -> Void) -> some View {
    Button(action: action) {
      Text(title)
        .font(.system(size: 17, weight: .semibold))
        .foregroundStyle(text)
        .lineLimit(1).minimumScaleFactor(0.8)
        .frame(maxWidth: .infinity)
        .frame(height: 48)
        .background(fill, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .contentShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
    .buttonStyle(AISkinDesignButtonStyle())
    .accessibilityIdentifier(identifier)
  }

  private func generate() {
    editing = false
    model.generate()
  }

  private func use() {
    guard let name = model.useSelected() else { return }
    ToastCenter.shared.show("已使用「\(name)」")
    dismiss()
  }
}

/// 底栏的按压反馈，按下时不透明度 80%。禁用状态由调用方自己处理：描述为空时「生成皮肤」用它自己的颜色变灰，任务进行中则整条底栏变淡。
private struct AISkinDesignButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label.opacity(configuration.isPressed ? 0.8 : 1)
  }
}
