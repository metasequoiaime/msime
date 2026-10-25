/// 在展开候选面板里选词之后面板留不留；Android（`CandidatePanelSelectionPolicy.java`）和鸿蒙（`CandidatePanelSelectionPolicy.ts`）按同一条规则判断（#9193）。
///
/// 选词被接受、组字还在（只选掉了前面一段）时面板留着，换上剩下组字的全部候选，用户接着在面板里往下选；整句上屏、组字清空或引擎拒绝了这次选择才收起。剩下的候选只够一页也留着：用户正在面板里逐段选词，收起只会让下一段再点一次展开。
enum CandidatePanelSelectionPolicy {
  /// `editingText` 是选词之后快照里的 `editing_text`。
  static func keepsOpen(handled: Bool, editingText: String) -> Bool {
    handled && !editingText.isEmpty
  }

  /// 部分选词后九键面板要发给 `msime_client_set_nine_key_filter` 的笔画前缀，nil 表示不必发请求。笔画比的是候选的第一个字，那个字刚被选掉，前缀对剩下的组字不再适用，清空；「单字」不动，照旧作用到组字结束，逐字选时不用每段重开。
  static func strokesAfterPartialSelection(_ strokes: String) -> String? {
    strokes.isEmpty ? nil : ""
  }
}
