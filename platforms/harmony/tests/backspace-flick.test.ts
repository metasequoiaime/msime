import {
  BackspaceFlickAction,
  BackspaceFlickGesture,
  BackspaceFlickPolicy,
  BackspaceFlickTextPolicy,
  BackspaceFlickTextSnapshot,
} from "../entry/src/main/ets/keyboard/input/BackspaceFlickPolicy";

declare const process: { exitCode?: number };

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
  if (!condition) throw new Error(message);
}

group("四向映射按上、下、左、右分开", () => {
  check(BackspaceFlickPolicy.action(0, -25) === BackspaceFlickAction.CLEAR, "上滑清空");
  check(BackspaceFlickPolicy.action(0, 25) === BackspaceFlickAction.RESTORE, "下滑恢复");
  check(BackspaceFlickPolicy.action(-25, 0) === BackspaceFlickAction.DELETE_ONE, "左滑删一个");
  check(BackspaceFlickPolicy.action(25, 0) === BackspaceFlickAction.RESTORE_ONE, "右滑恢复一个");
});

group("短距离和斜向移动不误触发", () => {
  check(BackspaceFlickPolicy.action(24, 0) === BackspaceFlickAction.NONE, "阈值严格大于");
  check(BackspaceFlickPolicy.action(10, 20) === BackspaceFlickAction.NONE, "未越过阈值");
  check(BackspaceFlickPolicy.action(30, 30) === BackspaceFlickAction.RESTORE, "垂直轴优先");
});

group("一次触摸锁定首个动作并在抬手后清理", () => {
  const gesture = new BackspaceFlickGesture();
  gesture.down(7, 100, 100);
  check(gesture.move(7, 75, 100) === BackspaceFlickAction.DELETE_ONE, "左滑判定");
  check(gesture.move(7, 100, 70) === BackspaceFlickAction.NONE, "滑回不切换");
  check(gesture.up(7) === BackspaceFlickAction.DELETE_ONE, "抬手交付左滑");
  check(gesture.up(7) === BackspaceFlickAction.NONE, "抬手后没有残留");
});

group("多指和取消不交付动作", () => {
  const gesture = new BackspaceFlickGesture();
  gesture.down(1, 0, 0);
  check(gesture.move(2, 0, -30) === BackspaceFlickAction.NONE, "别的手指不能接管");
  gesture.cancel();
  check(gesture.up(1) === BackspaceFlickAction.NONE, "取消后不执行");
});

group("清空和单字符删除按用户可见字符保存撤回内容", () => {
  const cleared = BackspaceFlickTextPolicy.deleteBeforeCursor("甲乙🙂丙", 3, 3);
  check(cleared.deleted === "甲乙🙂", "上滑清空光标前的全部字符");
  check(cleared.value === "丙" && cleared.cursor === 0, "清空后光标留在原位置");
  const one = BackspaceFlickTextPolicy.deleteBeforeCursor("甲乙🙂丙", 3);
  check(one.deleted === "🙂", "左滑删除一个完整 emoji");
  check(one.value === "甲乙丙" && one.cursor === 2, "单字符删除更新文本和光标");
});

group("撤回只接受原位置、上下文和会话代数都未变化的快照", () => {
  const snapshot: BackspaceFlickTextSnapshot = {
    text: "🙂",
    cursor: 2,
    before: "甲乙",
    after: "丙",
    generation: 4,
  };
  check(BackspaceFlickTextPolicy.canRestore(snapshot, 2, "甲乙", "丙", 4), "原上下文允许恢复");
  check(!BackspaceFlickTextPolicy.canRestore(snapshot, 3, "甲乙", "丙", 4), "光标移动后拒绝恢复");
  check(!BackspaceFlickTextPolicy.canRestore(snapshot, 2, "甲", "丙", 4), "外部编辑后拒绝恢复");
  check(!BackspaceFlickTextPolicy.canRestore(snapshot, 2, "甲乙", "丙", 5), "换编辑框后拒绝恢复");
});

if (failures > 0) process.exitCode = 1;
