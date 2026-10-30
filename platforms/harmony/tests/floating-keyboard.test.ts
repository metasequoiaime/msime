/**
 * 鸿蒙浮动键盘（#10225）：面板大小、位置钳在屏幕里、转屏后同一个千分比仍落在屏幕里，开关与位置存在设备本地的反馈文件里、不进共享结构。几何规则逐条对照 Android 的 `FloatingKeyboardPolicy` 与 `FloatingKeyboardPolicySmoke`。
 */
import { FloatingKeyboardPolicy } from "../entry/src/main/ets/keyboard/FloatingKeyboardPolicy";
import { KeyboardFeedback } from "../entry/src/main/ets/keyboard/KeyboardFeedback";
import { KeyboardFeedbackBridge } from "../entry/src/main/ets/keyboard/KeyboardFeedbackBridge";
import {
  FunctionPanelItem,
  FunctionPanelPolicy,
} from "../entry/src/main/ets/keyboard/FunctionPanelPolicy";
import { KeyboardIconPaths } from "../entry/src/main/ets/keyboard/KeyboardIconPaths";
import { OneHandedMode } from "../entry/src/main/ets/keyboard/OneHandedPolicy";

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

const P = FloatingKeyboardPolicy;

console.log("FloatingKeyboardPolicy");

group("常数与 Android 相同", () => {
  check(P.WIDTH_PERCENT === 80, "宽度是屏幕的 80%");
  check(P.MIN_WIDTH_VP === 240 && P.MAX_WIDTH_VP === 480, "夹在 240–480vp");
  check(P.BAR_HEIGHT_VP === 22 && P.CORNER_RADIUS_VP === 12, "拖动条 22vp，圆角 12vp");
  check(P.MAX_FRACTION === 1000, "位置按千分比存");
  check(
    P.DEFAULT_X_FRACTION === 500 && P.DEFAULT_Y_FRACTION === 1000,
    "第一次浮动水平居中、贴着底部",
  );
  check(
    P.DESCRIPTION === "浮动键盘：按住拖动移动位置" && P.DOCK_DESCRIPTION === "停靠键盘",
    "读屏名称与 Android FloatingKeyboardBar 相同",
  );
});

group("只在手机和平板上生效", () => {
  check(P.active(true, false), "开着且画触屏按键时浮动");
  check(!P.active(false, false), "关着不浮动");
  check(!P.active(true, true), "2in1 的候选窗不浮动");
});

group("宽度：屏幕的 80%，夹在 240–480vp，且不超过屏幕", () => {
  check(P.widthVp(400) === 320, "手机竖屏 400vp 宽时 320vp");
  check(P.widthVp(360) === 288, "360vp 宽时 288vp");
  check(P.widthVp(800) === 480, "手机横屏、平板上不超过 480vp");
  check(P.widthVp(1280) === 480, "大屏同样 480vp");
  check(P.widthVp(280) === 240, "窄屏 80% 不到 240vp 时取 240vp");
  check(P.widthVp(200) === 200, "屏幕比 240vp 还窄时取屏幕宽度");
  check(P.widthVp(0) === 240 && P.widthVp(Number.NaN) === 240, "宽度未知时取最小宽度");
  check(P.widthVp(411.43) === 329, "小数 vp 宽向下取整，与 Android 的整数除法相同");
});

group("高度：停靠时的键盘高度加拖动条，不另外缩放", () => {
  check(P.panelHeightVp(265) === 287, "265vp 的键盘浮动时 287vp");
  check(P.panelHeightVp(0) === 22, "只剩拖动条");
});

group("千分比与像素互换", () => {
  check(P.offset(0, 600) === 0 && P.offset(1000, 600) === 600, "两端是 0 与可移动范围");
  check(P.offset(500, 600) === 300, "500 是正中");
  check(P.offset(1500, 600) === 600 && P.offset(-20, 600) === 0, "范围外先钳回 0–1000");
  check(P.offset(500, 0) === 0 && P.offset(500, -40) === 0, "面板放不下时贴在起点");
  check(P.fractionOf(300, 600) === 500, "fractionOf 是 offset 的反函数");
  check(P.fractionOf(900, 600) === 1000 && P.fractionOf(-1, 600) === 0, "偏移越界时钳回来");
  check(P.fractionOf(10, 0) === 500, "没有可移动范围时按居中");
  check(P.fractionOf(Number.NaN, 600) === 500, "偏移不是有限数时按居中");
  for (const fraction of [0, 1, 137, 500, 999, 1000]) {
    const back: number = P.fractionOf(P.offset(fraction, 2000), 2000);
    check(Math.abs(back - fraction) <= 1, `${fraction} 换成像素再换回来不跑偏（得 ${back}）`);
  }
  check(P.fraction(Number.NaN, 700) === 700, "存的值不是数时取回退值");
  check(P.fraction(432.6) === 433, "存的值取整");
});

group("拖动：起点加位移，钳在屏幕里", () => {
  check(P.dragged(100, 50, 600) === 150, "向右拖 50px");
  check(P.dragged(100, -300, 600) === 0, "拖出左边停在 0");
  check(P.dragged(500, 400, 600) === 600, "拖出右边停在可移动范围的尽头");
  check(P.dragged(100, Number.NaN, 600) === 100, "位移不是有限数时停在起点");
  check(P.dragged(100, 50, -10) === 0, "面板比屏幕还宽时贴在起点");
});

group("位置钳在屏幕里：任何千分比换出来的面板都整个在屏幕上", () => {
  const screen: [number, number] = [1320, 2856];
  const panel: [number, number] = [1056, 800];
  for (const x of [-500, 0, 250, 1000, 5000]) {
    for (const y of [-500, 0, 500, 1000, 5000]) {
      const at: [number, number] = P.position(x, y, screen[0], screen[1], panel[0], panel[1]);
      check(
        at[0] >= 0 && at[1] >= 0 && at[0] + panel[0] <= screen[0] && at[1] + panel[1] <= screen[1],
        `(${x}, ${y}) 落在 (${at[0]}, ${at[1]})，面板出了屏幕`,
      );
    }
  }
  const bottom: [number, number] = P.position(
    P.DEFAULT_X_FRACTION,
    P.DEFAULT_Y_FRACTION,
    screen[0],
    screen[1],
    panel[0],
    panel[1],
  );
  check(bottom[0] === 132 && bottom[1] === 2056, "默认位置水平居中、底边贴着屏幕底边");
});

group("转屏：同一个千分比在新屏幕上换出新位置，仍在屏幕里", () => {
  // 1320×2856 px、密度 3.5 的手机竖屏（模拟器 msime_harmony_7176），转成横屏后宽高互换。
  const density = 3.5;
  const portrait: [number, number] = [1320, 2856];
  const landscape: [number, number] = [2856, 1320];
  const heightVp = 265;
  const size = (screen: [number, number]): [number, number] => [
    Math.round(P.widthVp(screen[0] / density) * density),
    Math.round(P.panelHeightVp(heightVp) * density),
  ];
  const portraitSize: [number, number] = size(portrait);
  const landscapeSize: [number, number] = size(landscape);
  check(
    portraitSize[0] === 1054,
    `竖屏 377vp 宽的 80% 取整为 301vp，即 1053.5px 四舍五入（得 ${portraitSize[0]}px）`,
  );
  check(landscapeSize[0] === 1680, `横屏 816vp 宽夹到 480vp（得 ${landscapeSize[0]}px）`);
  // 竖屏时拖到右下角的四分之三处。
  const dragged: [number, number] = [
    P.dragged(0, 99999, portrait[0] - portraitSize[0]),
    P.dragged(0, (portrait[1] - portraitSize[1]) * 0.75, portrait[1] - portraitSize[1]),
  ];
  const fraction: [number, number] = [
    P.fractionOf(dragged[0], portrait[0] - portraitSize[0]),
    P.fractionOf(dragged[1], portrait[1] - portraitSize[1]),
  ];
  check(fraction[0] === 1000 && fraction[1] === 750, `拖到最右、四分之三高（得 ${fraction}）`);
  const turned: [number, number] = P.position(
    fraction[0],
    fraction[1],
    landscape[0],
    landscape[1],
    landscapeSize[0],
    landscapeSize[1],
  );
  check(
    turned[0] + landscapeSize[0] === landscape[0],
    "横屏上仍贴着右边，没有按竖屏的像素停在中间",
  );
  check(
    turned[1] >= 0 && turned[1] + landscapeSize[1] <= landscape[1],
    `横屏上竖直方向仍在屏幕里（顶边 ${turned[1]}）`,
  );
  check(
    turned[1] === Math.round(((landscape[1] - landscapeSize[1]) * 750) / 1000),
    "竖直方向按横屏的可移动范围取同一个千分比",
  );
  const tooTall: [number, number] = P.position(500, 1000, 2856, 600, 1680, 1004);
  check(tooTall[1] === 0, "横屏上面板比屏幕还高时贴着顶边，不跑到负数");
});

console.log("浮动键盘的存储");

group("开关和位置存在设备本地的反馈文件里，默认关、居中贴底", () => {
  check(
    KeyboardFeedback.DEFAULTS.floating === false &&
      KeyboardFeedback.DEFAULTS.floatingX === 500 &&
      KeyboardFeedback.DEFAULTS.floatingY === 1000,
    "从未浮动过的键盘停靠在底部",
  );
  const old = KeyboardFeedback.parse(
    JSON.stringify({ sound: true, haptics: false, strength: "light" }),
  );
  check(
    old.floating === false && old.floatingX === 500 && old.floatingY === 1000,
    "加入浮动键盘之前写的文件读成默认值",
  );
  const stored = KeyboardFeedback.parse(
    JSON.stringify({
      sound: false,
      haptics: false,
      strength: "medium",
      oneHanded: "left",
      incognito: true,
      floating: true,
      floatingX: 120,
      floatingY: 640,
    }),
  );
  check(
    stored.floating && stored.floatingX === 120 && stored.floatingY === 640,
    "存着的开关和位置读回来",
  );
  const again = KeyboardFeedback.parse(KeyboardFeedback.serialize(stored));
  check(again.floating && again.floatingX === 120 && again.floatingY === 640, "再写一遍还在");
  const wild = KeyboardFeedback.parse(
    JSON.stringify({ floating: "yes", floatingX: 4000, floatingY: "low" }),
  );
  check(
    wild.floating === false && wild.floatingX === 1000 && wild.floatingY === 1000,
    "不是自己的值逐项回退：开关回默认，越界的位置钳回来，不是数的取默认",
  );
});

group("withFloating 只换浮动键盘三项，其余原样", () => {
  const stored = KeyboardFeedback.parse(
    JSON.stringify({
      sound: true,
      haptics: true,
      strength: "heavy",
      oneHanded: "right",
      incognito: true,
    }),
  );
  const on = KeyboardFeedback.withFloating(stored, true, 250, 300);
  check(on.floating && on.floatingX === 250 && on.floatingY === 300, "换上新的开关和位置");
  check(
    on.sound &&
      on.haptics &&
      on.strength === stored.strength &&
      on.oneHanded === OneHandedMode.RIGHT &&
      on.incognito,
    "按键反馈、单手模式和隐私模式不动：浮动时单手模式只是不生效，存着的值不变",
  );
  const clamped = KeyboardFeedback.withFloating(on, true, -10, Number.NaN);
  check(clamped.floatingX === 0 && clamped.floatingY === 300, "位置钳进范围，不是数时保留原值");
});

group("不进共享结构：设置页和账号同步保存时沿用存着的值", () => {
  const stored = KeyboardFeedback.withFloating(KeyboardFeedback.DEFAULTS, true, 100, 200);
  const shared = KeyboardFeedbackBridge.toShared(stored) as unknown as Record<string, unknown>;
  check(
    !("floating" in shared) && !("floatingX" in shared) && !("floatingY" in shared),
    "设置页拿不到它们，与 Android 只在本机、不随账号同步相同",
  );
  const saved = KeyboardFeedbackBridge.fromShared(
    { soundEnabled: true, hapticsEnabled: false, hapticStrength: "light" },
    stored,
  );
  check(
    saved.floating && saved.floatingX === 100 && saved.floatingY === 200 && saved.sound,
    "页面或账号同步写入后浮动键盘仍开着、位置不变",
  );
});

console.log("功能面板");

group("「浮动键盘」磁贴与 Android 同名，是开关，排在鸿蒙额外项的最后", () => {
  const items: FunctionPanelItem[] = FunctionPanelPolicy.items();
  const tile: FunctionPanelItem | undefined = items.find(
    (item: FunctionPanelItem) => item.id === FunctionPanelPolicy.FLOATING,
  );
  check(tile !== undefined, "有这一格");
  if (tile === undefined) {
    return;
  }
  check(tile.label === "浮动键盘" && tile.accessibilityLabel === "浮动键盘", "名称与 Android 相同");
  check(tile.toggle, "是开关，带已开启 / 已关闭");
  check(tile.icon === KeyboardIconPaths.FLOATING, "画浮动键盘图标");
  check(items[items.length - 1] === tile, "排在最后，不挪动已有格子");
  check(
    FunctionPanelPolicy.items(false).some(
      (item: FunctionPanelItem) => item.id === FunctionPanelPolicy.FLOATING,
    ),
    "AI 总闸关着时也在",
  );
});

if (failures > 0) {
  console.log(`${failures} floating keyboard group(s) failed`);
  throw new Error("floating keyboard checks failed");
}
