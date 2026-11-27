import UIKit

extension KeyboardViewController {
  /// 横滑删除这次删什么：组字时删组码（要能按原样重打回去，见 `BackspaceSwipeDeletion.canRetype`），否则删光标前的文字。手写板上的 ⌫ 撤的是笔迹；韩文、注音、越南文这类就地组字的方案，14/18 键的组码网格、本地模式、已选了一段词的组字，以及笔画和日文，组码重打不回原样，都不接横滑，⌫ 照常点按和长按连删。
  func backspaceSwipeUnit() -> BackspaceSwipeDeletion.Unit? {
    guard handwriting.isHidden else { return nil }
    if hasComposition {
      guard isChineseMode, !composesInPlace, !typesLetterGrid, !isInLocalMode, visiblePhrasePrefix.isEmpty,
            !inputScheme.isStroke, !inputScheme.isJapanese,
            BackspaceSwipeDeletion.canRetype(composition: lastEditingText, caret: lastEditingCaret) else { return nil }
      return .composition(original: lastEditingText)
    }
    // With a selection, the first deleteBackward removes the whole selection rather than the character before it, and swiping back would then insert characters that were never deleted. ⌫ keeps its tap and hold behaviour there.
    guard textDocumentProxy.selectedText?.isEmpty != false,
          let context = textDocumentProxy.documentContextBeforeInput, !context.isEmpty else { return nil }
    return .text(context: context)
  }

  /// 上滑清空能不能开始：手写板上的 ⌫ 撤的是笔迹，不接；有组字时清组字，没有组字时要有可删的正文。有选区时不接——一次 `deleteBackward` 会把整段选区删掉，清空既数不准也收不回来。
  var canClearBackspace: Bool {
    guard handwriting.isHidden else { return false }
    if hasComposition { return true }
    guard textDocumentProxy.selectedText?.isEmpty != false else { return false }
    return !(textDocumentProxy.documentContextBeforeInput ?? "").isEmpty
  }

  /// 展开面板右列那个删除键：与底部 ⌫ 共用同一个手势（`handleBackspaceSwipe`），点按仍走 `handleBackspace()`。滑动期间候选跟着换代、面板留着不收起，与在面板里选候选同一条路（`candidatePanelFollowsSelection`）；上滑清空把组字清掉之后，`refreshCandidatePanelAnnotations` 照常收起面板。
  @objc func handlePanelDeleteSwipe(_ pan: UIPanGestureRecognizer) {
    switch pan.state {
    case .began, .changed: candidatePanelFollowsSelection = true
    default: candidatePanelFollowsSelection = false
    }
    handleBackspaceSwipe(pan)
  }

  /// 按住 ⌫ 滑动：向上越过待命线后松手清空（`BackspaceClearSwipe`）；向左每滑一格（`KeyboardLayoutPreference.backspaceSwipeStep`，设置里的「滑动删除速度」）删一个组码或一个字，不松手往右滑把刚删的按原样恢复。起手方向决定这一次是哪种，中途不切换。
  @objc func handleBackspaceSwipe(_ pan: UIPanGestureRecognizer) {
    switch pan.state {
    case .began:
      let velocity = pan.velocity(in: view)
      if abs(velocity.y) > abs(velocity.x) {
        // 上滑清空：手指越过待命线后松手才清，滑回来就取消。
        guard beginBackspaceClear() else {
          pan.isEnabled = false
          pan.isEnabled = true
          return
        }
        applyBackspaceClear(to: -pan.translation(in: view).y)
        return
      }
      // 不能滑就让这次手势作废。
      guard beginBackspaceSwipe() else {
        pan.isEnabled = false
        pan.isEnabled = true
        return
      }
      applyBackspaceSwipe(to: pan.translation(in: view).x)
    case .changed:
      if backspaceClear != nil {
        applyBackspaceClear(to: -pan.translation(in: view).y)
      } else {
        applyBackspaceSwipe(to: pan.translation(in: view).x)
      }
    default:
      if let clear = backspaceClear {
        // 只有手势正常结束、且此刻仍在待命位才清空；手势被取消或滑回来了都作废。
        backspaceClear = nil
        if pan.state == .ended && clear.armed { clearBackspace() }
        return
      }
      endBackspaceSwipe()
    }
  }

  /// 开始一次上滑清空：取消这次按压，交付挂起的按键（组字可能因此变了），没有可清的就返回 false。
  func beginBackspaceClear() -> Bool {
    cancelBackspacePress()
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    guard canClearBackspace else { return false }
    backspaceClear = BackspaceClearSwipe()
    if hapticsEnabled { hapticStrength.impact(keyFeedback) }
    return true
  }

  /// 上滑过程中更新待命状态；刚越过待命线时补一次触感，与开始滑动时相同。
  func applyBackspaceClear(to upward: CGFloat) {
    guard var clear = backspaceClear else { return }
    let wasArmed = clear.armed
    clear.move(upward: upward)
    backspaceClear = clear
    if clear.armed && !wasArmed, hapticsEnabled {
      hapticStrength.impact(keyFeedback)
    }
  }

  /// 上滑清空：有组字时只清组码和候选（与 Android `ClearTarget.COMPOSITION` 相同），没有组字时清空光标前的文字。
  func clearBackspace() {
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    playInputClick(.backspace)
    pairedPunctuation.clear()
    if hasComposition {
      render(session.cancel())
      return
    }
    clearDocumentBeforeCursor()
    if !isChineseMode { refreshEnglishSuggestions() }
  }

  /// 逐字删掉光标前的全部文字；轮次和每轮字数由 `BackspaceClear` 限制，编辑器不配合时也会停下。
  func clearDocumentBeforeCursor() {
    _ = BackspaceClear.clearBeforeCursor(
      read: { [weak self] in self?.textDocumentProxy.documentContextBeforeInput },
      deleteOne: { [weak self] in
        guard let self else { return }
        self.deleteOwnBackward()
      })
  }

  /// 开始一次横滑：取消这次按压，交付挂起的按键后重新判断删什么（组字可能变了）；没有可删的就返回 false。
  func beginBackspaceSwipe() -> Bool {
    cancelBackspacePress()
    session.flushPendingCharacters()
    flushDeferredInputSnapshot()
    guard let unit = backspaceSwipeUnit() else { return false }
    if case .text = unit {
      pairedPunctuation.clear()
      // Deleting committed text goes straight to the host and never reaches the session, and the echo of that edit skips `textWillChange`'s refocus. Clear the after-commit prediction here the way a tap on ⌫ does, so its chips (and digit picks) do not outlive the text they followed.
      if enginePrediction { render(session.refocus()) }
    }
    backspaceSwipe = BackspaceSwipeDeletion(unit: unit, step: KeyboardLayoutPreference.backspaceSwipeStep)
    if hapticsEnabled { hapticStrength.impact(keyFeedback) }
    return true
  }

  /// 松手（或手势被取消）：删掉的就此定下来。
  func endBackspaceSwipe() {
    if backspaceSwipe != nil && !isChineseMode { refreshEnglishSuggestions() }
    backspaceSwipe = nil
  }

  func applyBackspaceSwipe(to translation: CGFloat) {
    guard var swipe = backspaceSwipe else { return }
    let before = swipe.applied
    let change = swipe.move(to: Double(translation))
    backspaceSwipe = swipe
    guard change != 0 else { return }
    switch swipe.unit {
    case .composition:
      if change > 0 {
        // 一格删一个音节（与长按 ⌫ 连删相同）。一次跨好几格时只在最后画一次，中间不刷新候选，滑得快也不卡。
        session.flushPendingCharacters()
        flushDeferredInputSnapshot()
        var current = lastEditingText
        var last: MetasequoiaInputSnapshot?
        for index in 0..<change {
          playInputClick(.backspace)
          let snapshot = session.segmentBackspace()
          swipe.recordRemoved(before: current, after: snapshot.editingText)
          current = snapshot.editingText
          last = snapshot
          if current.isEmpty {
            swipe.stop(at: before + index + 1)
            break
          }
        }
        backspaceSwipe = swipe
        if let last { render(last) }
      } else if swipe.restorable {
        // 按原顺序重打删掉的音节，组字连同候选回到删之前的样子。
        for _ in 0..<(-change) {
          guard let code = backspaceSwipe?.popRemovedCode() else { break }
          for character in code { handleCharacter(String(character)) }
        }
      }
    case .text:
      if change > 0 {
        for _ in 0..<change {
          playInputClick(.backspace)
          deleteOwnBackward()
        }
      } else {
        // 离光标最远的那个最后删，所以最先插回。
        for index in stride(from: before - 1, through: swipe.applied, by: -1) {
          guard let text = swipe.deletedText(at: index) else { continue }
          playInputClick()
          textDocumentProxy.insertText(text)
          noteOwnEdit()
        }
      }
    }
  }
}
