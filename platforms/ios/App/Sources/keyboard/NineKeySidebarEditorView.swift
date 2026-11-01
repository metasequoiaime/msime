import SwiftUI

/// 「按键布局」里两张左侧符号表共用的编辑页：「九键左侧符号」（9 键分段卡片里进来）和「数字键盘左侧符号」（「数字键盘」一组里进来），只差 `table`。照 Android `NineKeySidebarSheet` 的交互，三端相同：顶部一行说明，当前表可滑动删除、按住拖动调整顺序，下面是手动输入一项和还没加入的常用符号（点一下接到末尾），最后是「恢复默认」。整表合规（`NineKeySidebarDraft.canSave`）时右上角的「保存」才可点；保存只写这一张表，与 iOS 出厂表相同时写空数组，写不进去时提示并留在本页，文档里的值不变。不画键盘预览，回到「按键布局」页在 9 键预览里能看到。
struct NineKeySidebarEditorView: View {
  let table: NineKeySidebarTable
  @State private var draft: NineKeySidebarDraft
  @State private var manual = ""
  @State private var saveFailed = false
  @Environment(\.dismiss) private var dismiss

  init(table: NineKeySidebarTable) {
    self.table = table
    _draft = State(initialValue: NineKeySidebarDraft(table: table, style: TouchKeyLayoutStyle.mirrored))
  }

  var body: some View {
    List {
      Section {
        ForEach(Array(draft.symbols.enumerated()), id: \.offset) { index, symbol in
          Text(symbol).font(.system(size: 17)).foregroundStyle(.primary)
            .frame(maxWidth: .infinity, minHeight: 34, alignment: .leading)
            .accessibilityLabel("符号 \(symbol)")
            .accessibilityIdentifier("nineKeySidebarItem_\(index)")
            .designRow()
        }
        .onDelete { draft.remove(at: $0) }
        .onMove { draft.move(from: $0, to: $1) }
      } header: {
        DesignSectionHeader(subject: table.title, info: table.note)
          .accessibilityIdentifier("nineKeySidebarNote")
      } footer: {
        if saveFailed {
          Text(SettingsSaveFailure.message).accessibilityIdentifier("nineKeySidebarSaveFailure")
        }
      }
      Section("手动输入") {
        HStack(spacing: 12) {
          TextField("一个符号，最多 \(NineKeySidebar.maxSymbolCodePoints) 个字符", text: $manual)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .onSubmit(addManual)
            .accessibilityIdentifier("nineKeySidebarManualField")
          Button("加入", action: addManual)
            .buttonStyle(.borderless)
            .foregroundStyle(MetasequoiaTheme.accent)
            .disabled(!draft.canAdd(manual))
            .accessibilityIdentifier("nineKeySidebarManualAdd")
        }
        .designRow()
      }
      if !draft.addable.isEmpty {
        Section("常用符号") {
          NineKeySidebarFlowLayout(spacing: 8) {
            ForEach(draft.addable, id: \.self) { symbol in
              Button { _ = draft.add(symbol) } label: {
                Text(symbol).font(.system(size: 16))
                  .foregroundStyle(Color.primary)
                  .padding(.horizontal, 12)
                  .frame(minWidth: 44, minHeight: 36)
                  .background(MetasequoiaTheme.segBg, in: Capsule())
                  .contentShape(Capsule())
              }
              .buttonStyle(.borderless)
              .disabled(!draft.canAdd(symbol))
              .accessibilityLabel("添加 \(symbol)")
            }
          }
          .padding(.vertical, 8)
          .designRow()
          .accessibilityIdentifier("nineKeySidebarAddable")
        }
      }
      Section {
        Button("恢复默认") { draft.restoreDefaults() }
          .foregroundStyle(MetasequoiaTheme.accent)
          .disabled(draft.isDefault)
          .accessibilityIdentifier("nineKeySidebarRestore")
          .designRow()
      }
    }
    .designPage()
    .environment(\.editMode, .constant(.active))
    .tint(MetasequoiaTheme.accent)
    .navigationTitle(table.title).navigationBarTitleDisplayMode(.inline)
    .toolbar {
      ToolbarItem(placement: .confirmationAction) {
        Button("保存", action: save)
          .disabled(!draft.canSave)
          .accessibilityIdentifier("nineKeySidebarSave")
      }
    }
  }

  private func addManual() {
    guard draft.add(manual) else { return }
    manual = ""
  }

  private func save() {
    guard draft.canSave else { return }
    if TouchKeyLayoutStyle.save(sidebar: draft.symbols, table: table) {
      saveFailed = false
      dismiss()
    } else {
      saveFailed = true
    }
  }
}

/// 常用符号的小胶囊从左到右排、排满换行。
private struct NineKeySidebarFlowLayout: Layout {
  var spacing: CGFloat

  func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
    let rows = arrange(width: proposal.width ?? .infinity, subviews: subviews)
    let height = rows.reduce(0) { $0 + $1.height } + spacing * CGFloat(max(rows.count - 1, 0))
    return CGSize(width: proposal.width ?? rows.map(\.width).max() ?? 0, height: height)
  }

  func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
    var y = bounds.minY
    for row in arrange(width: bounds.width, subviews: subviews) {
      var x = bounds.minX
      for index in row.indices {
        let size = subviews[index].sizeThatFits(.unspecified)
        subviews[index].place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
        x += size.width + spacing
      }
      y += row.height + spacing
    }
  }

  private struct Row {
    var indices: [Int] = []
    var width: CGFloat = 0
    var height: CGFloat = 0
  }

  private func arrange(width: CGFloat, subviews: Subviews) -> [Row] {
    var rows = [Row()]
    for index in subviews.indices {
      let size = subviews[index].sizeThatFits(.unspecified)
      let last = rows[rows.count - 1]
      if !last.indices.isEmpty, last.width + spacing + size.width > width { rows.append(Row()) }
      var row = rows[rows.count - 1]
      row.width = row.indices.isEmpty ? size.width : row.width + spacing + size.width
      row.height = max(row.height, size.height)
      row.indices.append(index)
      rows[rows.count - 1] = row
    }
    return rows
  }
}
