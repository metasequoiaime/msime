/** 鸿蒙 26 键组字中的上滑、下滑辅助码：门槛、方向、点击先后和长按互斥。 */
import {
  HelpcodeSwipeGesture,
  HelpcodeSwipePolicy,
} from "../entry/src/main/ets/keyboard/input/HelpcodeSwipePolicy";

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

const P = HelpcodeSwipePolicy;
const DOWN_X = 100;
const DOWN_Y = 200;

group("上滑和下滑都能触发，严格大于 14 vp", () => {
  check(P.THRESHOLD_VP === 14, "和字母角标滑动共用 14 vp 门槛");
  check(!P.swiped(DOWN_X, DOWN_Y, DOWN_X, DOWN_Y - 14), "上滑正好 14 不算");
  check(P.swiped(DOWN_X, DOWN_Y, DOWN_X, DOWN_Y - 14.5), "上滑超过 14 算");
  check(!P.swiped(DOWN_X, DOWN_Y, DOWN_X, DOWN_Y + 14), "下滑正好 14 不算");
  check(P.swiped(DOWN_X, DOWN_Y, DOWN_X, DOWN_Y + 14.5), "下滑超过 14 算");
});

group("竖向位移必须大于横向位移，避免抢走滑行输入", () => {
  check(!P.swiped(DOWN_X, DOWN_Y, DOWN_X + 40, DOWN_Y - 20), "横向滑行不算辅助码");
  check(P.swiped(DOWN_X, DOWN_Y, DOWN_X + 10, DOWN_Y - 20), "竖向明显占优算辅助码");
  check(!P.swiped(DOWN_X, DOWN_Y, Number.NaN, DOWN_Y - 20), "无效横坐标不算");
});

group("一次触摸触发一次大写，点击先到也会被吞掉", () => {
  const gesture = new HelpcodeSwipeGesture();
  gesture.down(1, "a", DOWN_X, DOWN_Y, true);
  check(gesture.move(1, DOWN_X, DOWN_Y - 20) === "A", "上滑送 A");
  check(gesture.swallows("a"), "已触发的点按被吞掉");
  check(gesture.up(1) === "A", "抬手仍送 A");
  check(gesture.up(1) === "", "同一次触摸不重复送出");
  gesture.down(1, "b", DOWN_X, DOWN_Y, true);
  check(gesture.move(1, DOWN_X, DOWN_Y + 20) === "B", "下滑送 B");
});

group("未触发时照常点按，关闭或非字母不接管", () => {
  const gesture = new HelpcodeSwipeGesture();
  gesture.down(1, "a", DOWN_X, DOWN_Y, false);
  check(!gesture.tracking(1), "关闭时不跟踪");
  check(gesture.move(1, DOWN_X, DOWN_Y - 40) === "", "关闭时不触发");
  gesture.down(1, ";", DOWN_X, DOWN_Y, true);
  check(!gesture.tracking(1), "分号不接辅助码滑动");
  gesture.down(1, "c", DOWN_X, DOWN_Y, true);
  check(gesture.up(1) === "", "未过门槛抬手不送字母");
  check(!gesture.swallows("c"), "未触发时点按不被吞掉");
});

group("长按先成立后，移动不再抢成辅助码", () => {
  const gesture = new HelpcodeSwipeGesture();
  gesture.down(1, "d", DOWN_X, DOWN_Y, true);
  gesture.hold(1);
  check(gesture.move(1, DOWN_X, DOWN_Y - 40) === "", "长按后的移动不触发");
  check(gesture.up(1) === "", "长按后的抬手不送辅助码");
});

group("大写转换只接受单个字母", () => {
  check(P.letter("e") === "E" && P.letter("Z") === "Z", "字母转大写");
  check(P.letter(";") === "" && P.letter("") === "", "非字母不生成辅助码");
});

if (failures > 0) {
  console.log(`${failures} helpcode swipe group(s) failed`);
  throw new Error("helpcode swipe checks failed");
}
