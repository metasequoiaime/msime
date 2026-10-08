import SwiftUI

/// 手写输入，对应 Android 的 `HandwritingPage`：书写里是识别等待时间，笔迹里是墨迹颜色和粗细，方案显示并选择手写方案。说明文字和 ML Kit 条款原先放在输入设置里；那里的方案开关也保留，因为那个页面列出所有方案。
///
/// 不提供设计稿中的书写模式和识别后显示拼音：识别器没有多字或叠写模式，也不输出拼音。书写和笔迹的值是 App Group 中的 defaults（`HandwritingPreference`），键盘在打开手写面板时读取。
struct HandwritingSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var enabledSchemes = InputSchemePreference.enabledSchemes
  @State private var current = InputSchemePreference.scheme
  @State private var saveFailed = false
  @State private var delay = Double(HandwritingPreference.delayMilliseconds)
  @State private var strokeColor = HandwritingPreference.strokeColor
  @State private var strokeWidth = Double(HandwritingPreference.strokeWidth)

  private var enabled: Bool { enabledSchemes.contains(.handwriting) }

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "书写") {
          DesignSliderRow(title: "识别等待时间", value: Binding(get: { delay }, set: saveDelay),
                          range: Double(HandwritingPreference.delayRange.lowerBound)...Double(HandwritingPreference.delayRange.upperBound),
                          step: Double(HandwritingPreference.delayStep), identifier: "handwritingDelaySlider") {
            "\(Int($0))ms"
          }
        }

        DesignGroup(title: "笔迹") {
          DesignSelectRow(title: "笔迹颜色",
                          options: HandwritingPreference.StrokeColor.allCases.map { DesignOption(title: $0.title, value: $0) },
                          selection: Binding(get: { strokeColor }, set: saveStrokeColor),
                          sheetTitle: "笔迹颜色", identifier: "handwritingStrokeColorPicker")
          DesignDivider()
          DesignSliderRow(title: "笔迹粗细", value: Binding(get: { strokeWidth }, set: saveStrokeWidth),
                          range: Double(HandwritingPreference.strokeWidthRange.lowerBound)...Double(HandwritingPreference.strokeWidthRange.upperBound),
                          step: 1, identifier: "handwritingStrokeWidthSlider") {
            "\(Int($0))px"
          }
        }

        DesignGroup(title: "方案", footer: saveFailed
          ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
          : "开启后，手写出现在键盘的方案切换里。至少保留一种方案。") {
          DesignToggleRow(title: "在键盘中显示手写", isOn: Binding(get: { enabled }, set: setEnabled))
            .disabled(enabled && enabledSchemes.count == 1)
            .accessibilityIdentifier("handwritingEnabledToggle")
          DesignDivider()
          Button {
            saveFailed = !InputSchemePreference.select(.handwriting)
            reload()
          } label: {
            HStack(spacing: 12) {
              Text("设为当前方案").font(.system(size: 17)).foregroundStyle(enabled ? Color.primary : Color.secondary)
              Spacer(minLength: 8)
              if current == .handwriting {
                Image(systemName: "checkmark").font(.system(size: 17, weight: .semibold))
                  .foregroundStyle(MetasequoiaTheme.accent).accessibilityHidden(true)
              }
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 20)
            .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
            .contentShape(Rectangle())
          }
          .buttonStyle(PressFillButtonStyle())
          .disabled(!enabled)
          .accessibilityIdentifier("handwritingSelectButton")
          .accessibilityValue(current == .handwriting ? "已选择" : "未选择")
        }

        DesignGroup(title: "识别模型",
                    footer: "首次在键盘中下载中文模型，需要完全访问权限。下载后可离线识别，笔迹和识别结果不会上传。Google ML Kit 会发送性能及使用统计。") {
          Link(destination: URL(string: "https://developers.google.com/ml-kit/terms")!) {
            HStack(spacing: 12) {
              Text("手写 SDK 隐私说明").font(.system(size: 17)).foregroundStyle(.primary)
              Spacer(minLength: 8)
              Image(systemName: "arrow.up.right").font(.system(size: 13, weight: .semibold))
                .foregroundStyle(MetasequoiaTheme.sub.opacity(0.55)).accessibilityHidden(true)
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 20)
            .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
            .contentShape(Rectangle())
          }
          .buttonStyle(PressFillButtonStyle())
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("手写输入").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { reload() }
    }
  }

  private func saveDelay(_ value: Double) {
    HandwritingPreference.delayMilliseconds = Int(value.rounded())
    delay = Double(HandwritingPreference.delayMilliseconds)
  }

  private func saveStrokeColor(_ value: HandwritingPreference.StrokeColor) {
    HandwritingPreference.strokeColor = value
    strokeColor = value
  }

  private func saveStrokeWidth(_ value: Double) {
    HandwritingPreference.strokeWidth = Int(value.rounded())
    strokeWidth = Double(HandwritingPreference.strokeWidth)
  }

  /// 只改启用列表，选中的方案以文档里的为准：页面上的 `current` 来自 App Group 镜像，可能落后于文档，写回去会把用户在别处选的方案改掉。
  private func setEnabled(_ on: Bool) {
    saveFailed = !InputSchemePreference.setEnabled(.handwriting, on)
    reload()
  }

  private func reload() {
    InputSchemePreference.mirror(MetasequoiaInputSessionBridge.loadSharedPreferences())
    enabledSchemes = InputSchemePreference.enabledSchemes
    current = InputSchemePreference.scheme
    delay = Double(HandwritingPreference.delayMilliseconds)
    strokeColor = HandwritingPreference.strokeColor
    strokeWidth = Double(HandwritingPreference.strokeWidth)
  }
}
