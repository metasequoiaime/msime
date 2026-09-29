import SwiftUI
import UIKit

/// Every switch in the host app is the iOS system green (#34C759), while buttons, links and selection keep the brand accent. Applied once at each root so pushed pages and sheets inherit it.
struct GreenSwitchToggleStyle: ToggleStyle {
  func makeBody(configuration: Configuration) -> some View {
    Toggle(configuration).toggleStyle(.switch).tint(MetasequoiaTheme.switchOn)
  }
}

/// The settings pages the 设置 tab opens, in the order and grouping of the mobile design. Phone and iPad read the same list, so a page added here shows up on both.
enum SettingsPage: String, CaseIterable, Identifiable {
  case skin, candidate, toolbar, input, ai, dictionary, layout, voice, handwriting, developer

  var id: Self { self }

  static let groups: [[SettingsPage]] = [
    [.skin, .candidate, .toolbar],
    [.input, .ai, .dictionary],
    [.layout, .voice, .handwriting],
    [.developer],
  ]

  var title: String {
    switch self {
    case .skin: return "主题"
    case .candidate: return "候选栏"
    case .toolbar: return "键盘工具栏"
    case .input: return "输入"
    case .ai: return "表达"
    case .dictionary: return "词库"
    case .layout: return "键盘"
    case .voice: return "语音输入"
    case .handwriting: return "手写输入"
    case .developer: return "开发者选项"
    }
  }

  var symbol: String {
    switch self {
    case .skin: return "paintpalette"
    case .candidate: return "list.bullet.rectangle"
    case .toolbar: return "menubar.rectangle"
    case .input: return "character.cursor.ibeam"
    case .ai: return "sparkles"
    case .dictionary: return "books.vertical"
    case .layout: return "keyboard"
    case .voice: return "mic"
    case .handwriting: return "hand.draw"
    case .developer: return "hammer"
    }
  }

  /// The phone rows keep the identifiers the old card dashboard used, so UI tests and anything else that finds a page by id still reach it.
  var linkIdentifier: String {
    switch self {
    case .skin: return "skinSettingsLink"
    case .candidate: return "candidateOptionsSettingsLink"
    case .toolbar: return "keyboardToolbarSettingsLink"
    case .input: return "inputSettingsLink"
    case .ai: return "aiSettingsLink"
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
    case .ai: ServiceSettingsView(kind: .ai)
    case .dictionary: DictionarySettingsView()
    case .layout: KeyboardLayoutSettingsView()
    case .voice: ServiceSettingsView(kind: .voice)
    case .handwriting: HandwritingSettingsView()
    case .developer: DeveloperOptionsView()
    }
  }

  static func matching(_ query: String) -> [[SettingsPage]] {
    let query = query.trimmingCharacters(in: .whitespaces)
    guard !query.isEmpty else { return groups }
    let found = groups.joined().filter { $0.title.localizedCaseInsensitiveContains(query) }
    return found.isEmpty ? [] : [found]
  }
}

/// A settings row label from the iOS design: a 29pt glyph box with no fill and the glyph in the secondary colour, a 17pt title and an optional value on the right. The chevron comes from the NavigationLink.
struct SettingsNavLabel: View {
  let title: String
  let symbol: String
  var value: String? = nil

  var body: some View {
    HStack(spacing: 12) {
      Image(systemName: symbol).font(.system(size: 20)).foregroundStyle(.secondary)
        .frame(width: 29, height: 29).accessibilityHidden(true)
      Text(title).font(.system(size: 17)).foregroundStyle(.primary)
      Spacer(minLength: 8)
      if let value {
        Text(value).font(.system(size: 17)).foregroundStyle(.secondary).lineLimit(1)
      }
    }
    .contentShape(Rectangle())
  }
}

/// The card at the top of 设置. The host app cannot tell whether the keyboard has been added or given full access -- iOS offers no public API for either, and a flag the keyboard writes would stay set after the user revokes access -- so the two steps are shown as instructions, not as checked states.
struct KeyboardStatusCard: View {
  let scheme: ChineseInputScheme

  var body: some View {
    VStack(alignment: .leading, spacing: 14) {
      HStack(spacing: 12) {
        Image("MSIMELogo").resizable().scaledToFit().frame(width: 44, height: 44)
          .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous)).accessibilityHidden(true)
        VStack(alignment: .leading, spacing: 2) {
          Text("水杉输入法").font(.system(size: 17, weight: .semibold))
          Text("当前方案 · \(scheme.title)").font(.system(size: 13)).foregroundStyle(.secondary)
            .accessibilityIdentifier("keyboardStatusScheme")
        }
      }
      VStack(alignment: .leading, spacing: 10) {
        step(1, "添加水杉键盘", "设置 → 通用 → 键盘 → 键盘 → 添加新键盘")
        step(2, "打开「允许完全访问」", "云候选、云剪贴板和同步需要它，日常输入不联网")
      }
      Button {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
      } label: {
        Text("去开启").font(.system(size: 15, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
          .frame(maxWidth: .infinity).frame(height: 40)
          .background(MetasequoiaTheme.accentSoft, in: Capsule())
          .contentShape(Capsule())
      }
      .buttonStyle(.plain)
      .accessibilityIdentifier("openKeyboardSettingsButton")
      .accessibilityHint("打开水杉输入法的系统设置页面")
    }
    .padding(.vertical, 6)
  }

  private func step(_ number: Int, _ title: String, _ detail: String) -> some View {
    HStack(alignment: .top, spacing: 10) {
      Text("\(number)").font(.system(size: 13, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
        .frame(width: 22, height: 22).background(MetasequoiaTheme.accentSoft, in: Circle())
      VStack(alignment: .leading, spacing: 2) {
        Text(title).font(.system(size: 15))
        Text(detail).font(.system(size: 13)).foregroundStyle(.secondary)
          .fixedSize(horizontal: false, vertical: true)
      }
    }
    .accessibilityElement(children: .combine)
  }
}

/// Section header in the grouped lists: 13pt regular in the design's group-title grey, not the uppercase footnote UIKit draws by default.
struct SettingsGroupHeader: View {
  let title: String
  var body: some View {
    Text(title).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.groupTitle).textCase(nil)
  }
}
