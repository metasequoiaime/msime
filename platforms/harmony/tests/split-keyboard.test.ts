/**
 * 大屏分离式键盘（#10224）：偏好的读法、启用条件（大屏、方向、启用宽度、2in1）和几何（断点、重复的 G/V、空隙份额、拆开的空格）。规则与 Android `SplitKeyboardPolicy`、`SplitKeyboardPolicySmoke` 逐条相同，这里钉住鸿蒙这一份。
 */
import {
  SplitKeyboardPolicy,
  SplitKeyboardSettings,
  SplitRow,
  SplitSpace,
} from "../entry/src/main/ets/keyboard/SplitKeyboardPolicy";
import { KeyboardFormFactorPolicy } from "../entry/src/main/ets/keyboard/KeyboardFormFactorPolicy";
import { SettingsFormFactorCapabilities } from "../entry/src/main/ets/keyboard/settings/SettingsFormFactorCapabilities";

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

function near(actual: number, expected: number, message: string): void {
  check(Math.abs(actual - expected) < 1e-3, `${message}: ${actual} != ${expected}`);
}

const P = SplitKeyboardPolicy;

function settings(overrides: Partial<SplitKeyboardSettings>): SplitKeyboardSettings {
  return { ...P.DEFAULTS, enabled: true, ...overrides };
}

group("取值范围、步进和默认值与 Android 的本机设置和 client-core 的校验相同", () => {
  check(P.MIN_THRESHOLD_VP === 480 && P.MAX_THRESHOLD_VP === 1000, "启用宽度 480–1000");
  check(P.THRESHOLD_STEP_VP === 20 && P.DEFAULT_THRESHOLD_VP === 600, "步进 20，默认 600");
  check(P.MIN_GAP_PERCENT === 10 && P.MAX_GAP_PERCENT === 40, "空隙 10–40%");
  check(P.GAP_PERCENT_STEP === 5 && P.DEFAULT_GAP_PERCENT === 25, "步进 5，默认 25");
  check(P.MAX_KEY_AREA_WIDTH_VP === 720, "两侧键区合计最多 720");
  check(P.MIN_SPACE_SHARE === 0.15, "拆开的空格每一半至少 15%");
  check(
    P.DEFAULT_THRESHOLD_VP === 600 &&
      KeyboardFormFactorPolicy.largeScreen(600, 1000) &&
      !KeyboardFormFactorPolicy.largeScreen(599, 1000),
    "默认启用宽度就是大屏的界线",
  );
  check(!P.DEFAULTS.enabled && !P.DEFAULTS.portrait, "默认关，竖屏选项也关");
});

group("偏好读法：缺键、类型不对、超出范围和不在步进上的值按默认值", () => {
  const defaults: SplitKeyboardSettings = P.settings(undefined, undefined, undefined, undefined);
  check(!defaults.enabled && !defaults.portrait, "缺键按关");
  check(defaults.thresholdVp === 600 && defaults.gapPercent === 25, "缺键按 600 / 25");
  const stored: SplitKeyboardSettings = P.settings(true, true, 720, 35);
  check(
    stored.enabled && stored.portrait && stored.thresholdVp === 720 && stored.gapPercent === 35,
    "合规的值原样读出",
  );
  check(P.settings(true, true, 480, 10).thresholdVp === 480, "下界");
  check(P.settings(true, true, 1000, 40).gapPercent === 40, "上界");
  for (const threshold of [0, 460, 490, 1020, 600.5, "720", null, true]) {
    check(
      P.settings(true, false, threshold, 25).thresholdVp === 600,
      `启用宽度 ${JSON.stringify(threshold)} 按 600`,
    );
  }
  for (const gap of [0, 5, 12, 45, "30", null]) {
    check(P.settings(true, false, 600, gap).gapPercent === 25, `空隙 ${JSON.stringify(gap)} 按 25`);
  }
  check(
    !P.settings("true", 1, 600, 25).enabled && !P.settings("true", 1, 600, 25).portrait,
    "只有布尔 true 才算打开",
  );
});

group("启用条件：开关、大屏横屏、启用宽度；竖屏要另开，手机横屏和 2in1 从不分离", () => {
  const on: SplitKeyboardSettings = settings({});
  check(P.active(on, 1280, 800, false), "平板横屏分离");
  check(!P.active(settings({ enabled: false }), 1280, 800, false), "开关关着不分离");
  check(!P.active(on, 800, 1280, false), "平板竖屏默认不分离");
  check(P.active(settings({ portrait: true }), 800, 1280, false), "竖屏选项打开后竖放也分离");
  check(!P.active(on, 780, 360, false), "手机横屏最短边不到 600，不分离");
  check(!P.active(settings({ portrait: true }), 360, 780, false), "竖屏选项不绕过启用宽度");
  check(!P.active(on, 1280, 800, true), "2in1 不画触摸按键，不分离");
  check(!P.active(on, 0, 0, false), "屏幕尺寸未知时不分离");
  // 启用宽度的边界：最短边正好等于启用宽度时分离，少 1vp 不分离。
  check(P.active(on, 960, 600, false) && !P.active(on, 960, 599, false), "默认 600 的边界");
  const narrow: SplitKeyboardSettings = settings({ thresholdVp: 480 });
  check(P.active(narrow, 853, 480, false) && !P.active(narrow, 853, 479, false), "480 的边界");
  check(!P.active(narrow, 780, 360, false), "调到最低也不在手机横屏上分离");
  const wide: SplitKeyboardSettings = settings({ thresholdVp: 1000 });
  check(!P.active(wide, 1280, 800, false) && P.active(wide, 1600, 1000, false), "1000 的边界");
  check(
    P.drawn(on, 1280, 800, false, true) && !P.drawn(on, 1280, 800, false, false),
    "九键、14 键等不分离的键面不画",
  );
});

group("空隙比例：用户选的比例，宽屏上两侧键区合计不超过 720vp", () => {
  near(P.gapFraction(0, 25), 0.25, "宽度未知");
  near(P.gapFraction(960, 25), 0.25, "960 时 720 正好是 75%");
  near(P.gapFraction(1280, 25), 1 - 720 / 1280, "更宽时多出来的宽度留在中间");
  near(P.gapFraction(800, 40), 0.4, "用户选 40%");
  near(P.gapFraction(800, 10), 0.1, "用户选 10%");
  near(P.gapFraction(800, 50), 0.4, "超出范围按上界");
  near(P.gapWeight(10, 0, 0, 25), 10 / 3, "十个等宽键的行插入后空隙占 25%");
  near(P.gapWeight(10, 1, 0, 25), 10 / 3 - 1, "重复键从空隙借一个键宽");
  near(P.gapWeight(10, 20, 0, 25), 0, "不会是负数");
});

group("断点：第一行五对五，第二行 asdfg | hjkl 并重复 G，第三行 ⇧zxcv | bnm⌫ 并重复 V", () => {
  const first: SplitRow = P.row([1, 1, 1, 1, 1, 1, 1, 1, 1, 1], 0, 3, 0, 25);
  check(first.cut === 5 && !first.duplicate, "qwert | yuiop 不重复");
  near(first.gapShare, 10 / 3, "第一行空隙");
  // 第二行两端各半个键的缩进占位，与 KeyboardView 的 SECOND_ROW_INDENT_SHARE 相同。
  const second: SplitRow = P.row([0.5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0.5], 1, 3, 0, 25);
  check(second.cut === 6 && second.duplicate, "缩进 + asdfg | (g)hjkl + 缩进");
  near(second.gapShare, 10 / 3 - 1, "第二行空隙少一个键宽，键宽与第一行相同");
  // 第三行两端是 1.4 份的 ⇧ 和 ⌫（LETTER_EDGE_SHARE）。
  const third: SplitRow = P.row([1.4, 1, 1, 1, 1, 1, 1, 1, 1.4], 2, 3, 0, 25);
  check(third.cut === 5 && third.duplicate, "⇧zxcv | (v)bnm⌫");
  // 微软双拼第二行显示第十个键 `;`、不缩进：本来五对五，重复 G 会让空隙偏开，不重复。
  const semicolon: SplitRow = P.row([1, 1, 1, 1, 1, 1, 1, 1, 1, 1], 1, 3, 0, 25);
  check(semicolon.cut === 5 && !semicolon.duplicate, "asdfg | hjkl; 不重复");
  // 数字行和 123 / #+= 层只从中间断开。
  const digits: SplitRow = P.row([1, 1, 1, 1, 1, 1, 1, 1, 1, 1], -1, 3, 0, 25);
  check(digits.cut === 5 && !digits.duplicate, "12345 | 67890");
  const layer: SplitRow = P.row([1.4, 1, 1, 1, 1, 1, 1.4], -1, 3, 0, 25);
  check(layer.cut === 4 && !layer.duplicate, "符号层第三行奇数个键，多出的归左边");
  check(P.cutIndex([]) === 0, "空行");
});

group("底行：空格拆成两半，空隙落在整行中间", () => {
  // 设计底行：123 1.25 | 符 1.05 | 空格 4 | ， 1 | 中 1.05 | 回车 1.9。
  const weights: number[] = [1.25, 1.05, 4, 1, 1.05, 1.9];
  const split: SplitSpace = P.space(weights, 2, 0, 25);
  near(split.left, 2.825, "左边的空格");
  near(split.right, 1.175, "右边的空格");
  near(1.25 + 1.05 + split.left, split.right + 1 + 1.05 + 1.9, "两段份额相等");
  near(split.gap, 10.25 / 3, "空隙与字母行同一比例");
  // 一侧的键多得放不下时，空格每一半至少留 15%。
  near(P.leftSpaceWeight([6, 4, 1], 1), 0.6, "左边至少 15%");
  near(P.leftSpaceWeight([1, 4, 6], 1), 3.4, "右边至少 15%");
  check(P.leftSpaceWeight(weights, 9) === 0, "下标越界");
});

group("设置页只在有大屏触屏键盘的设备上列出分离式键盘", () => {
  check(SettingsFormFactorCapabilities.resolve("tablet").splitKeyboard, "平板列出");
  check(!SettingsFormFactorCapabilities.resolve("phone").splitKeyboard, "直板手机不列出");
  check(
    SettingsFormFactorCapabilities.resolve("phone", true).splitKeyboard,
    "可折叠手机展开后是大屏，列出",
  );
  check(
    !SettingsFormFactorCapabilities.resolve("2in1", true).splitKeyboard,
    "2in1 不画触摸按键，不列出",
  );
});

if (failures > 0) {
  console.log(`${failures} split keyboard group(s) failed`);
  throw new Error("split keyboard checks failed");
}
