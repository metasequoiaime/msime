import SwiftUI

struct CommunitySkinTrialView: View {
  let trial: KeyboardSkinTrial
  @State private var error: String?
  @Environment(\.dismiss) private var dismiss
  var body: some View {
    NavigationView {
      VStack(spacing: 0) {
        VStack(alignment: .leading, spacing: 9) {
          Label("正在试用 · \(trial.name)", systemImage: "paintpalette.fill").font(.subheadline.weight(.semibold))
          Text("在下方打几个字试试。不满意可恢复原皮肤；退出试用也会自动恢复。")
            .font(.caption).foregroundStyle(.secondary)
          HStack {
            Button("恢复原皮肤") { finish(keep: false) }.buttonStyle(.bordered).accessibilityIdentifier("restoreTrialSkin")
            Spacer()
            Button("保留使用") { finish(keep: true) }.buttonStyle(.borderedProminent).accessibilityIdentifier("keepTrialSkin")
          }
        }.padding(14).background(MetasequoiaTheme.accentSoft)
        KeyboardTryoutView(focusOnAppear: true)
      }
    }.navigationViewStyle(.stack)
      .alert("试用皮肤", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
        Button("好", role: .cancel) {}
      } message: { Text(error ?? "") }
  }
  private func finish(keep: Bool) {
    do { try KeyboardSkinTrialStore().finish(trial.id, keep: keep); dismiss() }
    catch { self.error = error.localizedDescription }
  }
}
