import SwiftUI
import UIKit
import PhotosUI
import UniformTypeIdentifiers
import ImageIO

struct CustomSkinEditorView: View {
  /// When opened from the publish flow, the editor should return to that flow instead of offering another publish entry.
  var publishable = true
  @Environment(\.scenePhase) private var scenePhase
  @State private var design = CustomKeyboardSkinStore.current
  @State private var nineKey = InputSchemePreference.scheme == .nineKey
  @State private var confirmReset = false
  @State private var section = "背景"
  @State private var undo: [CustomKeyboardSkin] = []
  @State private var redo: [CustomKeyboardSkin] = []
  @State private var sliderStart: CustomKeyboardSkin?
  @State private var saved = CustomSkinLibrary.designs
  @State private var showSave = false
  @State private var name = ""
  @State private var renaming: UUID?
  @State private var deleting: SavedKeyboardSkin?
  @State private var replacing: SavedKeyboardSkin?
  @State private var showAI = false
  @State private var showPhotos = false
  @State private var publishingSkin: SavedKeyboardSkin?
  @State private var message: String?
  @AppStorage(KeyboardFeedbackPreference.soundKey, store: KeyboardFeedbackPreference.defaults) private var soundEnabled = true
  @AppStorage(KeyboardFeedbackPreference.hapticsKey, store: KeyboardFeedbackPreference.defaults) private var hapticsEnabled = false
  @AppStorage(KeyboardFeedbackPreference.strengthKey, store: KeyboardFeedbackPreference.defaults) private var hapticStrength = KeyboardHapticStrength.medium.rawValue
  @State private var feedback: UIImpactFeedbackGenerator?
  @State private var documentWrite: Task<Void, Never>?


  private func apply(_ next: CustomKeyboardSkin, record: Bool = true) {
    let next = next.normalized
    guard next != design else { return }
    if record { undo.append(design); undo = Array(undo.suffix(30)); redo.removeAll() }
    CustomKeyboardSkinStore.save(next)
    design = next
    scheduleDocumentWrite()
  }

  /// The keyboard reads the design from the shared document, which a colour drag would otherwise rewrite on every frame; the write waits for the edits to settle. The App Group copy above only feeds this page's previews.
  private func scheduleDocumentWrite() {
    documentWrite?.cancel()
    documentWrite = Task { @MainActor in
      try? await Task.sleep(nanoseconds: 400_000_000)
      guard !Task.isCancelled else { return }
      documentWrite = nil
      writeDocument()
    }
  }

  private func flushDocumentWrite() {
    guard let pending = documentWrite else { return }
    pending.cancel()
    documentWrite = nil
    writeDocument()
  }

  private func writeDocument() {
    guard let mapping = GlobalThemePreference.storingDesign(design), GlobalThemePreference.update(mapping) else {
      message = "这次修改没能交给键盘，请稍后再改一次。"
      return
    }
  }

  private func useCustomSkin() {
    documentWrite?.cancel()
    documentWrite = nil
    if !GlobalThemePreference.apply(design) {
      message = "没能切换到我的皮肤，请稍后重试。"
    }
  }
  @AppStorage(GlobalThemePreference.key, store: KeyboardFeedbackPreference.defaults)
  private var selected = GlobalThemeCatalog.systemId

  private func update<T>(_ path: WritableKeyPath<CustomKeyboardSkin, T>, _ value: T) {
    var next = design
    next[keyPath: path] = value
    apply(next, record: sliderStart == nil)
  }

  private func trackSlider(_ editing: Bool) {
    if editing { sliderStart = design }
    else {
      if let start = sliderStart, start != design { undo.append(start); undo = Array(undo.suffix(30)); redo.removeAll() }
      sliderStart = nil
    }
  }

  private func color(_ path: WritableKeyPath<CustomKeyboardSkin, UInt32>) -> Binding<Color> {
    Binding(get: { Color(uiColor: CustomKeyboardSkin.color(design[keyPath: path])) },
            set: { update(path, CustomKeyboardSkin.rgb(UIColor($0))) })
  }

  private func value<T>(_ path: WritableKeyPath<CustomKeyboardSkin, T>) -> Binding<T> {
    Binding(get: { design[keyPath: path] }, set: { update(path, $0) })
  }

  private var activeCategory: String { section }

  private var categoryBar: some View {
    HStack(spacing: 0) {
      ForEach([("设计", "square.grid.2x2"), ("背景", "rectangle.on.rectangle"), ("按键", "square.on.square"), ("文本", "textformat"), ("音效", "music.note"), ("我的", "square.stack")], id: \.0) { title, symbol in
        Button { section = title } label: {
          VStack(spacing: 5) {
            Group {
              if title == "文本" { Text("T").font(.system(size: 24, weight: .medium, design: .serif)) }
              else { Image(systemName: symbol).font(.system(size: 22, weight: .regular)) }
            }.frame(height: 26)
            Text(title).font(.system(size: 13, weight: activeCategory == title ? .semibold : .regular))
          }.frame(maxWidth: .infinity).frame(height: 64)
            .foregroundStyle(activeCategory == title ? MetasequoiaTheme.accent : Color.secondary)
            .contentShape(Rectangle())
        }.buttonStyle(.plain).accessibilityIdentifier("skinEditorTab_" + title)
          .accessibilityAddTraits(activeCategory == title ? .isSelected : [])
      }
    }.background(Color(uiColor: .systemBackground))
  }

  private func previewDock() -> some View {
    VStack(spacing: 0) {
      Divider()
      HStack(spacing: 10) {
        Button {
          guard let previous = undo.popLast() else { return }; redo.append(design); apply(previous, record: false)
        } label: { Image(systemName: "arrow.uturn.backward").frame(width: 36, height: 38) }
          .disabled(undo.isEmpty || sliderStart != nil).accessibilityLabel("撤销设计").accessibilityIdentifier("undoSkinDesign")
        Button {
          guard let next = redo.popLast() else { return }; undo.append(design); apply(next, record: false)
        } label: { Image(systemName: "arrow.uturn.forward").frame(width: 36, height: 38) }
          .disabled(redo.isEmpty || sliderStart != nil).accessibilityLabel("重做设计").accessibilityIdentifier("redoSkinDesign")
        Picker("预览布局", selection: $nineKey) {
          Text("26 键").tag(false); Text("9 键").tag(true)
        }.pickerStyle(.segmented).frame(width: 124).accessibilityIdentifier("skinEditorPreviewLayout")
        Spacer(minLength: 0)
        Button { useCustomSkin() } label: {
          Label(selected == GlobalThemeCatalog.customId ? "正在使用" : "使用皮肤", systemImage: "checkmark.circle.fill")
            .font(.caption.weight(.semibold))
        }.accessibilityIdentifier("applyCustomSkin")
      }.padding(.horizontal, 12).background(Color(uiColor: .systemBackground))
      KeyboardSkinPreview(skin: .designed(design), nineKey: nineKey).id(design)
        .accessibilityIdentifier("fullKeyboardSkinPreview")
    }
  }

  private var backgroundGallery: some View {
    Section {
      Button { showAI = true } label: {
        Label("AI 皮肤抽卡", systemImage: "sparkles").font(.headline)
          .frame(maxWidth: .infinity).padding(.vertical, 10)
      }.accessibilityIdentifier("openAISkinDesigner")

      HStack(spacing: 12) {
        Button { section = "设计" } label: {
          Label("设计模板", systemImage: "square.grid.2x2").frame(maxWidth: .infinity).frame(height: 44)
        }.accessibilityIdentifier("skinEditorTemplates")
        Button { showPhotos = true } label: {
          Label("相册", systemImage: "photo.badge.plus").frame(maxWidth: .infinity).frame(height: 44)
        }.accessibilityIdentifier("skinEditorAlbum")
      }.buttonStyle(.bordered).tint(MetasequoiaTheme.accent)
      LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: 3), spacing: 8) {
        ForEach(Array(backgroundPresets.enumerated()), id: \.offset) { index, preset in
          let active = design.photo == nil && design.background == preset.0 && design.gradientEnd == preset.1
          Button {
            var next = design; next.photo = nil; next.background = preset.0; next.gradientEnd = preset.1
            next.gradientHorizontal = false
            apply(next)
          } label: {
            RoundedRectangle(cornerRadius: 10)
              .fill(LinearGradient(colors: [Color(uiColor: CustomKeyboardSkin.color(preset.0)), Color(uiColor: CustomKeyboardSkin.color(preset.1 ?? preset.0))], startPoint: .topLeading, endPoint: .bottomTrailing))
              .frame(height: 74)
              .overlay(RoundedRectangle(cornerRadius: 10).stroke(active ? MetasequoiaTheme.accent : Color.primary.opacity(0.08), lineWidth: active ? 2 : 1))
              .overlay(alignment: .bottomTrailing) {
                if active { Image(systemName: "checkmark.circle.fill").foregroundStyle(MetasequoiaTheme.onAccent, MetasequoiaTheme.accent).padding(6) }
              }
          }.buttonStyle(.plain).accessibilityLabel(preset.2)
            .accessibilityIdentifier("skinBackgroundPreset_\(index)")
            .accessibilityAddTraits(active ? .isSelected : [])
        }
      }
    }.listRowInsets(EdgeInsets(top: 10, leading: 12, bottom: 10, trailing: 12))
  }

  private var backgroundPresets: [(UInt32, UInt32?, String)] {
    [(0xFFFFFF, nil, "纯白"), (0xDFE2EB, nil, "雾灰"), (0x171717, nil, "曜黑"),
     (0xFFD5DB, nil, "樱粉"), (0xAEE3F3, nil, "晴空"), (0xC6E9A7, nil, "嫩绿"),
     (0x160A3F, 0xA747DF, "紫夜渐变"), (0xC5F5FF, 0xFAD8F6, "极光渐变"), (0x185C47, 0x80BFA8, "森林渐变")]
  }

  private var soundControls: some View {
    Section {
      Toggle("按键音", isOn: $soundEnabled).accessibilityIdentifier("skinEditorSound")
      if KeyboardFeedbackPreference.hapticsAvailable {
        Toggle("按键振动", isOn: $hapticsEnabled).accessibilityIdentifier("skinEditorHaptics")
        Picker("振动强度", selection: $hapticStrength) {
          ForEach(KeyboardHapticStrength.allCases, id: \.rawValue) { Text($0.title).tag($0.rawValue) }
        }.disabled(!hapticsEnabled)
        Button("试一下振动") {
          if hapticsEnabled {
            let strength = KeyboardHapticStrength(rawValue: hapticStrength) ?? .medium
            feedback = UIImpactFeedbackGenerator(style: strength.style)
            feedback?.prepare(); feedback?.impactOccurred(intensity: strength.intensity)
          }
        }.disabled(!hapticsEnabled)
      }
    } header: { Text("打字反馈") } footer: {
      Text(KeyboardFeedbackPreference.hapticsAvailable
        ? "音效与振动是所有皮肤共用的键盘设置。按键音受系统静音状态影响，振动需在支持的真机上体验。"
        : "按键音是所有皮肤共用的键盘设置，受系统静音状态影响。")
    }
  }

  private var editorControls: some View {
  Form {
    if section == "背景" { backgroundGallery }
    if section == "设计" {
      Section("水杉设计 · 14 款") {
        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
          ForEach(CustomKeyboardSkin.curatedTemplates + Array(CustomKeyboardSkin.templates.prefix(6)), id: \.0) { title, template in
            Button { apply(template) } label: {
              VStack(alignment: .leading, spacing: 10) {
                CommunityDesignPreview(design: template)
                Text(title).font(.caption.weight(.semibold)).foregroundStyle(Color(uiColor: CustomKeyboardSkin.color(template.accent)))
              }.padding(10).background(LinearGradient(colors: [Color(uiColor: CustomKeyboardSkin.color(template.background)), Color(uiColor: CustomKeyboardSkin.color(template.gradientEnd ?? template.background))], startPoint: .topLeading, endPoint: .bottomTrailing), in: RoundedRectangle(cornerRadius: 12))
            }.buttonStyle(.plain).accessibilityIdentifier("skinTemplate_" + title)
          }
        }
      }
      Section { Text("选择模板后，继续调整背景、按键和文本。修改自动保存；撤销可找回刚才的设计，命名保存可保留多套方案。").font(.footnote).foregroundStyle(.secondary) }
      Section { Button("重置我的皮肤", role: .destructive) { confirmReset = true } }
    }
    if section == "背景" {
      Section("背景底色") {
        ColorPicker("背景起始色", selection: color(\.background), supportsOpacity: false)
        Toggle("渐变背景", isOn: Binding(get: { design.gradientEnd != nil }, set: { update(\.gradientEnd, $0 ? design.background : nil) }))
          .accessibilityIdentifier("customSkinGradient")
        if design.gradientEnd != nil {
          ColorPicker("渐变结束色", selection: Binding(get: { Color(uiColor: CustomKeyboardSkin.color(design.gradientEnd ?? design.background)) }, set: { update(\.gradientEnd, CustomKeyboardSkin.rgb(UIColor($0))) }), supportsOpacity: false)
          Toggle("横向渐变", isOn: Binding(get: { design.gradientHorizontal ?? false }, set: { update(\.gradientHorizontal, $0) }))
        }
      }
      Section("照片壁纸") {
        Button { showPhotos = true } label: { Label(design.photo == nil ? "选择照片" : "更换照片", systemImage: "photo") }
          .accessibilityIdentifier("customSkinPhoto")
        if design.photo != nil {
          Text("照片位置").font(.subheadline)
          Slider(value: Binding(get: { design.photoPosition ?? 0.5 }, set: { update(\.photoPosition, $0) }), in: 0...1, onEditingChanged: trackSlider)
          Text("压暗照片 · \(Int((design.photoShade ?? 0.25) * 100))%")
          Slider(value: Binding(get: { design.photoShade ?? 0.25 }, set: { update(\.photoShade, $0) }), in: 0...0.8, onEditingChanged: trackSlider)
          Button("移除照片", role: .destructive) { update(\.photo, nil) }
        }
      }
      Section("纹理") {
        Picker("背景纹理", selection: value(\.pattern)) {
          Text("纯色").tag(0); Text("网点").tag(1); Text("网格").tag(2); Text("波纹").tag(3)
        }.accessibilityIdentifier("customSkinPattern")
        if design.pattern != 0 {
          Text("纹理强度 · \(Int((design.patternOpacity ?? 0.15) * 100))%")
          Slider(value: Binding(get: { design.patternOpacity ?? 0.15 }, set: { update(\.patternOpacity, $0) }), in: 0...0.5, onEditingChanged: trackSlider)
        }
      }
    }
    if section == "文本" {
Section("文字与提示") {
  Toggle("等宽字形", isOn: value(\.monospaced)).accessibilityIdentifier("customSkinMonospaced")
  ColorPicker("按键文字", selection: color(\.keyForeground), supportsOpacity: false)
  ColorPicker("提示与工具栏", selection: color(\.accent), supportsOpacity: false)
  if !design.hasReadableText {
    Label("部分文字与背景对比度偏低，建议调整配色。", systemImage: "eye")
      .font(.footnote).foregroundStyle(.secondary)
  }
  Button("优化文字对比度") {
    var next = design
    next.keyForeground = CustomKeyboardSkin.readableText(on: next.keyBackground)
    let black = min(CustomKeyboardSkin.contrast(0, next.background), CustomKeyboardSkin.contrast(0, next.keyBackground))
    let white = min(CustomKeyboardSkin.contrast(0xFFFFFF, next.background), CustomKeyboardSkin.contrast(0xFFFFFF, next.keyBackground))
    next.accent = black >= white ? 0 : 0xFFFFFF
    apply(next)
  }
}
}
if section == "按键" {
Section("按键配色") {
  ColorPicker("键帽颜色", selection: color(\.keyBackground), supportsOpacity: false)
  ColorPicker("功能键颜色", selection: color(\.actionBackground), supportsOpacity: false)
}
Section("键帽设计") {
  Picker("键帽造型", selection: Binding(get: { design.keyShape ?? .rounded }, set: { update(\.keyShape, $0) })) {
    ForEach(SkinKeyShape.allCases, id: \.self) { Text($0.title).tag($0) }
  }.accessibilityIdentifier("customSkinKeyShape")
  Picker("键帽材质", selection: Binding(get: { design.keyMaterial ?? .flat }, set: { update(\.keyMaterial, $0) })) {
    ForEach(SkinKeyMaterial.allCases, id: \.self) { Text($0.title).tag($0) }
  }.accessibilityIdentifier("customSkinKeyMaterial")
  VStack(alignment: .leading) {
    Text("键帽不透明度 · \(Int((design.keyOpacity ?? 1) * 100))%")
    Slider(value: Binding(get: { design.keyOpacity ?? 1 }, set: { update(\.keyOpacity, $0) }), in: 0.25...1, onEditingChanged: trackSlider)
      .accessibilityIdentifier("customSkinKeyOpacity")
  }
  if design.photo != nil || (design.keyOpacity ?? 1) < 1 {
    Text("半透明键帽会露出背景。可在“背景”中压暗照片，让文字更清晰。").font(.footnote).foregroundStyle(.secondary)
  }
  VStack(alignment: .leading) {
    Text("圆角 · \(Int(design.cornerRadius))")
    Slider(value: value(\.cornerRadius), in: 0...20, step: 1, onEditingChanged: trackSlider)
      .accessibilityIdentifier("customSkinCornerRadius")
  }
  VStack(alignment: .leading) {
    Text("边框 · \(design.borderWidth, specifier: "%.1f")")
    Slider(value: value(\.borderWidth), in: 0...2, step: 0.5, onEditingChanged: trackSlider)
  }
  VStack(alignment: .leading) {
    Text("阴影 · \(Int(design.shadow * 100))%")
    Slider(value: value(\.shadow), in: 0...0.4, step: 0.05, onEditingChanged: trackSlider)
  }
  ColorPicker("边框颜色", selection: Binding(get: { Color(uiColor: CustomKeyboardSkin.color(design.customBorderColor ?? design.accent)) }, set: { update(\.customBorderColor, CustomKeyboardSkin.rgb(UIColor($0))) }), supportsOpacity: false)
}
}
if section == "我的" {
  Section("我的皮肤 · \(saved.count)/12") {
    if saved.isEmpty { Text("还没有命名保存的皮肤。调整满意后，点击右上角“保存”。").foregroundStyle(.secondary) }
    ForEach(saved) { item in
      HStack {
        Button { apply(item.design) } label: {
          HStack {
            RoundedRectangle(cornerRadius: 6).fill(Color(uiColor: CustomKeyboardSkin.color(item.design.background))).frame(width: 32, height: 32)
            Text(item.name)
          }
        }.accessibilityIdentifier("savedSkin_" + item.name)
        Spacer()
        Menu {
          if publishable { Button("发布到社区") { publishingSkin = item } }
          Button("用当前设计更新") { replacing = item }
          Button("重命名") { renaming = item.id; name = item.name; showSave = true }
          Button("删除", role: .destructive) { deleting = item }
        } label: { Image(systemName: "ellipsis.circle").frame(width: 44, height: 44) }
          .accessibilityLabel("管理" + item.name)
      }
    }
  }
}
    if section == "音效" { soundControls }
  }
  .id(section)
  .accessibilityIdentifier("skinEditorControls")
  }

  var body: some View {
    GeometryReader { geometry in
      VStack(spacing: 0) {
        categoryBar
        if geometry.size.width > geometry.size.height {
          HStack(spacing: 0) {
            editorControls
            previewDock().frame(width: geometry.size.width * 0.55)
          }
        } else {
          editorControls
          previewDock()
        }
      }
      .background(Color(uiColor: .systemGroupedBackground))
    }
    .onChange(of: scenePhase) { phase in
      guard phase == .active else { return }
      let current = CustomKeyboardSkinStore.current
      if current != design { design = current; undo.removeAll(); redo.removeAll() }
      saved = CustomSkinLibrary.designs
    }
    .onDisappear { flushDocumentWrite() }
    .navigationTitle("自定义皮肤")
    .navigationBarTitleDisplayMode(.inline)
    .toolbar {
      ToolbarItem(placement: .navigationBarTrailing) {
        HStack(spacing: 10) {
          Menu {
            Button("AI 皮肤抽卡", systemImage: "sparkles") { showAI = true }
            Button("设计模板", systemImage: "square.grid.2x2") { section = "设计" }
            Button("我的皮肤", systemImage: "square.stack") { section = "我的" }
            Button("重做", systemImage: "arrow.uturn.forward") {
              guard let next = redo.popLast() else { return }; undo.append(design); apply(next, record: false)
            }.disabled(redo.isEmpty || sliderStart != nil)
            Button("重置我的皮肤", role: .destructive) { confirmReset = true }
          } label: { Image(systemName: "ellipsis.circle") }
            .accessibilityIdentifier("skinEditorTools")
          Button { renaming = nil; name = "我的设计 \(saved.count + 1)"; showSave = true } label: {
            Text("保存").font(.subheadline.weight(.semibold)).padding(.horizontal, 14).padding(.vertical, 7)
              .foregroundStyle(MetasequoiaTheme.onAccent).background(MetasequoiaTheme.accent, in: Capsule())
          }.accessibilityIdentifier("saveCustomSkin")
        }
      }
    }
    .sheet(isPresented: $showAI, onDismiss: { saved = CustomSkinLibrary.designs }) {
      AISkinGenerationView { next in apply(next); useCustomSkin(); section = "按键" }
    }
    .sheet(item: $publishingSkin) { item in SavedSkinPublishFlow(skinID: item.id) }
    .sheet(isPresented: $showPhotos) {
      SkinPhotoPicker { data in
        showPhotos = false
        if let data { update(\.photo, data) }
        else { message = "无法读取这张照片，请换一张再试。" }
      }
    }
    .sheet(isPresented: $showSave) {
      NavigationView {
        Form {
          TextField("皮肤名称", text: $name).accessibilityIdentifier("customSkinName")
            .onChange(of: name) { if $0.count > 32 { name = String($0.prefix(32)) } }
          Button("保存") {
            let title = String(name.trimmingCharacters(in: .whitespacesAndNewlines).prefix(32))
            guard !saved.contains(where: { $0.id != renaming && $0.name == title }) else {
              showSave = false; message = "已经有同名皮肤，请换一个名称。"; return
            }
            if let id = renaming, let index = saved.firstIndex(where: { $0.id == id }) { saved[index].name = title }
            else if saved.count < 12 { saved.append(SavedKeyboardSkin(name: title, design: design)) }
            else { showSave = false; message = "最多保存 12 套皮肤，请先删除不需要的设计。"; return }
            guard CustomSkinLibrary.save(saved) else {
              saved = CustomSkinLibrary.designs
              showSave = false; message = "保存失败，请检查设备可用空间后重试。"; return
            }
            if renaming == nil { useCustomSkin() }
            showSave = false
            section = "我的"
          }.disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .accessibilityIdentifier("confirmSaveCustomSkin")
        }.navigationTitle(renaming == nil ? "保存我的皮肤" : "重命名")
          .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { showSave = false } } }
      }
    }
    .alert("提示", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好") { message = nil }
    } message: { Text(message ?? "") }
    .confirmationDialog("用当前设计更新“\(replacing?.name ?? "")”？", isPresented: Binding(get: { replacing != nil }, set: { if !$0 { replacing = nil } }), titleVisibility: .visible) {
      Button("更新已保存的皮肤") {
        if let item = replacing, let index = saved.firstIndex(where: { $0.id == item.id }) {
          saved[index].design = design
          if !CustomSkinLibrary.save(saved) { saved = CustomSkinLibrary.designs; message = "保存失败，请重试。" }
        }
        replacing = nil
      }
    }
    .confirmationDialog("删除这套已保存的皮肤？", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }), titleVisibility: .visible) {
      Button("删除", role: .destructive) {
        if let item = deleting {
          saved.removeAll { $0.id == item.id }
          if !CustomSkinLibrary.save(saved) { saved = CustomSkinLibrary.designs; message = "删除失败，请重试。" }
        }
        deleting = nil
      }
    }
    .confirmationDialog("恢复默认配色和键帽设计？", isPresented: $confirmReset, titleVisibility: .visible) {
      Button("重置", role: .destructive) {
        let initial = CustomKeyboardSkin()
        apply(initial)
      }
      Button("取消", role: .cancel) {}
    }
  }
}

// The system picker grants access only to the chosen image; no library-wide permission is needed.
struct SkinPhotoPicker: UIViewControllerRepresentable {
  let receive: (Data?) -> Void
  @Environment(\.dismiss) private var dismiss
  func makeCoordinator() -> Coordinator { Coordinator(receive: receive, cancel: { dismiss() }) }
  func makeUIViewController(context: Context) -> PHPickerViewController {
    var config = PHPickerConfiguration()
    config.filter = .images; config.selectionLimit = 1
    let picker = PHPickerViewController(configuration: config)
    picker.delegate = context.coordinator
    return picker
  }
  func updateUIViewController(_ view: PHPickerViewController, context: Context) {}
  final class Coordinator: NSObject, PHPickerViewControllerDelegate {
    let receive: (Data?) -> Void
    let cancel: () -> Void
    init(receive: @escaping (Data?) -> Void, cancel: @escaping () -> Void) { self.receive = receive; self.cancel = cancel }
    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
      guard let item = results.first else { cancel(); return }
      item.itemProvider.loadFileRepresentation(forTypeIdentifier: UTType.image.identifier) { [self] url, _ in
        let result = url.flatMap { SkinPhotoData.thumbnail(at: $0) }
        DispatchQueue.main.async { self.receive(result) }
      }
    }
  }
}
