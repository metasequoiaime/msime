import SwiftUI

/// 键盘工具栏：键盘顶部工具栏如何显示、上面放哪些功能，这些设置位于它们组成的工具栏预览下方。可从 键盘 → 更多 和搜索进入。
///
/// 与 Android 一样，显示方式以及常用语 / 输入方式两个开关属于本设备自己（`TouchToolbarLocalPreference`）；其他开关是共享的 `touch_toolbar`。
struct KeyboardToolbarSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var toolbar = TouchToolbarPreference()
  @State private var phrases = TouchToolbarLocalPreference.phrases
  @State private var scheme = TouchToolbarLocalPreference.scheme
  @State private var hidden = TouchToolbarLocalPreference.hidden
  @State private var skin = KeyboardTheme.current
  @State private var saveFailed = false

  /// 开关的存储位置。
  private enum Source {
    case shared(WritableKeyPath<TouchToolbarPreference, Bool>)
    case phrases
    case scheme
  }

  /// 按设计稿顺序和措辞排列的开关：表情 / 常用语 / 剪贴板 / 皮肤 / 输入方式，之后是 iOS 独有的项。标题是本页自己的；共享的 `TouchToolbarPreference.options` 保留共享设置页的措辞。
  private static let buttons: [(name: String, title: String)] = [
    ("emoji", "表情"), ("phrases", "常用语"), ("clipboard", "剪贴板"), ("skin", "皮肤"), ("scheme", "输入方式"),
    ("ai", "AI 润色"), ("character_set", "简繁切换"), ("fullwidth", "全角 / 半角"),
    ("punctuation", "中英文标点"),
  ]

  private static let rows: [(name: String, title: String, source: Source)] =
    buttons.compactMap { button in
      switch button.name {
      case "phrases": return (button.name, button.title, .phrases)
      case "scheme": return (button.name, button.title, .scheme)
      default:
        return TouchToolbarPreference.options.first { $0.name == button.name }.map { (button.name, button.title, .shared($0.keyPath)) }
      }
    }

  /// 与 Android 提供的一致的显示方式：输入时显示和隐藏。不提供始终显示，因为打字时候选会占用工具栏那一行。
  private static let displayModes = [
    DesignOption(title: "输入时显示", value: false, identifier: "appToolbarShownWhileTyping"),
    DesignOption(title: "隐藏", value: true, identifier: "appToolbarHidden"),
  ]

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        preview
        DesignGroup(title: "显示", footer: hidden ? "工具栏已隐藏，只显示候选栏。" : nil) {
          DesignSelectRow(title: "显示方式", options: Self.displayModes,
                          selection: Binding(get: { hidden }, set: { enabled in
                            TouchToolbarLocalPreference.hidden = enabled
                            hidden = enabled
                          }),
                          sheetTitle: "显示方式", identifier: "appToolbarDisplayMode")
        }
        DesignGroup(title: "按钮", footer: saveFailed ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。" : nil) {
          ForEach(Array(Self.rows.enumerated()), id: \.element.name) { index, row in
            if index > 0 { DesignDivider() }
            DesignToggleRow(title: row.title, isOn: binding(row.source))
              .accessibilityIdentifier("appToolbar_\(row.name)")
          }
        }
        // 工具栏隐藏时它的按钮都不显示，所以开关暂不可用，与 Android 把它们置灰一致。
        .disabled(hidden)
        .opacity(hidden ? 0.5 : 1)
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("键盘工具栏").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { reload() }
    }
  }

  /// 当前皮肤下的键盘，带着这些开关组成的工具栏，工具栏隐藏时只有按键；裁成设计稿的 14pt 圆角，外加一圈细线。
  private var preview: some View {
    let shape = RoundedRectangle(cornerRadius: 14, style: .continuous)
    let strip: KeyboardPreviewTopStrip = hidden
      ? .hidden : .toolbar(Self.previewItems(toolbar, phrases: phrases, scheme: scheme))
    return KeyboardSkinPreview(skin: skin, nineKey: false, topStrip: strip)
      .clipShape(shape)
      .overlay(shape.strokeBorder(MetasequoiaTheme.hair, lineWidth: 1))
      .accessibilityElement(children: .ignore)
      .accessibilityLabel("工具栏预览")
      .accessibilityIdentifier("keyboardToolbarPreview")
  }

  /// 按键盘绘制顺序排列的按钮：表情、常用语、剪贴板、皮肤、输入方式，然后是已启用的 iOS 独有项。
  static func previewItems(_ toolbar: TouchToolbarPreference, phrases: Bool, scheme: Bool) -> [KeyboardPreviewToolbarItem] {
    let ordered: [(enabled: Bool, item: KeyboardPreviewToolbarItem)] = [
      (toolbar.emoji, .emoji), (phrases, .phrases), (toolbar.clipboard, .clipboard), (toolbar.skin, .skin),
      (scheme, .scheme), (toolbar.ai, .ai), (toolbar.characterSet, .characterSet),
      (toolbar.fullwidth, .fullwidth), (toolbar.punctuation, .punctuation),
    ]
    return ordered.filter(\.enabled).map(\.item)
  }

  private func binding(_ source: Source) -> Binding<Bool> {
    switch source {
    case .shared(let keyPath):
      return Binding(get: { toolbar[keyPath: keyPath] }, set: { enabled in
        var next = toolbar
        next[keyPath: keyPath] = enabled
        save(next)
      })
    case .phrases:
      return Binding(get: { phrases }, set: { enabled in
        TouchToolbarLocalPreference.phrases = enabled
        phrases = enabled
      })
    case .scheme:
      return Binding(get: { scheme }, set: { enabled in
        TouchToolbarLocalPreference.scheme = enabled
        scheme = enabled
      })
    }
  }

  /// A refused write leaves the switch where the document is, instead of showing a bar the keyboard will not draw.
  private func save(_ next: TouchToolbarPreference) {
    saveFailed = !TouchToolbarPreference.save(next)
    toolbar = saveFailed ? TouchToolbarPreference.load() : next
  }

  /// 文档就是键盘将要使用的内容，包括从其他设备同步过来、还没有键盘镜像到 App Group 的值。本设备自己的开关会一并读取。
  private func reload() {
    phrases = TouchToolbarLocalPreference.phrases
    scheme = TouchToolbarLocalPreference.scheme
    hidden = TouchToolbarLocalPreference.hidden
    let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
    skin = KeyboardTheme.reload(preferences)
    guard let preferences else { return }
    toolbar = TouchToolbarPreference(in: preferences)
  }
}
