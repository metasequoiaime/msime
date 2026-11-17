import Foundation

@main
struct BackspacePressTests {
  static func check(_ condition: @autoclosure () -> Bool, _ message: String) {
    precondition(condition(), message)
  }

  static func main() {
    var press = BackspacePressState<Int>()
    var remaining = 200
    for _ in 0..<100 {
      press.begin(1)
      if press.finish(1) == true { remaining -= 1 }
      check(press.finish(1) == nil, "一次松手不能再次删字")
      check(press.nextRepeat() == nil, "松手后的迟到连删不能生效")
    }
    check(remaining == 100, "快速点按只按实际按压次数删除")

    press.begin(1)
    check(press.nextRepeat() == true, "长按第一拍")
    check(press.nextRepeat() == false, "后续连删不是新长按")
    check(press.finish(1) == false, "连删后松手不追加单删")

    press.begin(1)
    check(press.nextRepeat() == true, "滑出前已发生连删")
    check(press.pause(1), "当前按键可以暂停")
    check(press.nextRepeat() == nil, "滑出立即停止连删")
    check(press.finish(1) == false, "滑回松手不能把长按重新当成短按")

    press.begin(1)
    check(press.pause(1), "短按滑出")
    check(press.finish(1) == true, "短按滑回后仍只删一字")

    press.begin(1)
    check(!press.pause(2), "另一按键不能暂停当前按压")
    check(press.finish(2) == nil, "另一按键的迟到松手不能删除")
    check(press.nextRepeat() == true, "当前按压仍然可以连删")
    check(press.finish(1) == false, "当前按压正常结束")

    press.begin(1)
    press.begin(2)
    check(press.finish(1) == nil, "新按压替换旧按压后忽略旧松手")
    check(press.finish(2) == true, "新按压仍正常单删")

    press.begin(1)
    press.cancel()
    check(press.nextRepeat() == nil, "取消后不再连删")
    check(press.finish(1) == nil, "取消后不把迟到松手当成短按")
    press.begin(1)
    check(press.finish(1) == true, "取消不会吞掉下一次正常点按")

    press.begin(1)
    check(press.nextRepeat() == true, "组字边界前发生首拍")
    press.stopRepeating()
    check(press.nextRepeat() == nil, "组字或手写删除结束后不进入正文")
    check(press.finish(1) == false, "停止连删仍保留本次长按身份")

    check(press.finish(1, programmaticActivation: true) == true, "没有触摸的显式程序化激活单删一次")
    check(press.nextRepeat() == nil, "程序化激活不启动连删")
    press.begin(1)
    press.cancel()
    check(press.finish(1) == nil, "真实触摸取消后的迟到松手仍被忽略")
    check(press.finish(1, programmaticActivation: true) == true, "取消不影响下一次程序化激活")
    press.begin(1)
    check(press.nextRepeat() == true, "旧按压发生连删")
    check(press.finish(2, programmaticActivation: true) == true, "另一控件的程序化激活是独立操作")
    check(press.nextRepeat() == nil, "程序化激活终止旧按压")
    check(press.finish(1) == nil, "旧控件迟到松手不追加删除")

    check(BackspaceRepeatTiming.initialDelay < 0.4, "长按首拍应早于旧实现")
    check(BackspaceRepeatTiming.initialDelay > BackspaceRepeatTiming.interval,
          "首拍仍须有明确的长按等待")
    print("iOS 删除按压：9 组行为回归通过")
  }
}
