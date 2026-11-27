import UIKit

/// 候选栏下滑展开：横向候选条本来就竖着滚不动，往下拖一段就展开候选列表，等于点一下右边的展开箭头。
extension KeyboardViewController {
  /// 候选栏下滑展开的手势名，代理在这个名字上判起手方向。
  static let candidateStripSwipeName = "candidateStripSwipeDown"
  /// 要在纵向上拖过多少点。太短会跟「点一下候选」时的手指抖动混淆，太长又要拖很久才响应。
  static let candidateStripSwipeDistance: CGFloat = 18

  /// 挂在候选条上的手势。一旦接手就吃掉这次触摸：手指落在某个候选上往下拖时，不能在展开的同时把那个候选也上屏。
  func makeCandidateStripSwipe() -> UIPanGestureRecognizer {
    let pan = UIPanGestureRecognizer(target: self, action: #selector(handleCandidateStripSwipeDown(_:)))
    pan.name = Self.candidateStripSwipeName
    pan.maximumNumberOfTouches = 1
    pan.cancelsTouchesInView = true
    pan.delegate = self
    return pan
  }

  /// 只接向下：横向是候选列表自己在滚动，向上另有用途；面板打开时它盖在候选条上面，本来也碰不到。
  func candidateStripSwipeShouldBegin(_ pan: UIPanGestureRecognizer) -> Bool {
    let velocity = pan.velocity(in: view)
    return candidatePanel == nil && velocity.x.isFinite && velocity.y.isFinite
      && velocity.y > 0 && abs(velocity.y) > abs(velocity.x)
  }

  /// 方向在手势接手时已经判过（`candidateStripSwipeShouldBegin`），这里只看位移：纵向下滑越过阈值就展开，一次手势只认一次。
  @objc func handleCandidateStripSwipeDown(_ pan: UIPanGestureRecognizer) {
    switch pan.state {
    case .began:
      stripSwipeArmed = true
    case .changed:
      guard stripSwipeArmed else { return }
      let translation = pan.translation(in: view)
      guard translation.y >= Self.candidateStripSwipeDistance,
            abs(translation.y) > abs(translation.x) else { return }
      stripSwipeArmed = false
      guard candidatePanel == nil else { return }
      showCandidatePanel()
    default:
      stripSwipeArmed = false
    }
  }
}
