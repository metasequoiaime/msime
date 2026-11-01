/**
 * 鸿蒙「键盘调整」遮罩：四项的取值和存放、哪几项此刻生效（决定画哪些柄）、外框换算、每个柄只动自己那条边、面板高度含底部抬高。三端按同一份契约实现，见 `.agents/notes/implemented/feature/2026-11-01-harmony-keyboard-adjust-overlay.md`。离开即保存的接线在视图里（ArkTS），由 run.sh 的文本检查钉住。
 */
import {
  AdjustAvailability,
  AdjustFrame,
  KeyboardAdjustPolicy,
} from "../entry/src/main/ets/keyboard/KeyboardAdjustPolicy";
import { KeyboardFeedback } from "../entry/src/main/ets/keyboard/KeyboardFeedback";
import { KeyboardFeedbackBridge } from "../entry/src/main/ets/keyboard/KeyboardFeedbackBridge";
import { KeyboardGeometry } from "../entry/src/main/ets/keyboard/KeyboardGeometry";
import { KeyboardMetrics } from "../entry/src/main/ets/keyboard/KeyboardMetrics";
import {
  FunctionPanelItem,
  FunctionPanelPolicy,
} from "../entry/src/main/ets/keyboard/FunctionPanelPolicy";
import { KeyboardIconPaths } from "../entry/src/main/ets/keyboard/KeyboardIconPaths";

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

function near(a: number, b: number, tolerance: number = 0.25): boolean {
  return Math.abs(a - b) <= tolerance;
}

const P = KeyboardAdjustPolicy;
const ALL: AdjustAvailability = { lift: true, width: true, position: true };
const NONE: AdjustAvailability = { lift: false, width: false, position: false };

/** 外框的左沿和右沿在面板里的 x（vp）。 */
function edges(available: number, widthPercent: number, positionX: number): [number, number] {
  const frame: AdjustFrame = P.frame(
    { height: 0, lift: 0, widthPercent: widthPercent, positionX: positionX },
    ALL,
    available,
  );
  return [frame.left, available - frame.right];
}

console.log("KeyboardAdjustPolicy");

group("范围和缺省与契约相同", () => {
  check(
    P.MIN_LIFT_VP === 0 && P.MAX_LIFT_VP === 160 && P.DEFAULT_LIFT_VP === 0,
    "底部抬高 0–160vp，缺省 0",
  );
  check(
    P.MIN_WIDTH_PERCENT === 60 && P.MAX_WIDTH_PERCENT === 100 && P.DEFAULT_WIDTH_PERCENT === 100,
    "宽度 60–100%，缺省 100",
  );
  check(P.MIN_WIDTH_VP === 240, "拖动时键盘至少 240vp 宽");
  check(P.MAX_POSITION === 1000 && P.DEFAULT_POSITION === 500, "位置是千分比，缺省 500 居中");
  check(
    KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_VP === -12 &&
      KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP === 48,
    "高度沿用共享偏好的 -12..48",
  );
  check(
    P.DEFAULTS.height === 0 &&
      P.DEFAULTS.lift === 0 &&
      P.DEFAULTS.widthPercent === 100 &&
      P.DEFAULTS.positionX === 500,
    "重置回到高度 0、抬高 0、宽度 100%、居中",
  );
  check(P.MASK_COLOR === "#73000000", "遮罩是约 45% 不透明的黑色");
});

group("存着的值取整钳进范围，不是数时用缺省", () => {
  check(P.lift(-5) === 0 && P.lift(400) === 160 && P.lift(12.4) === 12, "抬高钳到 0–160 并取整");
  check(
    P.lift(Number.NaN) === 0 && P.lift(undefined) === 0 && P.lift(null, 30) === 30,
    "抬高不是数时取缺省或给的回退",
  );
  check(
    P.widthPercent(10) === 60 && P.widthPercent(140) === 100 && P.widthPercent(77.6) === 78,
    "宽度钳到 60–100",
  );
  check(P.widthPercent(Number.POSITIVE_INFINITY) === 100, "宽度不是有限数时取 100");
  check(
    P.position(-1) === 0 && P.position(5000) === 1000 && P.position(Number.NaN) === 500,
    "位置钳到 0–1000，不是数时居中",
  );
});

group("哪几项生效，画哪些柄", () => {
  const docked = P.availability(false, false, false, false, false);
  check(docked.lift && docked.width && docked.position, "停靠竖屏：四项都生效");
  const dockedHandles = P.handles(docked);
  check(
    dockedHandles.top &&
      dockedHandles.bottom &&
      dockedHandles.left &&
      dockedHandles.right &&
      dockedHandles.move,
    "四个柄和「移动」都画",
  );
  for (const [floating, landscape, desktop, name] of [
    [true, false, false, "浮动键盘"],
    [false, true, false, "手机横屏"],
    [false, false, true, "2in1"],
  ] as [boolean, boolean, boolean, string][]) {
    const availability = P.availability(floating, landscape, desktop, false, false);
    check(
      !availability.lift && !availability.width && !availability.position,
      `${name}上抬高、宽度、位置都不生效`,
    );
    const handles = P.handles(availability);
    check(
      handles.top && !handles.bottom && !handles.left && !handles.right && !handles.move,
      `${name}上只留上沿，不画「移动」`,
    );
  }
  for (const [oneHanded, split, name] of [
    [true, false, "单手模式"],
    [false, true, "分离式键盘"],
  ] as [boolean, boolean, string][]) {
    const availability = P.availability(false, false, false, oneHanded, split);
    check(
      availability.lift && !availability.width && !availability.position,
      `${name}时宽度和位置不生效，抬高照常`,
    );
    const handles = P.handles(availability);
    check(handles.top && handles.bottom && !handles.left && !handles.right, `${name}时不画左右柄`);
    check(handles.move, `${name}时「移动」还在，只改抬高`);
  }
});

group("宽度下限：60% 与 240vp 取更宽的", () => {
  check(P.minWidthPercent(400) === 60, "400vp 宽时 60% 是 240vp，取 60");
  check(P.minWidthPercent(1000) === 60, "平板上 60%");
  check(P.minWidthPercent(360) === 67, "360vp 宽时 240vp 是 66.7%，向上取到 67");
  check(
    P.minWidthPercent(240) === 100 && P.minWidthPercent(200) === 100,
    "屏幕不到 240vp 时不能收窄",
  );
  check(
    P.minWidthPercent(0) === 100 && P.minWidthPercent(Number.NaN) === 100,
    "宽度未知时不能收窄",
  );
  check(P.drawnWidthPercent(60, 360) === 67, "存着 60% 转到更窄的屏幕上时画成下限");
  check(P.drawnWidthPercent(80, 360) === 80, "高于下限的照存着的画");
});

group("外框：剩余空间按千分比分到两侧，抬高单独算", () => {
  const centred = P.frame({ height: 0, lift: 30, widthPercent: 80, positionX: 500 }, ALL, 400);
  check(
    centred.left === 40 && centred.right === 40 && centred.lift === 30,
    "400vp 宽、80%、居中：两侧各 40，抬高 30",
  );
  const leftmost = P.frame({ height: 0, lift: 0, widthPercent: 80, positionX: 0 }, ALL, 400);
  check(leftmost.left === 0 && leftmost.right === 80, "千分比 0 贴左");
  const rightmost = P.frame({ height: 0, lift: 0, widthPercent: 80, positionX: 1000 }, ALL, 400);
  check(rightmost.left === 80 && rightmost.right === 0, "千分比 1000 贴右");
  const full = P.frame({ height: 0, lift: 0, widthPercent: 100, positionX: 0 }, ALL, 400);
  check(full.left === 0 && full.right === 0, "铺满时位置无所谓");
  const floating = P.frame({ height: 0, lift: 30, widthPercent: 80, positionX: 0 }, NONE, 400);
  check(
    floating.left === 0 && floating.right === 0 && floating.lift === 0,
    "不生效时铺满、不抬高，存着的值不变",
  );
  const narrowOnly = P.frame(
    { height: 0, lift: 0, widthPercent: 80, positionX: 0 },
    { lift: true, width: true, position: false },
    400,
  );
  check(narrowOnly.left === 40 && narrowOnly.right === 40, "位置不生效时居中");
  check(
    P.liftVp(30, ALL) === 30 && P.liftVp(30, NONE) === 0 && P.liftVp(999, ALL) === 160,
    "生效时抬高钳进范围，不生效时为 0",
  );
});

group("上沿：往上拖变高，1vp 对 1vp，下沿不动", () => {
  check(P.dragTop(0, -12) === 12, "往上 12vp，高 12vp");
  check(P.dragTop(0, 20) === -12, "往下拖到最矮的 -12 停住");
  check(P.dragTop(48, -100) === 48, "最高 48");
  check(
    P.dragTop(10, 0.4) === 10 && P.dragTop(10, -0.6) === 11,
    "总位移只取整一次，慢拖不会被抹掉",
  );
  check(P.dragTop(10, Number.NaN) === 10, "位移不是数时不动");
});

group("下沿：先拿高度换抬高、上沿不动，高度到下限后整块继续往上，抬高到 160 停", () => {
  const up = P.dragBottom(0, 0, -10);
  check(up.lift === 10 && up.height === -10, "往上 10vp：抬高 +10、高度 -10，上沿不动");
  const crossing = P.dragBottom(0, 0, -30);
  check(
    crossing.lift === 30 && crossing.height === -12,
    "一次拖动跨过下限：高度停在 -12，抬高照样跟到 30",
  );
  const atFloor = P.dragBottom(-12, 40, -25);
  check(
    atFloor.lift === 65 && atFloor.height === -12,
    "高度已在下限时往上拖，抬高继续加，整块上移",
  );
  const highest = P.dragBottom(48, 150, -50);
  check(highest.lift === 160 && highest.height === 38, "抬高到 160 时停住，高度只让出用掉的那段");
  const ceiling = P.dragBottom(-12, 160, -40);
  check(ceiling.lift === 160 && ceiling.height === -12, "两项都到头时不动");
  const down = P.dragBottom(0, 5, 20);
  check(down.lift === 0 && down.height === 5, "往下拖抬高到 0 停住，高度只补回 5");
  const tallest = P.dragBottom(40, 20, 20);
  check(tallest.lift === 12 && tallest.height === 48, "往下拖高度到 48 停住");
  for (let height = -12; height <= 48; height += 7) {
    for (let lift = 0; lift <= 160; lift += 23) {
      for (const dy of [-500, -37, -1, 0, 3, 41, 500]) {
        const rise = P.dragBottom(height, lift, dy);
        check(
          rise.height >= -12 && rise.height <= 48 && rise.lift >= 0 && rise.lift <= 160,
          `高度 ${height}、抬高 ${lift}、位移 ${dy}：两项都在范围里`,
        );
        if (dy > 0) {
          check(
            rise.height + rise.lift === height + lift,
            `高度 ${height}、抬高 ${lift}、往下 ${dy}：高度加抬高不变，上沿不动`,
          );
          continue;
        }
        const u = -dy;
        check(
          rise.lift === Math.min(lift + u, 160),
          `高度 ${height}、抬高 ${lift}、往上 ${u}：抬高跟着手指，到 160 停`,
        );
        check(
          rise.height === height - Math.min(rise.lift - lift, height + 12),
          `高度 ${height}、抬高 ${lift}、往上 ${u}：高度让出同样多，不低于 -12`,
        );
        check(
          rise.height + rise.lift >= height + lift,
          `高度 ${height}、抬高 ${lift}、往上 ${u}：上沿不会往下掉`,
        );
      }
    }
  }
});

group("左沿：往左拖变宽，右沿不动", () => {
  const narrowed = P.dragLeft(400, 100, 500, 80);
  check(
    narrowed.widthPercent === 80 && narrowed.positionX === 1000,
    "从铺满往右拖 80vp：80%，贴右",
  );
  const back = P.dragLeft(400, 80, 1000, -1000);
  check(back.widthPercent === 100, "往左拖回铺满");
  const atEdge = P.dragLeft(400, 80, 0, -50);
  check(atEdge.widthPercent === 80 && atEdge.positionX === 0, "左沿已在屏幕边时拖不出去");
  const floor = P.dragLeft(400, 100, 500, 1000);
  check(floor.widthPercent === 60, "收到 60% 停住");
  const narrowScreen = P.dragLeft(360, 100, 500, 1000);
  check(narrowScreen.widthPercent === 67, "窄屏收到 240vp 停住");
  const tiny = P.dragLeft(200, 100, 500, 50);
  check(tiny.widthPercent === 100, "屏幕不到 240vp 时不收窄");
});

group("右沿：往右拖变宽，左沿不动", () => {
  const narrowed = P.dragRight(400, 100, 500, -80);
  check(narrowed.widthPercent === 80 && narrowed.positionX === 0, "从铺满往左拖 80vp：80%，贴左");
  const atEdge = P.dragRight(400, 80, 1000, 50);
  check(atEdge.widthPercent === 80 && atEdge.positionX === 1000, "右沿已在屏幕边时拖不出去");
});

group("拖左右沿时对边在屏幕上不动（位置按整数千分比存，误差不超过剩余空间的半个千分比）", () => {
  for (const available of [320, 360, 393, 412, 600, 800, 1280]) {
    // 剩余空间不超过面板宽，半个千分比的取整误差不超过面板宽的 1/2000：手机上 0.2vp，1280vp 的平板上 0.64vp。
    const tolerance = available / 2000 + 1e-6;
    for (const width of [60, 67, 75, 88, 100]) {
      for (const position of [0, 137, 500, 862, 1000]) {
        const [left, right] = edges(available, P.drawnWidthPercent(width, available), position);
        for (const dx of [-300, -41, -7, -1, 0, 2, 13, 77, 300]) {
          const fromLeft = P.dragLeft(available, width, position, dx);
          const leftMoved = edges(
            available,
            P.drawnWidthPercent(fromLeft.widthPercent, available),
            fromLeft.positionX,
          );
          check(
            near(leftMoved[1], right, tolerance),
            `${available}vp、${width}%、${position}‰、左沿 ${dx}：右沿 ${right} 变成 ${leftMoved[1]}`,
          );
          check(leftMoved[0] >= -1e-9, `${available}vp、左沿 ${dx}：左沿不出屏幕`);
          const fromRight = P.dragRight(available, width, position, dx);
          const rightMoved = edges(
            available,
            P.drawnWidthPercent(fromRight.widthPercent, available),
            fromRight.positionX,
          );
          check(
            near(rightMoved[0], left, tolerance),
            `${available}vp、${width}%、${position}‰、右沿 ${dx}：左沿 ${left} 变成 ${rightMoved[0]}`,
          );
          check(rightMoved[1] <= available + 1e-9, `${available}vp、右沿 ${dx}：右沿不出屏幕`);
          check(
            rightMoved[1] - rightMoved[0] >= Math.min(available, P.MIN_WIDTH_VP) - 1e-9,
            `${available}vp、右沿 ${dx}：不窄于 240vp`,
          );
        }
      }
    }
  }
});

group("移动：同时改抬高和水平位置，大小不变", () => {
  check(P.movedLift(10, -30, ALL) === 40, "往上 30vp，抬高 +30");
  check(P.movedLift(10, 30, ALL) === 0, "往下到 0 停住");
  check(P.movedLift(150, -30, ALL) === 160, "往上到 160 停住");
  check(P.movedLift(10, -30, NONE) === 10, "抬高不生效时不变");
  check(P.movedPosition(400, 80, 500, 20, ALL) === 750, "80%、居中时往右 20vp：左边空出 60，750‰");
  check(P.movedPosition(400, 80, 500, 1000, ALL) === 1000, "往右到底");
  check(P.movedPosition(400, 80, 500, -1000, ALL) === 0, "往左到底");
  check(P.movedPosition(400, 100, 300, 50, ALL) === 300, "铺满时位置不变");
  const oneHanded: AdjustAvailability = P.availability(false, false, false, true, false);
  check(
    P.movedPosition(400, 80, 500, 50, oneHanded) === 500 && P.movedLift(0, -20, oneHanded) === 20,
    "单手时「移动」只改抬高",
  );
});

group("读屏名称三端相同，当前值在说明里", () => {
  check(
    P.TOP_LABEL === "键盘高度" &&
      P.BOTTOM_LABEL === "键盘底部高度" &&
      P.LEFT_LABEL === "键盘宽度（左边）" &&
      P.RIGHT_LABEL === "键盘宽度（右边）",
    "四个柄的名称与 Android 相同",
  );
  check(P.heightValue(0) === "100%" && P.heightValue(48) === "127%", "上沿报高度百分比");
  check(P.liftValue(24) === "24vp" && P.liftValue(999) === "160vp", "下沿报抬高");
  check(P.widthValue(85) === "85%", "左右沿报宽度");
  check(
    P.RESET_LABEL === "重置" && P.MOVE_LABEL === "移动" && P.CONFIRM_LABEL === "确认",
    "三个按钮的文字",
  );
  check(
    P.RESET_DESCRIPTION.length > 0 &&
      P.MOVE_DESCRIPTION.length > 0 &&
      P.CONFIRM_DESCRIPTION.length > 0,
    "三个按钮都有说明",
  );
});

console.log("面板高度");

group("totalHeightVp 含底部抬高，2in1 不算", () => {
  const base = KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, false);
  check(
    KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, false, true, 0, 100, 40) === base + 40,
    "抬高 40vp，面板高 40vp",
  );
  check(
    KeyboardMetrics.totalHeightVp(70, 12, 0, 18, 15, false, true, 0, 100, 20) -
      KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, false, true, 0, 100, 32) ===
      0,
    "拖下沿时高度减多少、抬高加多少，面板总高不变",
  );
  check(
    KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, false, true, 0, 100, -5) === base &&
      KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, false, true, 0, 100, Number.NaN) === base,
    "负数或不是数的抬高不算",
  );
  check(
    KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, true, true, 0, 100, 40) ===
      KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, true),
    "2in1 的框架不看抬高",
  );
});

console.log("存放");

group("三项存在本机的按键反馈文件里，缺省与契约相同", () => {
  const defaults = KeyboardFeedback.DEFAULTS;
  check(
    defaults.bottomLift === 0 && defaults.widthPercent === 100 && defaults.positionX === 500,
    "缺省不抬高、铺满、居中",
  );
  const old = KeyboardFeedback.parse(JSON.stringify({ sound: true, floating: true }));
  check(
    old.bottomLift === 0 &&
      old.widthPercent === 100 &&
      old.positionX === 500 &&
      old.sound &&
      old.floating,
    "旧文件没有这三项时取缺省，其余照读",
  );
  const stored = KeyboardFeedback.parse(
    KeyboardFeedback.serialize(KeyboardFeedback.withAdjust(KeyboardFeedback.DEFAULTS, 24, 85, 300)),
  );
  check(
    stored.bottomLift === 24 && stored.widthPercent === 85 && stored.positionX === 300,
    "写下再读回来",
  );
  const wild = KeyboardFeedback.parse(
    JSON.stringify({ bottomLift: 900, widthPercent: "narrow", positionX: -3 }),
  );
  check(
    wild.bottomLift === 160 && wild.widthPercent === 100 && wild.positionX === 0,
    "被改坏的值钳回来或取缺省",
  );
});

group("withAdjust 只换这三项，withFloating 不碰它们", () => {
  const floating = KeyboardFeedback.withFloating(KeyboardFeedback.DEFAULTS, true, 120, 640);
  const adjusted = KeyboardFeedback.withAdjust(floating, 30, 70, 0);
  check(
    adjusted.floating && adjusted.floatingX === 120 && adjusted.floatingY === 640,
    "浮动键盘的开关和位置不变",
  );
  check(
    adjusted.bottomLift === 30 && adjusted.widthPercent === 70 && adjusted.positionX === 0,
    "换上三项",
  );
  const kept = KeyboardFeedback.withAdjust(adjusted, Number.NaN, 30, 2000);
  check(
    kept.bottomLift === 30 && kept.widthPercent === 60 && kept.positionX === 1000,
    "不是数时保留原值，范围外钳回来",
  );
  const moved = KeyboardFeedback.withFloating(adjusted, false, 500, 1000);
  check(
    moved.bottomLift === 30 && moved.widthPercent === 70 && moved.positionX === 0,
    "改浮动键盘时三项不变",
  );
});

group("不进共享结构：设置页和账号同步保存时沿用存着的值", () => {
  const stored = KeyboardFeedback.withAdjust(KeyboardFeedback.DEFAULTS, 40, 75, 250);
  const shared = KeyboardFeedbackBridge.toShared(stored) as unknown as Record<string, unknown>;
  check(
    !("bottomLift" in shared) && !("widthPercent" in shared) && !("positionX" in shared),
    "设置页拿不到它们，也就不随账号同步",
  );
  const saved = KeyboardFeedbackBridge.fromShared(
    { soundEnabled: true, hapticsEnabled: false, hapticStrength: "light" },
    stored,
  );
  check(
    saved.bottomLift === 40 && saved.widthPercent === 75 && saved.positionX === 250 && saved.sound,
    "页面或账号同步写入后三项不变",
  );
});

console.log("功能面板");

group("原来的「键盘高度」磁贴改名「键盘调整」，图标和位置不变", () => {
  const items: FunctionPanelItem[] = FunctionPanelPolicy.items();
  const tile: FunctionPanelItem | undefined = items.find(
    (item: FunctionPanelItem) => item.id === FunctionPanelPolicy.KEYBOARD_HEIGHT,
  );
  check(tile !== undefined, "有这一格");
  if (tile === undefined) {
    return;
  }
  check(tile.label === "键盘调整", "叫「键盘调整」");
  check(tile.icon === KeyboardIconPaths.KEYBOARD_HEIGHT, "图标不变");
  check(!tile.toggle, "是动作，不是开关");
  check(items.indexOf(tile) === 6, "仍排在「词库」后面");
  check(!items.some((item: FunctionPanelItem) => item.label === "键盘高度"), "不再有「键盘高度」");
});

if (failures > 0) {
  console.log(`${failures} keyboard adjust group(s) failed`);
  throw new Error("keyboard adjust checks failed");
}
