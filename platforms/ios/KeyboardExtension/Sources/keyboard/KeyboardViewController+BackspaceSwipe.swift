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
    guard let context = textDocumentProxy.documentContextBeforeInput, !context.isEmpty else { return nil }
    return .text(context: context)
  }

  /// 按住 ⌫ 横滑：往左每滑一格（`KeyboardLayoutPreference.backspaceSwipeStep`，设置里的「滑动删除速度」）删一个组码或一个字，不松手往右滑把刚删的按原样恢复，松手定下来。
  @objc func handleBackspaceSwipe(_ pan: UIPanGestureRecognizer) {
    switch pan.state {
    case .began:
      cancelBackspacePress()
      session.flushPendingCharacters()
      flushDeferredInputSnapshot()
      // 交付挂起的按键之后组字可能变了，重新判断一次；不能滑就让这次手势作废。
      guard let unit = backspaceSwipeUnit() else {
        pan.isEnabled = false
        pan.isEnabled = true
        return
      }
      if case .text = unit { pairedPunctuation.clear() }
      backspaceSwipe = BackspaceSwipeDeletion(unit: unit, step: KeyboardLayoutPreference.backspaceSwipeStep)
      if hapticsEnabled { hapticStrength.impact(keyFeedback) }
      applyBackspaceSwipe(to: pan.translation(in: view).x)
    case .changed:
      applyBackspaceSwipe(to: pan.translation(in: view).x)
    default:
      if backspaceSwipe != nil && !isChineseMode { refreshEnglishSuggestions() }
      backspaceSwipe = nil
    }
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
