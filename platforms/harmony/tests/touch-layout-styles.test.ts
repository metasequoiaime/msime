/**
 * 触屏按键布局样式与顶部数字行（笔记 2026-10-27-touch-key-layout-styles.md）：每种样式的键序与份额、默认行与加样式之前逐键相同、偏好读法、数字行门槛和补充行。三端画同一组排法，这里钉住鸿蒙这一份。
 */
import {
  CommaHold,
  CommaSwipeGesture,
  DesignRowEntry,
  DesignRowSlot,
  NumberRowFace,
  StyledFace,
  TouchHeightRelay,
  TouchLayoutStylePolicy,
  TouchPanelShape,
} from "../entry/src/main/ets/keyboard/TouchLayoutStylePolicy";
import {
  NineKeyAction,
  NineKeyActionKind,
  NineKeyLayout,
} from "../entry/src/main/ets/keyboard/input/NineKeyLayout";
import { KeyboardMetrics } from "../entry/src/main/ets/keyboard/KeyboardMetrics";

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

const P = TouchLayoutStylePolicy;

/** 一行写成 `slot:份额` 的串，份额四舍五入到两位，避免浮点尾数。 */
function describe(row: readonly DesignRowEntry[]): string {
  return row.map((item) => `${item.slot}:${Math.round(item.share * 100) / 100}`).join(" ");
}

function total(row: readonly { share?: number; weight?: number }[]): number {
  return row.reduce((sum, item) => sum + (item.share ?? item.weight ?? 0), 0);
}

function nine(row: readonly NineKeyAction[]): string {
  return row.map((item) => `${item.kind}:${Math.round(item.weight * 100) / 100}`).join(" ");
}

const close = (left: number, right: number) => Math.abs(left - right) < 1e-9;

const letters: NumberRowFace = {
  kanaGrid: false,
  handwriting: false,
  nineKeyGrid: false,
  zhuyin: false,
  stroke: false,
};

console.log("TouchLayoutStylePolicy");

group("偏好读法：认识的样式原样保留，缺键、非字符串和不认识的值按 standard", () => {
  for (const style of ["standard", "comma_period", "long_space", "emoji"]) {
    check(P.twentySixKeyStyle(style) === style, `26 键 ${style}`);
  }
  for (const style of ["standard", "long_space", "emoji"]) {
    check(P.fourteenKeyStyle(style) === style, `14 键 ${style}`);
  }
  for (const style of ["standard", "comma_period", "emoji", "tall_return"]) {
    check(P.nineKeyStyle(style) === style, `9 键 ${style}`);
  }
  for (const value of [undefined, null, 1, true, "", "split", "LONG_SPACE"]) {
    check(P.twentySixKeyStyle(value) === "standard", `26 键 ${String(value)} 按 standard`);
    check(P.fourteenKeyStyle(value) === "standard", `14 键 ${String(value)} 按 standard`);
    check(P.nineKeyStyle(value) === "standard", `9 键 ${String(value)} 按 standard`);
  }
  // 每种键面类型只认它真能画出来的取值：14 键没有 comma_period，九键没有 long_space。
  check(P.fourteenKeyStyle("comma_period") === "standard", "14 键不认 comma_period");
  check(P.nineKeyStyle("long_space") === "standard", "9 键不认 long_space");
  check(P.twentySixKeyStyle("tall_return") === "standard", "26 键不认 tall_return");
  check(P.fourteenKeyStyle("tall_return") === "standard", "14 键不认 tall_return");
});

group("默认行与加样式之前逐键相同", () => {
  check(
    describe(P.designRow(true, false, StyledFace.NONE, "standard")) ===
      "layer:1.25 symbol_panel:1.05 space:4 comma:1 language:1.05 return:1.9",
    "26 键一族：123 | 符 | 空格 | ， | 中 | 回车",
  );
  check(
    describe(P.designRow(true, false, StyledFace.TWENTY_SIX, "standard")) ===
      describe(P.designRow(true, false, StyledFace.NONE, "standard")),
    "中文 26 键的 standard 就是原来的 26 键底行",
  );
  check(
    describe(P.designRow(true, true, StyledFace.NONE, "standard")) ===
      "layer:1.25 symbol_panel:1.05 period:1 space:3 comma:1 language:1.05 return:1.9",
    "英文 26 键：句点紧挨空格左边，空格让到 3",
  );
  check(
    describe(P.designRow(false, false, StyledFace.FOURTEEN, "standard")) ===
      "layer:1.25 comma:1 space:4 period:1 language:1.05 return:1.9",
    "14/18 键：123 | ， | 空格 | 。 | 中 | 回车",
  );
  check(
    describe(P.designRow(false, false, StyledFace.NONE, "standard")) ===
      describe(P.designRow(false, false, StyledFace.FOURTEEN, "standard")),
    "大千注音与 14 键 standard 同一行",
  );
  check(close(P.ENGLISH_SPACE_SHARE, 3), "ENGLISH_SPACE_SHARE 仍是 3");
});

group("中文 26 键的三种样式", () => {
  const standard = P.designRow(true, false, StyledFace.TWENTY_SIX, "standard");
  const commaPeriod = P.designRow(true, false, StyledFace.TWENTY_SIX, "comma_period");
  check(
    describe(commaPeriod) ===
      "layer:1.25 symbol_panel:1.05 comma:1 space:3 period:1 language:1.05 return:1.9",
    `逗号句号在空格两侧：${describe(commaPeriod)}`,
  );
  const longSpace = P.designRow(true, false, StyledFace.TWENTY_SIX, "long_space");
  check(
    describe(longSpace) === "layer:1.25 comma:1 space:5.05 language:1.05 return:1.9",
    `长空格：${describe(longSpace)}`,
  );
  const emoji = P.designRow(true, false, StyledFace.TWENTY_SIX, "emoji");
  check(
    describe(emoji) === "layer:1.25 emoji:1.05 space:4 comma:1 language:1.05 return:1.9",
    `表情键占 符 的位置和宽度：${describe(emoji)}`,
  );
  for (const row of [commaPeriod, longSpace, emoji]) {
    check(close(total(row), total(standard)), `整行份额不变：${describe(row)}`);
    check(P.languageBesideReturn(row), "中/英 紧挨回车");
  }
  // 中文 26 键的样式不看英文参数：英文 26 键由视图按 StyledFace.NONE 交来。
  check(
    describe(P.designRow(true, true, StyledFace.TWENTY_SIX, "emoji")) === describe(emoji),
    "样式行不加英文句点",
  );
});

group("14 键与 18 键的两种样式", () => {
  const standard = P.designRow(false, false, StyledFace.FOURTEEN, "standard");
  const longSpace = P.designRow(false, false, StyledFace.FOURTEEN, "long_space");
  check(
    describe(longSpace) === "layer:1.25 comma:1 space:5 language:1.05 return:1.9",
    `长空格：。 的一份给空格：${describe(longSpace)}`,
  );
  const emoji = P.designRow(false, false, StyledFace.FOURTEEN, "emoji");
  check(
    describe(emoji) === "layer:1.25 emoji:1 space:4 comma:1 language:1.05 return:1.9",
    `表情键：123 | 😀 | 空格 | ， ，😀 是原 ， 的一份、， 是原 。 的一份：${describe(emoji)}`,
  );
  check(
    close(total(longSpace), total(standard)) && close(total(emoji), total(standard)),
    "整行份额不变",
  );
  // 14 键不认 comma_period，读进来就是 standard；直接交来也画 standard。
  check(
    describe(P.designRow(false, false, StyledFace.FOURTEEN, "comma_period")) === describe(standard),
    "14 键上的 comma_period 画 standard",
  );
});

group("逗号的长按与下滑", () => {
  check(P.commaHold(true, StyledFace.NONE, "standard") === CommaHold.PERIOD, "26 键一族长按句号");
  check(P.commaHold(false, StyledFace.NONE, "standard") === CommaHold.MENU, "大千注音长按常用标点");
  check(
    P.commaHold(true, StyledFace.TWENTY_SIX, "standard") === CommaHold.PERIOD,
    "中文 26 键标准",
  );
  check(
    P.commaHold(true, StyledFace.TWENTY_SIX, "long_space") === CommaHold.PERIOD,
    "长空格仍长按句号",
  );
  check(
    P.commaHold(true, StyledFace.TWENTY_SIX, "emoji") === CommaHold.PERIOD,
    "表情键样式长按句号",
  );
  check(
    P.commaHold(true, StyledFace.TWENTY_SIX, "comma_period") === CommaHold.MENU,
    "有独立句号时逗号长按打开常用标点",
  );
  for (const style of ["standard", "long_space", "emoji"]) {
    check(
      P.commaHold(false, StyledFace.FOURTEEN, style) === CommaHold.MENU,
      `14 键 ${style} 长按常用标点`,
    );
  }
  check(P.commaSwipesPeriod(StyledFace.TWENTY_SIX, "long_space"), "26 键长空格下滑句号");
  check(P.commaSwipesPeriod(StyledFace.FOURTEEN, "long_space"), "14 键长空格下滑句号");
  for (const style of ["standard", "comma_period", "emoji"]) {
    check(!P.commaSwipesPeriod(StyledFace.TWENTY_SIX, style), `26 键 ${style} 不接下滑`);
  }
  check(!P.commaSwipesPeriod(StyledFace.NONE, "long_space"), "不受样式影响的底行不接下滑");
  check(P.longSpaceActive(true, "long_space", false), "中文 26 键长空格");
  check(!P.longSpaceActive(false, "long_space", false), "英文 26 键不随长空格变化");
  check(!P.longSpaceActive(true, "emoji", false), "其他样式不是长空格");
  // 与 Android 的 longSpaceLayoutActive 一样排除密码框和只收数字的编辑框：用户在密码框里切到中文时画标准底行。
  check(!P.longSpaceActive(true, "long_space", true), "密码框、数字框里长空格不生效");
});

group(
  "密码框和只收数字的编辑框：三种键面类型的每种样式都退回标准底行（Android `bottomRowStyle`）",
  () => {
    for (const style of ["standard", "comma_period", "long_space", "emoji", "tall_return"]) {
      check(P.fieldRowStyle(style, true) === "standard", `${style} 退回标准`);
      check(P.fieldRowStyle(style, false) === style, `普通编辑框保留 ${style}`);
    }
    for (const style of ["comma_period", "long_space", "emoji"]) {
      check(
        describe(P.designRow(true, false, StyledFace.TWENTY_SIX, P.fieldRowStyle(style, true))) ===
          describe(P.designRow(true, false, StyledFace.TWENTY_SIX, "standard")),
        `中文 26 键 ${style} 在密码框里与标准底行逐键相同`,
      );
    }
    for (const style of ["long_space", "emoji"]) {
      check(
        describe(P.designRow(false, false, StyledFace.FOURTEEN, P.fieldRowStyle(style, true))) ===
          describe(P.designRow(false, false, StyledFace.FOURTEEN, "standard")),
        `14 键 ${style} 在密码框里与标准底行逐键相同`,
      );
    }
    // 九键的每种样式在普通编辑框里都确实换了外框（底行或大回车三列），所以退回标准是看得见的变化；视图经 `nineKeyFrameStyle` 用 `fieldRowStyle` 的接线由 run.sh 钉住。
    for (const style of ["comma_period", "emoji", "tall_return"]) {
      check(
        nine(NineKeyLayout.actionRow(false, false, style)) !==
          nine(NineKeyLayout.actionRow(false)) || NineKeyLayout.tallReturn(false, style),
        `九键 ${style} 在普通编辑框里换了外框`,
      );
    }
  },
);

group("逗号下滑手势：向下过门槛且竖向大于横向才算，点按随之被吞一次", () => {
  const gesture = new CommaSwipeGesture();
  gesture.down(100, 100);
  check(gesture.up(102, 100 + CommaSwipeGesture.THRESHOLD_VP + 1), "向下滑过门槛");
  check(gesture.swallows(), "这次点按被吞");
  check(!gesture.swallows(), "只吞一次");
  gesture.down(100, 100);
  check(!gesture.up(100, 100 + CommaSwipeGesture.THRESHOLD_VP), "正好在门槛上不算");
  check(!gesture.swallows(), "普通点按照常输入逗号");
  gesture.down(100, 100);
  check(!gesture.up(140, 120), "横向更多不算");
  gesture.down(100, 100);
  check(!gesture.up(100, 60), "向上不算");
  gesture.down(100, 100);
  gesture.cancel();
  check(!gesture.up(100, 140), "长按识别后抬起不再按下滑处理");
  gesture.down(100, 100);
  gesture.up(100, 140);
  gesture.down(100, 100);
  check(!gesture.swallows(), "点按没有送来时，下一次按下清掉吞键");
});

group("九键样式：符 | 123 | ， | 空格 | 。 与 符 | 123 | 😀 | 空格，数字层和英文九键不变", () => {
  const standard = NineKeyLayout.actionRow(false, false, "standard");
  check(nine(standard) === nine(NineKeyLayout.actionRow(false)), "standard 就是原来的九键底行");
  check(
    nine(standard) === "symbol_panel:1.05 layer_toggle:1.25 space:4.2 language:1.05 return:1.6",
    nine(standard),
  );
  const commaPeriod = NineKeyLayout.actionRow(false, false, "comma_period");
  check(
    nine(commaPeriod) ===
      "symbol_panel:1.05 layer_toggle:1.25 comma:1 space:2.2 period:1 language:1.05 return:1.6",
    `底栏带标点：${nine(commaPeriod)}`,
  );
  const emoji = NineKeyLayout.actionRow(false, false, "emoji");
  check(
    nine(emoji) ===
      "symbol_panel:1.05 layer_toggle:1.25 emoji:1.05 space:3.15 language:1.05 return:1.6",
    `表情键：${nine(emoji)}`,
  );
  check(
    close(total(commaPeriod), total(standard)) && close(total(emoji), total(standard)),
    "整行份额不变",
  );
  for (const style of ["comma_period", "emoji"]) {
    check(
      nine(NineKeyLayout.actionRow(true, false, style)) === nine(NineKeyLayout.actionRow(true)),
      `数字层不受 ${style} 影响`,
    );
    check(
      nine(NineKeyLayout.actionRow(false, true, style)) ===
        nine(NineKeyLayout.actionRow(false, true)),
      `英文九键不受 ${style} 影响`,
    );
  }
  check(
    emoji[emoji.length - 2].kind === NineKeyActionKind.LANGUAGE &&
      commaPeriod[commaPeriod.length - 2].kind === NineKeyActionKind.LANGUAGE,
    "中/英 紧挨回车",
  );
});

group("九键「右侧大回车」：网格下方的底行恰好与网格同宽，回车不在这一行", () => {
  check(NineKeyLayout.tallReturn(false, "tall_return"), "中文九键");
  check(!NineKeyLayout.tallReturn(true, "tall_return"), "英文九键不受影响");
  for (const style of ["standard", "comma_period", "emoji"]) {
    check(!NineKeyLayout.tallReturn(false, style), `${style} 不是大回车`);
  }
  const letters = NineKeyLayout.actionRow(false, false, "tall_return");
  check(
    nine(letters) === "layer_toggle:1.25 space:3.7 language:1.05",
    `字母层：123 | 空格 | 中/英：${nine(letters)}`,
  );
  const digits = NineKeyLayout.actionRow(true, false, "tall_return");
  check(
    nine(digits) === "layer_toggle:1.25 space:2.65 zero:1.05 language:1.05",
    `数字层：切换键 | 空格 | 0 | 中/英，0 在空格的位置、空格留在 0 左边：${nine(digits)}`,
  );
  for (const row of [letters, digits]) {
    check(close(total(row), NineKeyLayout.GRID_SHARE), `底行恰好与网格同宽：${total(row)}`);
    check(
      !row.some(
        (item) =>
          item.kind === NineKeyActionKind.SYMBOL_PANEL || item.kind === NineKeyActionKind.RETURN,
      ),
      "符 和回车由外框画在左列和右列",
    );
    check(!NineKeyLayout.languageBesideReturn(row), "中/英 与回车不在同一行，不让出回车键帽");
  }
  check(
    close(NineKeyLayout.SIDE_SHARE * 2 + NineKeyLayout.GRID_SHARE, 8.3),
    "左列、网格、右列与普通网格同宽：1.15 | 6 | 1.15",
  );
  check(
    nine(NineKeyLayout.actionRow(false, true, "tall_return")) ===
      nine(NineKeyLayout.actionRow(false, true)),
    "英文九键字母层不变",
  );
  check(
    nine(NineKeyLayout.actionRow(true, true, "tall_return")) ===
      nine(NineKeyLayout.actionRow(true, true)),
    "英文九键数字层不变",
  );
  // ！ 从右列让到左列末尾；其他样式的左列不变。
  check(NineKeyLayout.sideMarks(true).join(" ") === ", . ? !", "左列末尾多一个 !");
  check(NineKeyLayout.sideMarks().join(" ") === ", . ?", "其他样式左列仍是 , . ?");
  check(NineKeyLayout.sideMarks(false).join(" ") === ", . ?", "显式 false 同上");
  // 默认和其他样式的整行仍以中/英紧挨回车结尾，保留回车让位。
  for (const style of ["standard", "comma_period", "emoji"]) {
    check(
      NineKeyLayout.languageBesideReturn(NineKeyLayout.actionRow(false, false, style)),
      `${style} 中/英 紧挨回车`,
    );
  }
  check(NineKeyLayout.languageBesideReturn(NineKeyLayout.actionRow(true)), "数字层中/英 紧挨回车");
});

group("九键「底栏带标点」的逗号长按打开常用标点，与 Android、iOS 相同", () => {
  check(NineKeyLayout.commaOpensQuickPunctuation(false, false, "comma_period"), "中文九键字母层");
  check(
    !NineKeyLayout.commaOpensQuickPunctuation(false, true, "comma_period"),
    "英文九键的逗号不接长按",
  );
  check(
    !NineKeyLayout.commaOpensQuickPunctuation(true, false, "comma_period"),
    "数字层没有这个逗号",
  );
  for (const style of ["standard", "emoji", "tall_return"]) {
    check(!NineKeyLayout.commaOpensQuickPunctuation(false, false, style), `${style} 没有这个逗号`);
  }
});

group("扩展能力按上次记下的键面定第一帧：数字行和长空格样式的工具条一开始就算进去", () => {
  const twentySix = { lettersRow: true, chineseTwentySix: true };
  const nineGrid = { lettersRow: false, chineseTwentySix: false };
  const english = { lettersRow: true, chineseTwentySix: false };
  const shape = (value: { strip: boolean; extraRows: number }) =>
    `${value.strip}:${value.extraRows}`;
  // 没记过：与加样式之前相同，工具条只看工具栏设置，不多行。
  check(
    shape(P.firstPanel(null, true, "long_space", false, false)) === "true:0",
    "没记过，工具栏开着",
  );
  check(
    shape(P.firstPanel(null, true, "long_space", true, false)) === "false:0",
    "没记过，工具栏隐藏",
  );
  check(
    shape(P.firstPanel(twentySix, true, "standard", false, false)) === "true:1",
    "26 键开着数字行多一行",
  );
  check(
    shape(P.firstPanel(twentySix, false, "standard", false, false)) === "true:0",
    "数字行关着不多行",
  );
  check(
    shape(P.firstPanel(twentySix, true, "standard", false, true)) === "true:0",
    "手机横屏不多行",
  );
  check(
    shape(P.firstPanel(nineGrid, true, "standard", false, false)) === "true:0",
    "九键不画数字行",
  );
  check(
    shape(P.firstPanel(english, true, "long_space", true, false)) === "false:1",
    "英文 26 键有数字行、不算长空格",
  );
  check(
    shape(P.firstPanel(twentySix, false, "long_space", true, false)) === "true:0",
    "中文 26 键长空格：工具栏设成隐藏也画工具条",
  );
  check(
    shape(P.firstPanel(twentySix, false, "emoji", true, false)) === "false:0",
    "其他样式照工具栏设置",
  );
  // 与视图出现后算的一致：同样的键面和偏好，第一帧的多出行数就是 numberRowExtraRows 的结果。
  check(
    P.firstPanel(twentySix, true, "standard", false, false).extraRows ===
      P.numberRowExtraRows(true, letters, false),
    "多出的行与视图的数字行门槛一致",
  );
  const text = P.touchPanelFaceText(twentySix);
  check(text === '{"letters_row":true,"chinese_twenty_six":true}', text);
  const parsed = P.parseTouchPanelFace(text);
  check(parsed !== null && parsed.lettersRow && parsed.chineseTwentySix, "写下的能读回来");
  const back = P.parseTouchPanelFace(P.touchPanelFaceText(nineGrid));
  check(back !== null && !back.lettersRow && !back.chineseTwentySix, "两个 false 也读得回来");
  for (const broken of [
    "",
    "{",
    "[]",
    "null",
    "1",
    '{"letters_row":1,"chinese_twenty_six":true}',
    '{"letters_row":true}',
  ]) {
    check(P.parseTouchPanelFace(broken) === null, `读不懂的 ${broken} 按没记过`);
  }
});

group("数字行门槛：26/14/18 键一类、开关打开、不是手机横屏", () => {
  check(P.numberRowExtraRows(true, letters, false) === 1, "竖屏多一行");
  check(P.numberRowExtraRows(false, letters, false) === 0, "开关关着不画");
  check(P.numberRowExtraRows(true, letters, true) === 0, "手机横屏不画");
  for (const key of ["kanaGrid", "handwriting", "nineKeyGrid", "zhuyin", "stroke"] as const) {
    const face: NumberRowFace = { ...letters, [key]: true };
    check(P.numberRowExtraRows(true, face, false) === 0, `${key} 不画`);
    check(!P.lettersRowFace(face), `${key} 不是字母行一类`);
  }
  check(P.NUMBER_ROW.join("") === "1234567890", "数字行从 1 到 0");
  // 多出的一行正好是不带高度调整的一行键高加一道行距，切到 123 层时这一行还在（补充行），面板高度不变。
  const base = KeyboardMetrics.totalHeightVp(70, 0, 0, 20, 15, false, true, 0);
  const withRow = KeyboardMetrics.totalHeightVp(70, 0, 0, 20, 15, false, true, 1);
  check(close(withRow - base, KeyboardMetrics.ROW_HEIGHT_VP + 7), `面板高一行：${withRow - base}`);
});

group("123、#+= 层和借用数字九宫格的补充行与 Android `symbolExtraRow` 同表", () => {
  check(P.symbolExtraRow(true, false, true).join(" ") === "1 2 3 4 5 6 7 8 9 0", "#+= 层补数字");
  check(P.symbolExtraRow(true, true, true).join(" ") === "1 2 3 4 5 6 7 8 9 0", "#+= 优先于九宫格");
  check(
    P.symbolExtraRow(false, false, true).join(" ") === "[ ] { } # % ^ * + =",
    "123 层补该层没有的符号",
  );
  check(
    P.symbolExtraRow(false, true, true).join(" ") === "（ ） % ¥ $ € £ ‰ ² ³",
    "数字九宫格补全角括号、百分号、货币与上标",
  );
  check(
    P.symbolExtraRow(false, true, false).slice(0, 2).join(" ") === "( )",
    "英文标点时括号用半角",
  );
  for (const more of [true, false]) {
    for (const pad of [true, false]) {
      check(P.symbolExtraRow(more, pad, true).length === 10, "每排十个键，与数字行等宽");
    }
  }
});

group("中/英 不紧挨回车时回车不让出键帽", () => {
  check(
    !P.languageBesideReturn([
      { slot: DesignRowSlot.LANGUAGE, share: 1 },
      { slot: DesignRowSlot.SPACE, share: 1 },
      { slot: DesignRowSlot.RETURN, share: 1 },
    ]),
    "中间隔着空格",
  );
});

group("记下的键面过时：视图在扩展能力挂上回调之前请求的高度不会丢，面板停在真实键面的高度", () => {
  const height = (shape: TouchPanelShape): number =>
    KeyboardMetrics.totalHeightVp(
      undefined,
      0,
      0,
      undefined,
      undefined,
      false,
      shape.strip,
      shape.extraRows,
    );
  // 记下的是九键（不算 26/14/18 键一类），键盘进程没开时在设置里换成 26 键、开着数字行。
  const opened: TouchPanelShape = P.firstPanel(
    { lettersRow: false, chineseTwentySix: false },
    true,
    "standard",
    false,
    false,
  );
  const actual: TouchPanelShape = {
    strip: true,
    extraRows: P.numberRowExtraRows(true, letters, false),
  };
  check(
    opened.extraRows === 0 && actual.extraRows === 1,
    "扩展能力按过时的键面少算一行，视图按真实键面多一行",
  );
  check(height(actual) > height(opened), "两个高度不同");
  // 照扩展能力的次序走一遍：作废上一个面板的请求 → setUiContent → 定第一帧高度（优先用视图已经请求的）→ 挂上回调。视图的 aboutToAppear 可能落在这三步之间的任意一处，也可能在之后。
  const open = (viewAppears: "during_mount" | "before_attach" | "after_attach"): number[] => {
    const relay = new TouchHeightRelay();
    relay.request(height({ strip: false, extraRows: 0 }));
    relay.detach();
    const resized: number[] = [];
    if (viewAppears === "during_mount") relay.request(height(actual));
    const requested: number | null = relay.takePending();
    resized.push(requested !== null ? requested : height(opened));
    if (viewAppears === "before_attach") relay.request(height(actual));
    relay.attach((heightVp: number): void => {
      resized.push(heightVp);
    });
    if (viewAppears === "after_attach") relay.request(height(actual));
    return resized;
  };
  const mounted: number[] = open("during_mount");
  check(
    mounted.length === 1 && mounted[0] === height(actual),
    `视图先出现时第一帧直接用它的高度：${mounted.join(",")}`,
  );
  const between: number[] = open("before_attach");
  check(
    between.length === 2 && between[0] === height(opened) && between[1] === height(actual),
    `定完第一帧、挂上回调之前出现时挂上就补发：${between.join(",")}`,
  );
  const after: number[] = open("after_attach");
  check(
    after.length === 2 && after[0] === height(opened) && after[1] === height(actual),
    `挂上之后出现时直接改高度：${after.join(",")}`,
  );
  // 上一个面板留下的请求在 detach 时作废，不会被新面板当成第一帧的高度。
  for (const order of [mounted, between, after]) {
    check(order[order.length - 1] === height(actual), "最后停在真实键面的高度");
  }
  const released = new TouchHeightRelay();
  const late: number[] = [];
  released.attach((heightVp: number): void => {
    late.push(heightVp);
  });
  released.detach();
  released.request(height(actual));
  check(late.length === 0, "面板释放后不再改它的高度");
  check(released.takePending() === height(actual) && released.takePending() === null, "取过即清");
});

if (failures > 0) {
  console.log(`${failures} touch layout style group(s) failed`);
  throw new Error("touch layout style checks failed");
}
