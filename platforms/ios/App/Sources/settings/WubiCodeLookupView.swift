import SwiftUI

/// 五笔查码（#6789）：输入或粘贴汉字，列出每个字（以及整段词）的 86 和 98 五笔编码，当前所选的码表那一列标「（当前）」。
///
/// 与共享设置页的「五笔查码」一组同样的文字和排法（`WubiCodeLookup`）。iOS 的产品是这个原生 App，没有 WebView，共享设置页那一组到不了用户手里，所以在这里原生画一遍。查码在 App 进程里经 `MetasequoiaInputSessionBridge.wubiCodes` 发词库请求 `wubi_codes`，读键盘扩展包里的同一份码表，不需要键盘在跑。
struct WubiCodeLookupView: View {
  /// 当前所选的五笔码表（`wubi86` 或 `wubi98`）。
  let profile: String
  @State private var text = ""
  @State private var lookupState = WubiLookupRequestState()

  var body: some View {
    Form {
      Section {
        TextField("例如：水杉输入法", text: $text, axis: .vertical)
          .lineLimit(2...6)
          .accessibilityLabel("要查五笔编码的文字")
          .accessibilityIdentifier("wubiCodeLookupText")
          .onChange(of: text) { _, next in
            let limited = String(next.prefix(WubiCodeLookup.maximumLength))
            if limited != next { text = limited }
            lookupState.edit(limited)
          }
        Button(lookupState.busy ? "正在查码…" : "查码") { lookup() }
          .disabled(lookupState.busy || !WubiCodeLookup.searchable(text))
          .accessibilityIdentifier("wubiCodeLookupButton")
      } header: {
        DesignSectionHeader(info: WubiCodeLookup.description) { SettingsGroupHeader(title: "查汉字的五笔编码") }
      }
      .designRow()
      if lookupState.failed {
        Section { Text(WubiCodeLookup.failed).foregroundStyle(.secondary) }
          .designRow()
      } else if let result = lookupState.result {
        Section {
          if result.rows.isEmpty {
            Text(WubiCodeLookup.nothing).foregroundStyle(.secondary)
          }
          ForEach(Array(result.rows.enumerated()), id: \.offset) { index, item in
            row(item).accessibilityIdentifier("wubiCodeLookupRow-\(index)")
          }
        } footer: {
          if let notice = result.truncationNotice() { Text(notice) }
        }
        .designRow()
      }
    }
    .designPage()
    .navigationTitle("五笔查码")
    .navigationBarTitleDisplayMode(.inline)
  }

  private func row(_ item: WubiCodeLookup.Codes) -> some View {
    VStack(alignment: .leading, spacing: 4) {
      Text(item.text).font(.system(size: 17))
      ForEach([("wubi86", item.wubi86), ("wubi98", item.wubi98)], id: \.0) { column, codes in
        HStack(alignment: .firstTextBaseline, spacing: 8) {
          Text(WubiCodeLookup.columnTitle(column, profile: profile))
            .designSubtext()
          Text(WubiCodeLookup.codes(codes)).font(.system(size: 15, design: .monospaced))
        }
      }
    }
    .padding(.vertical, 2)
    .accessibilityElement(children: .ignore)
    .accessibilityLabel(WubiCodeLookup.spoken(item, profile: profile))
  }

  /// 同一段输入只发一个请求，编辑后旧回答不能覆盖当前结果。
  private func lookup() {
    guard let generation = lookupState.begin() else { return }
    let query = lookupState.text
    Task {
      let answer = await Task.detached(priority: .userInitiated) {
        try? MetasequoiaInputSessionBridge.wubiCodes(query)
      }.value
      _ = lookupState.complete(generation, answer: answer)
    }
  }
}
