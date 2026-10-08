import SwiftUI
import UIKit

/// 宿主 app 里的每个开关都使用 `MetasequoiaTheme.switchOn`，也就是季节强调色，让开关、按钮、链接和选中态共用一种颜色。这个名字早于季节配色，那时开关还是 iOS 系统绿；保留它是因为 app 根视图按名字应用它。在每个根视图应用一次，推入的页面和弹窗都会继承。
struct GreenSwitchToggleStyle: ToggleStyle {
  func makeBody(configuration: Configuration) -> some View {
    Toggle(configuration).toggleStyle(.switch).tint(MetasequoiaTheme.switchOn)
  }
}

/// The settings pages the 设置 tab opens, in the order and grouping of the mobile design. Phone and iPad read the same list, so a page added here shows up on both.
enum SettingsPage: String, CaseIterable, Identifiable {
  case skin, candidate, toolbar, input, ai, phrases, dictionary, layout, voice, handwriting, developer

  var id: Self { self }

  /// 根页面的各分组，对应 Android 的 `KeyboardFragment.buildRows`。移动端设计稿去掉了 键盘，但它仍留在根页面，因为布局、高度、反馈、候选栏 和 键盘工具栏 都在那里；后两个页面从 键盘 和搜索进入，不从根页面进入。
  static let groups: [[SettingsPage]] = [
    [.skin, .layout],
    [.input, .ai, .dictionary],
    [.voice, .handwriting],
    [.developer],
  ]

  var title: String {
    switch self {
    case .skin: return "皮肤"
    case .candidate: return "候选栏"
    case .toolbar: return "键盘工具栏"
    case .input: return "输入"
    case .ai: return "表达"
    case .phrases: return "常用语"
    case .dictionary: return "词库"
    case .layout: return "键盘"
    case .voice: return "语音输入"
    case .handwriting: return "手写输入"
    case .developer: return "开发者选项"
    }
  }

  var symbol: String {
    switch self {
    case .skin: return "paintpalette.fill"
    case .candidate: return "list.bullet.rectangle.fill"
    case .toolbar: return "menubar.rectangle"
    case .input: return "character.textbox"
    case .ai: return "character.bubble.fill"
    case .phrases: return "text.bubble.fill"
    case .dictionary: return "book.closed.fill"
    case .layout: return "keyboard.fill"
    case .voice: return "mic.fill"
    case .handwriting: return "pencil"
    case .developer: return "wrench.and.screwdriver.fill"
    }
  }

  /// 除标题外搜索还会匹配的词，这样搜某个设置项就能找到包含它的页面。
  var keywords: [String] {
    switch self {
    case .skin: return ["主题", "配色", "外观"]
    case .candidate: return ["候选", "字号", "字体"]
    case .toolbar: return ["工具栏", "按钮"]
    case .input: return ["方案", "拼音", "双拼", "五笔", "模糊音", "简繁", "辅助码", "翻译"]
    case .ai: return ["标点", "联想", "英文"]
    case .phrases: return ["短语", "快捷回复"]
    case .dictionary: return ["词库", "自造词", "学习"]
    case .layout: return ["键盘", "高度", "间距", "按键音", "振动"]
    case .voice: return ["语音", "识别"]
    case .handwriting: return ["手写"]
    case .developer: return ["日志", "诊断", "重置"]
    }
  }

  /// 手机上的各行沿用旧卡片面板的标识符，UI 测试和其他按 id 查找页面的地方仍能找到它。表达 行现在打开 表达 页，所以有自己的 id；`aiSettingsLink` 属于 键盘 页上的 AI 行。
  var linkIdentifier: String {
    switch self {
    case .skin: return "skinSettingsLink"
    case .candidate: return "candidateOptionsSettingsLink"
    case .toolbar: return "keyboardToolbarSettingsLink"
    case .input: return "inputSettingsLink"
    case .ai: return "expressionSettingsLink"
    case .phrases: return "commonPhrasesSettingsLink"
    case .dictionary: return "dictionarySettingsLink"
    case .layout: return "keyboardLayoutLink"
    case .voice: return "voiceSettingsLink"
    case .handwriting: return "handwritingSettingsLink"
    case .developer: return "developerOptionsLink"
    }
  }

  @ViewBuilder var destination: some View {
    switch self {
    case .skin: SkinSettingsView()
    case .candidate: CandidateOptionsSettingsView()
    case .toolbar: KeyboardToolbarSettingsView()
    case .input: InputSettingsView()
    case .ai: ExpressionSettingsView()
    case .phrases: CommonPhrasesSettingsView()
    case .dictionary: DictionarySettingsView()
    case .layout: KeyboardLayoutSettingsView()
    case .voice: VoiceSettingsView()
    case .handwriting: HandwritingSettingsView()
    case .developer: DeveloperOptionsView()
    }
  }

  /// 某个查询要显示的分组：查询为空时显示全部分组，否则每个标题或关键词包含它的页面各占一组。不在根页面上的页面（候选栏、键盘工具栏、常用语）也会被搜索，排在根页面之后。
  static func matching(_ query: String) -> [[SettingsPage]] {
    let query = query.trimmingCharacters(in: .whitespaces)
    guard !query.isEmpty else { return groups }
    let rootPages = Array(groups.joined())
    let ordered = rootPages + allCases.filter { !rootPages.contains($0) }
    let found = ordered.filter { page in
      page.title.localizedCaseInsensitiveContains(query) || page.keywords.contains { $0.localizedCaseInsensitiveContains(query) }
    }
    return found.isEmpty ? [] : [found]
  }
}

/// iOS 设计稿里的设置行标签：29pt 无填充图标框，图标用 sub 色（iPad 侧栏传入文字色），17pt 标题，右侧可选的值。箭头由 `NavigationLink` 提供。
struct SettingsNavLabel: View {
  let title: String
  let symbol: String
  var value: String? = nil
  var iconColor: Color = MetasequoiaTheme.sub

  var body: some View {
    HStack(spacing: 12) {
      Image(systemName: symbol).font(.system(size: 20)).foregroundStyle(iconColor)
        .frame(width: 29, height: 29).accessibilityHidden(true)
      Text(title).font(.system(size: 17)).foregroundStyle(.primary)
      Spacer(minLength: 8)
      if let value {
        Text(value).font(.system(size: 17)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
      }
    }
    .contentShape(Rectangle())
  }
}

/// 手机上 设置 分组卡片中的一行：图标框、标题、值和箭头，按下时的填充横跨整张卡片。分隔线只在标题下方，从 x=61 到卡片边缘前 20pt，卡片最后一行下方不画。
struct SettingsCardRow: View {
  let page: SettingsPage
  let value: String?
  let isLast: Bool
  @Environment(\.displayScale) private var displayScale
  @EnvironmentObject private var router: SettingsRouter

  var body: some View {
    // 通过 router 推入，设置 导航栈用 `navigationDestination(item:)` 呈现它，所以打开的页面保存在视图树之外，app 因新的 app 主题或季节重建内容时页面会恢复。
    Button { router.settingsPage = page } label: {
      HStack(spacing: 12) {
        Image(systemName: page.symbol).font(.system(size: 20)).foregroundStyle(MetasequoiaTheme.sub)
          .frame(width: 29, height: 29).accessibilityHidden(true)
        HStack(spacing: 8) {
          Text(page.title).font(.system(size: 17)).foregroundStyle(.primary)
          Spacer(minLength: 8)
          if let value {
            Text(value).font(.system(size: 17)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
          }
          Image(systemName: "chevron.right").font(.system(size: 14, weight: .semibold))
            .foregroundStyle(MetasequoiaTheme.sub.opacity(0.55))
            .padding(.trailing, 2)
            .accessibilityHidden(true)
        }
        .frame(minHeight: 52)
        .overlay(alignment: .bottom) {
          if !isLast {
            Rectangle().fill(MetasequoiaTheme.hair).frame(height: 1 / displayScale).accessibilityHidden(true)
          }
        }
      }
      .padding(.horizontal, 20)
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityIdentifier(page.linkIdentifier)
    .accessibilityValue(value ?? "")
  }
}

/// 设置 根页面的搜索框：大标题下内容里的 40pt 胶囊，随页面滚动，而不是放在导航栏里。
struct SettingsSearchPill: View {
  @Binding var query: String

  init(query: Binding<String>) {
    _query = query
  }

  var body: some View {
    HStack(spacing: 8) {
      Image(systemName: "magnifyingglass").font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub)
        .accessibilityHidden(true)
      TextField("搜索", text: $query, prompt: Text("搜索").foregroundStyle(MetasequoiaTheme.sub))
        .font(.system(size: 17))
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .submitLabel(.search)
        .accessibilityIdentifier("settingsSearchField")
        .accessibilityAddTraits(.isSearchField)
      if !query.isEmpty {
        Button { query = "" } label: {
          Image(systemName: "xmark.circle.fill").font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("清除搜索")
      }
    }
    .padding(.horizontal, 14)
    .frame(minHeight: 40)
    .background(MetasequoiaTheme.segBg, in: Capsule())
  }
}

/// 设置 顶部的卡片：app 标志、当前方案、两个设置步骤和 试用键盘 按钮。
///
/// 宿主 app 无法判断键盘是否已添加、是否已获得完全访问权限 —— iOS 对这两者都没有公开 API，而由键盘写入的标记在用户撤销权限后仍会保留 —— 所以每一步都显示中性的序号标记，而不是设计稿的 ✓ / ! 状态，两个 去开启 链接也始终显示。
struct KeyboardStatusCard: View {
  /// 试用键盘 的行为：把试用页推入手机的导航栈，或执行一个操作（iPad 在侧栏中选中试用页）。
  enum Tryout {
    case push
    case action(() -> Void)
  }

  let scheme: ChineseInputScheme
  let tryout: Tryout

  init(scheme: ChineseInputScheme, tryout: Tryout) {
    self.scheme = scheme
    self.tryout = tryout
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 14) {
      HStack(spacing: 12) {
        AppMarkDisc(diameter: 52, markSize: 30)
        VStack(alignment: .leading, spacing: 2) {
          Text("水杉输入法").font(.system(size: 17, weight: .semibold))
          Text(scheme.shortLabel).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
            .accessibilityIdentifier("keyboardStatusScheme")
        }
      }
      VStack(alignment: .leading, spacing: 8) {
        setupRow(1, "添加到键盘列表", identifier: "openKeyboardSettingsButton", hint: "设置 → 通用 → 键盘 → 键盘 → 添加新键盘")
        setupRow(2, "允许完全访问", identifier: "openFullAccessSettingsButton", hint: "云候选、云剪贴板和同步需要它，日常输入不联网")
      }
      tryoutButton.padding(.top, 4)
    }
    .padding(.vertical, 18)
    .padding(.horizontal, 20)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
  }

  private func setupRow(_ number: Int, _ label: String, identifier: String, hint: String) -> some View {
    HStack(spacing: 10) {
      Text("\(number)").font(.system(size: 12, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
        .frame(width: 20, height: 20).background(MetasequoiaTheme.accentSoft, in: Circle())
        .accessibilityHidden(true)
      // 下面的链接为 VoiceOver 提供标签，这样每一步只朗读一次。
      Text(label).font(.system(size: 14)).accessibilityHidden(true)
      Spacer(minLength: 8)
      Button {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
      } label: {
        Text("去开启").font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.accent)
      }
      .buttonStyle(.plain)
      .accessibilityLabel("去开启：\(label)")
      .accessibilityIdentifier(identifier)
      .accessibilityHint(hint)
    }
  }

  private var tryoutLabel: some View {
    HStack(spacing: 6) {
      Image(systemName: "keyboard").font(.system(size: 14)).accessibilityHidden(true)
      Text("试用键盘").font(.system(size: 15, weight: .semibold))
    }
    .foregroundStyle(MetasequoiaTheme.accent)
    .frame(maxWidth: .infinity)
    .frame(minHeight: 40)
    .background(MetasequoiaTheme.accentSoft, in: Capsule())
    .contentShape(Capsule())
  }

  @ViewBuilder private var tryoutButton: some View {
    switch tryout {
    case .push:
      NavigationLink(destination: KeyboardTryoutView(focusOnAppear: true)) { tryoutLabel }
        .buttonStyle(DimOnPressButtonStyle())
        .accessibilityIdentifier("keyboardTryoutLink")
    case .action(let action):
      Button(action: action) { tryoutLabel }
        .buttonStyle(DimOnPressButtonStyle())
        .accessibilityIdentifier("keyboardTryoutLink")
    }
  }
}

/// 填充胶囊按钮按下时淡到 70%，与设计稿的按钮一致。
private struct DimOnPressButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label.opacity(configuration.isPressed ? 0.7 : 1)
  }
}

/// Section header in the grouped lists: 13pt regular in the design's group-title grey, not the uppercase footnote UIKit draws by default.
struct SettingsGroupHeader: View {
  let title: String
  var body: some View {
    Text(title).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.groupTitle).textCase(nil)
  }
}
