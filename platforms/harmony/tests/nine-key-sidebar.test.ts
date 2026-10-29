/**
 * 九键左侧符号栏自定义（#10223）：空表回到鸿蒙出厂表、自定义表原样使用、右侧大回车只给出厂表接 !、一屏几项随左列高度变、ForEach 键、偏好读法和整对符号的光标。视图的接线由 run.sh 钉住。
 */
import {
  NineKeyLayout,
  NineKeySidebarItem,
} from "../entry/src/main/ets/keyboard/input/NineKeyLayout";
import { TouchLayoutStylePolicy } from "../entry/src/main/ets/keyboard/TouchLayoutStylePolicy";
import { PairedPunctuationPolicy } from "../entry/src/main/ets/keyboard/input/PairedPunctuationPolicy";

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

function close(actual: number, expected: number): boolean {
  return Math.abs(actual - expected) < 1e-6;
}

group("an empty table keeps the factory sidebars exactly as before", () => {
  check(NineKeyLayout.sideMarks(false, []).join(" ") === ", . ?", "字母层仍是交给 Engine 的 , . ?");
  check(NineKeyLayout.sideMarks(true, []).join(" ") === ", . ? !", "右侧大回车仍在末尾接 !");
  check(NineKeyLayout.sideMarks() === NineKeyLayout.sideMarks(false, []), "不传表等于空表");
  check(
    NineKeyLayout.digitSidebar([]) === NineKeyLayout.digitPunctuation(),
    "数字层仍是原来那张四则运算表",
  );
  check(NineKeyLayout.digitSidebar().join(" ") === "+ - * /", "出厂数字层是 + - * /");
  check(
    NineKeyLayout.strokeSidebar([]) === NineKeyLayout.punctuation(),
    "笔画标点栏仍是原来的四格",
  );
  check(!NineKeyLayout.customSidebar([]), "空表不是自定义");
});

group("a custom table is used as stored, and tall return appends nothing", () => {
  const custom: string[] = ["、", "……", "（）", "#+="];
  check(NineKeyLayout.customSidebar(custom), "非空表是自定义");
  check(NineKeyLayout.sideMarks(false, custom).join(" ") === "、 …… （） #+=", "字母层原样");
  check(NineKeyLayout.sideMarks(true, custom).join(" ") === "、 …… （） #+=", "右侧大回车不接 !");
  check(NineKeyLayout.digitSidebar(custom).join(" ") === "、 …… （） #+=", "数字层原样");
  check(NineKeyLayout.strokeSidebar(custom).join(" ") === "、 …… （） #+=", "笔画原样");
  // 返回副本：视图改不到偏好里那张表，也改不到出厂常量。
  check(NineKeyLayout.sideMarks(false, custom) !== custom, "返回的是副本");
  check(NineKeyLayout.sideMarks(false, ["！"]).join(" ") === "！", "一项的表就是一项");
});

group("visible rows follow the per-platform limit and the 28 vp floor", () => {
  const L: number = NineKeyLayout.LETTER_SIDEBAR_VISIBLE_ROWS;
  const D: number = NineKeyLayout.DIGIT_SIDEBAR_VISIBLE_ROWS;
  check(L === 3 && D === 4, "字母层一屏最多 3 项，数字层 4 项");
  check(NineKeyLayout.SIDEBAR_MIN_ROW_VP === 28, "每项至少 28 vp，与 Android 相同");
  check(NineKeyLayout.sidebarVisibleRows(8, L, 180) === 3, "多于一屏时一屏 3 项");
  check(NineKeyLayout.sidebarVisibleRows(2, L, 180) === 2, "两项时按两项均分");
  check(NineKeyLayout.sidebarVisibleRows(1, L, 180) === 1, "一项占满左列");
  check(NineKeyLayout.sidebarVisibleRows(8, D, 180) === 4, "数字层一屏 4 项");
  // 键盘调到最矮：70 vp 只放得下两项 28 vp 的键。
  check(NineKeyLayout.sidebarVisibleRows(8, L, 70) === 2, "左列矮时少放几项");
  check(NineKeyLayout.sidebarVisibleRows(8, L, 10) === 1, "再矮也至少一项");
  check(
    close(NineKeyLayout.sidebarRowHeight(180, 8, L, 6), (180 - 12) / 3),
    "三项等高，扣掉两道行距",
  );
  check(close(NineKeyLayout.sidebarRowHeight(180, 1, L, 6), 180), "一项占满整列");
  check(close(NineKeyLayout.sidebarRowHeight(180, 2, L, 0), 90), "笔画没有行距");
});

group("the custom number pad column keeps #+= where and as tall as the factory one", () => {
  const height: number = 200;
  const spacing: number = 5;
  const cell: number = NineKeyLayout.digitSidebarCellHeight(height, spacing);
  // 出厂：四个运算符加 #+= 五格等高，四道行距。
  check(close(cell, (200 - 4 * 5) / 5), "一格是五格均分");
  const scroll: number = NineKeyLayout.digitSidebarScrollHeight(height, spacing);
  check(close(scroll, 4 * cell + 3 * spacing), "滚动区是上面四格加三道行距");
  check(close(scroll + spacing + cell, height), "滚动区、一道行距和 #+= 正好铺满左列");
  // 自定义四项时每项与出厂一格同高。
  check(close(NineKeyLayout.sidebarRowHeight(scroll, 4, 4, spacing), cell), "四项时每项与出厂同高");
  check(close(NineKeyLayout.sidebarRowHeight(scroll, 9, 4, spacing), cell), "多于四项时也是这么高");
});

group("sidebar items carry the symbol, index, height and face in their ForEach key", () => {
  const items: NineKeySidebarItem[] = NineKeyLayout.sidebarItems(
    ["、", "、", "；"],
    180,
    3,
    6,
    "face",
  );
  check(items.length === 3, "逐项都画");
  check(
    items.every((item: NineKeySidebarItem) => close(item.height, 56)),
    "每项等高",
  );
  check(
    new Set(items.map((item: NineKeySidebarItem) => item.key)).size === 3,
    "重复的符号不被并掉",
  );
  check(items[0].key.includes(":0:") && items[1].key.includes(":1:"), "键里有下标");
  check(
    items.every((item: NineKeySidebarItem) => item.key.endsWith(":face")),
    "键里有 faceKey",
  );
  const taller: NineKeySidebarItem[] = NineKeyLayout.sidebarItems(
    ["、", "、", "；"],
    240,
    3,
    6,
    "face",
  );
  check(taller[0].key !== items[0].key, "高度变了键跟着变，按值传下去的高度才会刷新");
});

group("the preference reader keeps string items only", () => {
  const read = TouchLayoutStylePolicy.sidebarSymbols;
  check(read(undefined).length === 0, "缺键是空表");
  check(read("， 。").length === 0, "不是数组是空表");
  check(read(["、", 3, null, "", "；"]).join(" ") === "、 ；", "只留非空字符串");
  check(read([]).length === 0, "空数组是空表");
  const letters = TouchLayoutStylePolicy.NINE_KEY_SIDEBAR_DEFAULTS;
  const digits = TouchLayoutStylePolicy.DIGIT_SIDEBAR_DEFAULTS;
  check(read(["，", "。", "？"], letters).length === 0, "等于字母层出厂表按出厂表读");
  check(read(["+", "-", "*", "/"], digits).length === 0, "等于数字层出厂表按出厂表读");
  check(read(["，", "？", "。"], letters).join(" ") === "， ？ 。", "换了顺序是自定义表");
  check(read(["+", "-", "*", "/"], letters).length === 4, "只和自己那张出厂表比");
});

group("a whole pair puts the caret between its halves, as Android's table does", () => {
  const closing = PairedPunctuationPolicy.literalPairClosing;
  check(closing("（）") === "）", "全角括号");
  check(closing("()") === ")", "半角括号");
  check(closing("《》") === "》" && closing("【】") === "】", "书名号、方头括号");
  check(closing("“”") === "”" && closing("‘’") === "’", "弯引号");
  check(closing("{}") === "}" && closing("[]") === "]", "花括号、方括号");
  check(closing('""') === null && closing("''") === null, "ASCII 引号前后同字，不算");
  check(closing("<>") === null, "< 多作小于号，不算");
  check(closing("（") === null && closing("(a)") === null, "半个或带内容的不算");
  check(closing(":-)") === null, "颜文字不算");
  // 三端的表逐项相同由 scripts/test-nine-key-sidebar-parity.py 核对；这里钉住每一对的后半个只占一个 UTF-16 单元，光标退一格就停在中间。
  for (const pair of [
    "（）",
    "《》",
    "〈〉",
    "【】",
    "「」",
    "『』",
    "〔〕",
    "〖〗",
    "＜＞",
    "｛｝",
    "［］",
    "“”",
    "‘’",
    "()",
    "[]",
    "{}",
  ]) {
    const half: string | null = closing(pair);
    check(
      half !== null && half.length === 1 && pair === pair.slice(0, -1) + half,
      `${pair} 两半各一个字，退一格就停在中间`,
    );
  }
});

if (failures > 0) {
  console.log(`${failures} nine-key sidebar group(s) failed`);
  throw new Error("nine-key sidebar checks failed");
}
