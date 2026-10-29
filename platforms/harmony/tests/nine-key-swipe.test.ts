/**
 * 拼音九键滑动输入数字（#10229）：偏好读法、判定的方向与阈值、1 键、一次触摸只判定一次、长按与点击的先后。规则与 Android `NineKeySwipePolicy.gesture`、iOS 的同名判定逐条相同，这里钉住鸿蒙这一份。
 */
import {
  NineKeySwipe,
  NineKeySwipeGesture,
  NineKeySwipePolicy,
} from "../entry/src/main/ets/keyboard/input/NineKeySwipePolicy";
import { NineKeyLayout } from "../entry/src/main/ets/keyboard/input/NineKeyLayout";

let failures = 0;

function group(name: string, body: () => void): void {
  try {
    body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

const P = NineKeySwipePolicy;
const DOWN_Y = 200;

group("偏好的写法与 client-core 相同，缺键和不认识的值按关闭", () => {
  check(P.OFF === "off" && P.UP === "up" && P.DOWN === "down", "线上写法是 off / up / down");
  check(
    P.mode("up") === P.UP && P.mode("down") === P.DOWN && P.mode("off") === P.OFF,
    "认得三种写法",
  );
  for (const value of [undefined, null, "", "UP", "left", true, 1, {}]) {
    check(P.mode(value) === P.OFF, `${JSON.stringify(value)} 按关闭`);
  }
});

group("阈值固定 24 vp，严格大于才算", () => {
  check(P.THRESHOLD_VP === 24, "与 Android 默认的 24 dp、iOS 的 24 pt 相同");
  check(P.gesture(P.UP, DOWN_Y, DOWN_Y - 24, true) === NineKeySwipe.NONE, "上滑正好 24 不算");
  check(P.gesture(P.UP, DOWN_Y, DOWN_Y - 24.5, true) === NineKeySwipe.DIGIT, "上滑 24.5 输入数字");
  check(P.gesture(P.UP, DOWN_Y, DOWN_Y + 24, true) === NineKeySwipe.NONE, "下滑正好 24 不算");
  check(
    P.gesture(P.UP, DOWN_Y, DOWN_Y + 24.5, true) === NineKeySwipe.LETTERS,
    "下滑 24.5 弹出字母",
  );
});

group("上滑设置：上滑出数字、下滑出字母；下滑设置反过来", () => {
  check(P.gesture(P.UP, DOWN_Y, DOWN_Y - 40, true) === NineKeySwipe.DIGIT, "up 时上滑是数字");
  check(P.gesture(P.UP, DOWN_Y, DOWN_Y + 40, true) === NineKeySwipe.LETTERS, "up 时下滑是字母");
  check(P.gesture(P.DOWN, DOWN_Y, DOWN_Y + 40, true) === NineKeySwipe.DIGIT, "down 时下滑是数字");
  check(P.gesture(P.DOWN, DOWN_Y, DOWN_Y - 40, true) === NineKeySwipe.LETTERS, "down 时上滑是字母");
});

group("关闭或不认识的设置没有滑动手势", () => {
  for (const mode of [P.OFF, "left", ""]) {
    check(P.gesture(mode, DOWN_Y, DOWN_Y - 80, true) === NineKeySwipe.NONE, `${mode} 上滑不算`);
    check(P.gesture(mode, DOWN_Y, DOWN_Y + 80, true) === NineKeySwipe.NONE, `${mode} 下滑不算`);
  }
});

group("只看纵向位移，坐标无效时不算", () => {
  // 判定函数只收纵坐标：横着滑得再远也不会成立。
  check(P.gesture(P.UP, DOWN_Y, DOWN_Y, true) === NineKeySwipe.NONE, "原地不动不算");
  check(P.gesture(P.UP, Number.NaN, DOWN_Y - 40, true) === NineKeySwipe.NONE, "按下点无效不算");
  check(
    P.gesture(P.UP, DOWN_Y, Number.POSITIVE_INFINITY, true) === NineKeySwipe.NONE,
    "当前点无效不算",
  );
});

group("1 键沿设定方向输入 1，反方向什么也不做", () => {
  const touchRows = NineKeyLayout.touchRows();
  const one = touchRows[0][0];
  check(NineKeyLayout.showsQuickMarks(one), "触屏网格第一格是 @#");
  check(NineKeyLayout.digitHint(one) === "1", "滑出来的数字是 1");
  const oneHasLetters = NineKeyLayout.holdOptions(one).length > 1;
  check(!oneHasLetters, "1 键长按只有 1，没有字母");
  check(P.gesture(P.UP, DOWN_Y, DOWN_Y - 40, oneHasLetters) === NineKeySwipe.DIGIT, "上滑输入 1");
  check(P.gesture(P.UP, DOWN_Y, DOWN_Y + 40, oneHasLetters) === NineKeySwipe.NONE, "下滑不弹出");
  // 2–9 键滑出来的是键上的数字，反方向弹出和长按相同的选项。
  const expected = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
  const digits = touchRows.flat().map((key) => NineKeyLayout.digitHint(key));
  check(digits.join("") === expected.join(""), `字母层九格的数字是 1–9：${digits.join("")}`);
  for (const key of touchRows.flat().slice(1)) {
    const options = NineKeyLayout.holdOptions(key);
    check(options[0] === key.input && options.length > 1, `${key.label} 弹出 ${options.join(" ")}`);
  }
});

group("一次触摸只判定一次：锁定后滑回原处也不改", () => {
  const gesture = new NineKeySwipeGesture();
  gesture.down("2", 7, DOWN_Y, P.UP, true);
  check(gesture.move("2", 7, DOWN_Y - 10) === NineKeySwipe.NONE, "还没过阈值");
  check(gesture.move("2", 7, DOWN_Y - 30) === NineKeySwipe.DIGIT, "过阈值的那一下报数字");
  check(gesture.move("2", 7, DOWN_Y + 60) === NineKeySwipe.NONE, "锁定后不再报，也不改成字母");
  check(gesture.move("2", 7, DOWN_Y) === NineKeySwipe.NONE, "滑回原处不报");
  check(gesture.swallows("2"), "点击被吞掉");
  check(gesture.up("2", 7) === NineKeySwipe.DIGIT, "抬手仍是数字");
  check(gesture.swallows("2"), "抬手之后到的点击也吞掉");
  check(gesture.up("2", 7) === NineKeySwipe.NONE, "同一次触摸只抬一次手");
});

group("点击先于抬手到达时，抬手照样输入数字", () => {
  const gesture = new NineKeySwipeGesture();
  gesture.down("5", 1, DOWN_Y, P.DOWN, true);
  check(gesture.move("5", 1, DOWN_Y + 30) === NineKeySwipe.DIGIT, "下滑设置下滑出数字");
  check(gesture.swallows("5"), "先到的点击吞掉");
  check(gesture.up("5", 1) === NineKeySwipe.DIGIT, "后到的抬手输入数字");
});

group("没滑出去的触摸照常是点按", () => {
  const gesture = new NineKeySwipeGesture();
  gesture.down("3", 1, DOWN_Y, P.UP, true);
  check(gesture.move("3", 1, DOWN_Y - 24) === NineKeySwipe.NONE, "正好 24 不算");
  check(gesture.up("3", 1) === NineKeySwipe.NONE, "抬手不输入数字");
  check(!gesture.swallows("3"), "点击照常输入");
  // 设置是关闭时同样。
  gesture.down("3", 1, DOWN_Y, P.OFF, true);
  check(gesture.move("3", 1, DOWN_Y - 80) === NineKeySwipe.NONE, "关闭时不判定");
  check(!gesture.swallows("3") && gesture.hold("3"), "关闭时点按、长按照旧");
  // 没按下过的键。
  check(!gesture.swallows("9") && gesture.hold("9"), "没跟踪过的键照常");
});

group("反方向弹出字母：那一下就结束这次触摸，抬手不再输入", () => {
  const gesture = new NineKeySwipeGesture();
  gesture.down("4", 2, DOWN_Y, P.UP, true);
  check(gesture.move("4", 2, DOWN_Y + 30) === NineKeySwipe.LETTERS, "下滑弹出字母");
  check(gesture.move("4", 2, DOWN_Y - 80) === NineKeySwipe.NONE, "之后上滑不再输入数字");
  check(gesture.up("4", 2) === NineKeySwipe.NONE, "抬手什么也不做，弹窗等用户选");
  check(gesture.swallows("4"), "点击吞掉");
  check(!gesture.hold("4"), "长按不再弹一次");
});

group("1 键反方向滑过阈值后抬手照常是点按", () => {
  const gesture = new NineKeySwipeGesture();
  gesture.down("@", 1, DOWN_Y, P.UP, false);
  check(gesture.move("@", 1, DOWN_Y + 60) === NineKeySwipe.NONE, "没有字母可弹");
  check(gesture.up("@", 1) === NineKeySwipe.NONE, "抬手不输入数字");
  check(!gesture.swallows("@"), "点击不吞，由按钮自己的点按规则决定");
});

group("长按与滑动谁先成立谁接管", () => {
  const held = new NineKeySwipeGesture();
  held.down("6", 1, DOWN_Y, P.UP, true);
  check(held.hold("6"), "没滑出去时长按照旧弹出");
  check(held.move("6", 1, DOWN_Y - 80) === NineKeySwipe.NONE, "长按弹出后不再判定滑动");
  check(held.up("6", 1) === NineKeySwipe.NONE, "抬手不输入数字");
  const swiped = new NineKeySwipeGesture();
  swiped.down("6", 1, DOWN_Y, P.UP, true);
  check(swiped.move("6", 1, DOWN_Y - 30) === NineKeySwipe.DIGIT, "先滑出去");
  check(!swiped.hold("6"), "长按到时什么也不做");
  check(swiped.up("6", 1) === NineKeySwipe.DIGIT, "抬手输入数字");
});

group("取消时什么也不输入，随后的点击照样吞掉", () => {
  const gesture = new NineKeySwipeGesture();
  gesture.down("7", 1, DOWN_Y, P.UP, true);
  check(gesture.move("7", 1, DOWN_Y - 30) === NineKeySwipe.DIGIT, "滑出数字");
  check(gesture.cancel("7") === NineKeySwipe.DIGIT, "取消报出原来的判定");
  check(gesture.up("7", 1) === NineKeySwipe.NONE, "取消之后的抬手不输入");
  check(gesture.swallows("7"), "点击吞掉");
  check(gesture.cancel("7") === NineKeySwipe.NONE, "重复取消没有判定");
});

group("判定挂在每个键的那次触摸上，手指之间互不影响", () => {
  const gesture = new NineKeySwipeGesture();
  gesture.down("2", 1, DOWN_Y, P.UP, true);
  gesture.down("8", 2, DOWN_Y, P.UP, true);
  check(gesture.move("2", 1, DOWN_Y - 30) === NineKeySwipe.DIGIT, "一根手指在 2 上滑出数字");
  check(gesture.move("8", 1, DOWN_Y - 30) === NineKeySwipe.NONE, "别的手指的移动不算到 8 上");
  check(!gesture.swallows("8"), "另一根手指在 8 上的点按照常");
  check(gesture.up("8", 2) === NineKeySwipe.NONE, "8 抬手是点按");
  check(gesture.up("2", 2) === NineKeySwipe.NONE, "不是按着 2 的手指，抬手不算");
  check(gesture.up("2", 1) === NineKeySwipe.DIGIT, "按着 2 的手指抬手输入数字");
});

group("新的按下重新开始判定，键被替换后全部丢弃", () => {
  const gesture = new NineKeySwipeGesture();
  gesture.down("9", 1, DOWN_Y, P.UP, true);
  check(gesture.move("9", 1, DOWN_Y - 30) === NineKeySwipe.DIGIT, "滑出数字");
  // 抬手被漏掉（键被重建）之后，下一次按下不受上一次影响。
  gesture.down("9", 3, DOWN_Y, P.UP, true);
  check(!gesture.swallows("9"), "新的按压不吞点击");
  check(gesture.move("9", 1, DOWN_Y - 30) === NineKeySwipe.NONE, "原来那根手指不再算");
  check(gesture.move("9", 3, DOWN_Y + 30) === NineKeySwipe.LETTERS, "新的手指照常判定");
  gesture.clear();
  check(!gesture.swallows("9") && gesture.hold("9"), "丢弃后没有残留");
  // 按下时记下设置，这次触摸按它走完。
  gesture.down("2", 1, DOWN_Y, P.DOWN, true);
  check(gesture.move("2", 1, DOWN_Y + 30) === NineKeySwipe.DIGIT, "按下时是下滑设置");
  gesture.down("2", 1, DOWN_Y, "sideways", true);
  check(gesture.move("2", 1, DOWN_Y + 80) === NineKeySwipe.NONE, "不认识的设置按关闭");
});

if (failures > 0) {
  console.log(`${failures} nine key swipe group(s) failed`);
  throw new Error("nine key swipe checks failed");
}
