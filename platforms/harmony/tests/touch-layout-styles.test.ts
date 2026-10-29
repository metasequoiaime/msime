/**
 * 触屏按键布局样式与顶部数字行（笔记 2026-10-27-touch-key-layout-styles.md）：每种样式的键序与份额、默认行与加样式之前逐键相同、偏好读法、数字行门槛和补充行。三端画同一组排法，这里钉住鸿蒙这一份。
 */
import {
  CommaHold,
  CommaSwipeGesture,
  DesignRowEntry,
  DesignRowItem,
  DesignRowSlot,
  NumberRowFace,
  StyledFace,
  TouchHeightRelay,
  TouchKeyboard,
  TouchLayoutStylePolicy,
  TouchPanelShape,
} from "../entry/src/main/ets/keyboard/TouchLayoutStylePolicy";
import {
  NineKeyAction,
  NineKeyActionItem,
  NineKeyActionKind,
  NineKeyLayout,
} from "../entry/src/main/ets/keyboard/input/NineKeyLayout";
import { KeyboardScheme, SchemeDefinition } from "../entry/src/main/ets/keyboard/KeyboardScheme";
import { KeyboardMetrics } from "../entry/src/main/ets/keyboard/KeyboardMetrics";
import { StrokeLayout } from "../entry/src/main/ets/keyboard/input/StrokeLayout";
import { SwipeHintPolicy } from "../entry/src/main/ets/keyboard/input/SwipeHintPolicy";
import { LetterHintGesture } from "../entry/src/main/ets/keyboard/input/LetterHintGesture";

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

group("逗号的长按与滑动", () => {
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
  check(P.commaSwipesPeriod(StyledFace.TWENTY_SIX, "long_space"), "26 键长空格滑动句号");
  check(P.commaSwipesPeriod(StyledFace.FOURTEEN, "long_space"), "14 键长空格滑动句号");
  for (const style of ["standard", "comma_period", "emoji"]) {
    check(!P.commaSwipesPeriod(StyledFace.TWENTY_SIX, style), `26 键 ${style} 不接滑动`);
  }
  check(!P.commaSwipesPeriod(StyledFace.NONE, "long_space"), "不受样式影响的底行不接滑动");
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

group(
  "逗号滑动手势：上滑或下滑过 14 vp 且竖向大于横向才算，满长按时长还没滑出去归长按，点按随之被吞一次",
  () => {
    const T: number = CommaSwipeGesture.THRESHOLD_VP;
    const H: number = CommaSwipeGesture.HOLD_MILLIS;
    check(T === SwipeHintPolicy.THRESHOLD_VP && T === 14, "门槛就是字母键滑动输入符号的 14 vp");
    check(H === LetterHintGesture.HOLD_MILLIS, "长按时长与字母键相同");
    const gesture = new CommaSwipeGesture();
    gesture.down(100, 100, 0);
    check(gesture.up(102, 100 + T + 1, 80), "向下滑过门槛");
    check(gesture.swallows(), "这次点按被吞");
    check(!gesture.swallows(), "只吞一次");
    gesture.down(100, 100, 0);
    check(gesture.up(102, 100 - T - 1, 80), "向上滑过门槛，与字母键默认方向一致");
    check(gesture.swallows(), "上滑的点按同样被吞");
    gesture.down(100, 100, 0);
    check(!gesture.up(100, 100 + T, 80), "正好在门槛上不算");
    check(!gesture.swallows(), "普通点按照常输入逗号");
    gesture.down(100, 100, 0);
    check(!gesture.up(100, 100 - T, 80), "向上正好在门槛上也不算");
    gesture.down(100, 100, 0);
    gesture.move(101, 96, 40);
    check(!gesture.up(102, 92, 80), "点按时手指往上带一点（8 vp，不到门槛）不出句号");
    check(!gesture.swallows(), "漂移的点按照常输入逗号");
    gesture.down(100, 100, 0);
    check(!gesture.up(140, 120, 80), "横向更多不算");
    gesture.down(100, 100, 0);
    check(!gesture.up(60, 80, 80), "往另一侧横向更多也不算");
    gesture.down(100, 100, 0);
    gesture.move(100, 100 - T - 6, 60);
    check(gesture.up(100, 100, 300), "滑过门槛的那一刻就定下是滑动，手指回到原处抬起仍输入句号");
    gesture.down(100, 100, 0);
    gesture.move(100, 101, H - 1);
    check(gesture.up(100, 100 - T - 6, H - 1), "长按时长到之前滑出去仍算");
    gesture.down(100, 100, 0);
    gesture.move(100, 101, H);
    check(!gesture.up(100, 100 - T - 6, H + 200), "满长按时长还没滑出去，之后上滑不再起滑动");
    gesture.down(100, 100, 0);
    check(!gesture.up(100, 100 + T + 6, H + 1), "只有抬起事件时也按时长判断");
    gesture.down(100, 100, 0);
    check(gesture.longPress(), "没滑出去时长按照旧");
    check(!gesture.up(100, 140, 100), "长按识别后抬起不再按滑动处理");
    gesture.down(100, 100, 0);
    gesture.move(100, 100 - T - 6, 60);
    check(!gesture.longPress(), "已经滑出去时长按不再做事");
    check(gesture.up(100, 100 - T - 6, 700), "抬起照样输入一次句号");
    gesture.down(100, 100, 0);
    gesture.move(100, 140, 60);
    gesture.cancel();
    check(!gesture.up(100, 140, 100), "触摸被取消后抬起不再按滑动处理");
    gesture.down(100, 100, 0);
    gesture.up(100, 140, 80);
    gesture.down(100, 100, 0);
    check(!gesture.swallows(), "点按没有送来时，下一次按下清掉吞键");
  },
);

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
  // 份额按没有键距算：外框 8.3 份，标准底行 9.15 份，123 与 中/英 是标准底行里的 1.25、1.05 份换算过来的宽度。
  check(
    nine(letters) === "layer_toggle:1.13 space:3.91 language:0.95",
    `字母层：123 | 空格 | 中/英：${nine(letters)}`,
  );
  const digits = NineKeyLayout.actionRow(true, false, "tall_return");
  check(
    nine(digits) === "layer_toggle:1.13 zero:2.96 space:0.95 language:0.95",
    `数字层：切换键 | 0 | 空格 | 中/英，0 占剩下的宽度、空格在它右边：${nine(digits)}`,
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

group(
  "九键「右侧大回车」：123、中/英 和数字层空格与标准底行同宽，空格或 0 占网格剩下的宽度",
  () => {
    const frameShare = NineKeyLayout.SIDE_SHARE * 2 + NineKeyLayout.GRID_SHARE;
    // 一块 390vp 宽、键距 6vp 的键面，和没有键距的份额两种量法。
    for (const [faceWidth, spacing] of [
      [390, 6],
      [360, 4],
      [frameShare, 0],
    ]) {
      for (const digitLayer of [false, true]) {
        const standard = NineKeyLayout.actionRow(digitLayer);
        const standardShare = total(standard);
        // 标准底行五个键、四道键距铺满整块键面。
        const unit = (faceWidth - (standard.length - 1) * spacing) / standardShare;
        const standardWidth = (kind: NineKeyActionKind): number =>
          (standard.find((item) => item.kind === kind)?.weight ?? 0) * unit;
        const row = NineKeyLayout.tallReturnRow(digitLayer, faceWidth, spacing);
        const width = (kind: NineKeyActionKind): number =>
          row.find((item) => item.kind === kind)?.weight ?? -1;
        const label = `${digitLayer ? "数字层" : "字母层"} ${faceWidth}/${spacing}`;
        check(
          close(
            width(NineKeyActionKind.LAYER_TOGGLE),
            standardWidth(NineKeyActionKind.LAYER_TOGGLE),
          ),
          `${label}：123 与标准底行同宽`,
        );
        check(
          close(width(NineKeyActionKind.LANGUAGE), standardWidth(NineKeyActionKind.LANGUAGE)),
          `${label}：中/英 与标准底行同宽`,
        );
        if (digitLayer) {
          check(
            close(width(NineKeyActionKind.SPACE), 1.05 * unit),
            `${label}：数字层空格是标准底行的 1.05 份`,
          );
          check(
            row.map((item) => item.kind).join(" ") === "layer_toggle zero space language",
            `${label}：切换键 | 0 | 空格 | 中/英`,
          );
        } else {
          check(
            row.map((item) => item.kind).join(" ") === "layer_toggle space language",
            `${label}：123 | 空格 | 中/英`,
          );
        }
        check(
          NineKeyLayout.tallReturnFill(digitLayer) ===
            (digitLayer ? NineKeyActionKind.ZERO : NineKeyActionKind.SPACE),
          `${label}：占剩余宽度的键`,
        );
        // 外框三列两道键距，中列再有 length - 1 道键距：这一段加键距正好是网格的宽度。
        const grid = ((faceWidth - 2 * spacing) * NineKeyLayout.GRID_SHARE) / frameShare;
        check(
          close(total(row) + (row.length - 1) * spacing, grid),
          `${label}：底行恰好与网格同宽：${total(row) + (row.length - 1) * spacing} 对 ${grid}`,
        );
        check(
          row.every((item) => item.weight > 0),
          `${label}：每个键都有宽度`,
        );
      }
    }
    // 没有键距时就是 actionRow 的份额，设置页示意图和热力图用的是这一份。
    for (const digitLayer of [false, true]) {
      check(
        nine(NineKeyLayout.tallReturnRow(digitLayer, frameShare, 0)) ===
          nine(NineKeyLayout.actionRow(digitLayer, false, "tall_return")),
        "actionRow 的份额是没有键距时的宽度",
      );
    }
  },
);

group("九键「右侧大回车」：右列与网格逐行对齐，回车跨网格最后一行和底行，笔画网格按两行切", () => {
  check(StrokeLayout.ROWS.length === 2, "笔画网格两行");
  check(
    NineKeyLayout.touchRows().length === 3 && NineKeyLayout.digits().length === 3,
    "拼音九键和数字层三行",
  );
  for (const [gridHeight, rowSpacing, bottomRow] of [
    [150, 6, 44],
    [132, 4, 40],
  ]) {
    for (const gridRows of [StrokeLayout.ROWS.length, 3]) {
      const label = `${gridRows} 行 ${gridHeight}/${rowSpacing}`;
      // 网格是 Column({ space: rowSpacing }) 里 gridRows 个 layoutWeight(1) 的行：各行同高，第 i 行从 i × (行高 + 行距) 开始。
      const gridRow = (gridHeight - (gridRows - 1) * rowSpacing) / gridRows;
      const lastRowTop = (gridRows - 1) * (gridRow + rowSpacing);
      const row = NineKeyLayout.tallReturnGridRowHeight(gridHeight, rowSpacing, gridRows);
      check(close(row, gridRow), `${label}：右列一格与网格一行同高：${row} 对 ${gridRow}`);
      // 右列回车上方每个网格行一个键（笔画只有 ⌫，拼音九键是 ⌫ 和第二格），各隔一道行距。
      const returnTop = (gridRows - 1) * (row + rowSpacing);
      check(
        close(returnTop, lastRowTop),
        `${label}：回车顶边与网格最后一行顶边对齐：${returnTop} 对 ${lastRowTop}`,
      );
      // 右列与外框同高：网格、一道行距、底行；回车占剩下的部分。
      const returnHeight = gridHeight + rowSpacing + bottomRow - returnTop;
      check(
        close(returnHeight, gridRow + rowSpacing + bottomRow),
        `${label}：回车是网格最后一行、一道行距和底行：${returnHeight}`,
      );
    }
    // 笔画网格两行：按三行切的旧写法（三行里的两行加一道行距）让 ⌫ 比笔画第一行高，回车从笔画第二行中间开始。
    const strokeRow = NineKeyLayout.tallReturnGridRowHeight(
      gridHeight,
      rowSpacing,
      StrokeLayout.ROWS.length,
    );
    const threeRowCut = ((gridHeight - 2 * rowSpacing) / 3) * 2 + rowSpacing;
    check(!close(strokeRow, threeRowCut), `笔画 ⌫ 不按三行切：${strokeRow} 对 ${threeRowCut}`);
  }
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
  // 中文键面不是 26 键的键盘（如韩文）停在英文 26 键上：字母层是 26 键一类，但不是中文 26 键。
  const otherOnEnglish = { lettersRow: true, chineseTwentySix: false };
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
    shape(P.firstPanel(otherOnEnglish, true, "long_space", true, false)) === "false:1",
    "中文键面不是 26 键的键盘（如韩文）停在英文 26 键：有数字行、不算长空格",
  );
  // 中文 26 键停在英文 26 键上记下的键面与停在中文时相同（chineseTwentySix 按键盘算，见下面「这块键盘的中文键面」那组）：长空格照样留着工具条。
  check(
    shape(P.firstPanel(twentySix, true, "long_space", true, false)) === "true:1",
    "中文 26 键停在英文 26 键、长空格、工具栏隐藏：工具条留着，有数字行",
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

group(
  "设计底行逐个画的键：份额和让位都在 ForEach 的键里，切中/英、改样式时变了的键重建、没变的留着",
  () => {
    const items = (english: boolean, styled: StyledFace, style: string) =>
      P.designRowItems(P.designRow(true, english, styled, style));
    const keys = (row: readonly DesignRowItem[]) => row.map((item) => P.designRowItemKey(item));
    const sources: DesignRowEntry[][] = [
      P.designRow(true, false, StyledFace.NONE, "standard"),
      P.designRow(true, true, StyledFace.NONE, "standard"),
      P.designRow(false, false, StyledFace.FOURTEEN, "standard"),
    ];
    for (const style of ["standard", "comma_period", "long_space", "emoji"]) {
      sources.push(P.designRow(true, false, StyledFace.TWENTY_SIX, style));
    }
    for (const style of ["long_space", "emoji"]) {
      sources.push(P.designRow(false, false, StyledFace.FOURTEEN, style));
    }
    for (const source of sources) {
      const row = P.designRowItems(source);
      const label = describe(source);
      // 次序、键位和份额原样取自 designRow。
      check(describe(row) === label, `${describe(row)} 对 ${label}`);
      check(new Set(keys(row)).size === row.length, `ForEach 的键在一行里不重复：${label}`);
      for (const slot of [
        DesignRowSlot.LAYER,
        DesignRowSlot.SPACE,
        DesignRowSlot.LANGUAGE,
        DesignRowSlot.RETURN,
      ]) {
        check(
          row.some((item) => item.slot === slot),
          `${slot} 每一行都画：${label}`,
        );
      }
      for (const item of row) {
        const yields = item.slot === DesignRowSlot.LANGUAGE || item.slot === DesignRowSlot.RETURN;
        check(
          item.besideReturn === yields,
          `只有中/英 和回车让位，且这些行里中/英 都紧挨回车：${label} ${item.slot}`,
        );
      }
    }
    // 中文 26 键切到英文 26 键：空格从 4 让到 3、多出句点，这两个键的键变了；123、符、逗号、中/英、回车的键不变，节点留着。
    const chinese = keys(items(false, StyledFace.NONE, "standard"));
    const english = keys(items(true, StyledFace.NONE, "standard"));
    const space = (row: string[]) => row.find((key) => key.startsWith(`${DesignRowSlot.SPACE}:`));
    check(
      space(chinese) !== space(english),
      `空格的键随份额变：${space(chinese)} → ${space(english)}`,
    );
    check(!chinese.some((key) => key.startsWith(`${DesignRowSlot.PERIOD}:`)), "中文 26 键没有句点");
    check(
      english.some((key) => key.startsWith(`${DesignRowSlot.PERIOD}:`)),
      "英文 26 键多出句点",
    );
    for (const key of chinese.filter((key) => key !== space(chinese))) {
      check(english.includes(key), `没变的键留着：${key}`);
    }
    // 换成长空格：空格份额变了，键也跟着变。
    const longSpace = keys(items(false, StyledFace.TWENTY_SIX, "long_space"));
    check(space(longSpace) !== space(chinese), "改样式后空格按新份额重建");
    // 中/英 不紧挨回车时两键都不让位，键也不同，让位一变就重建。
    const apart = P.designRowItems([
      { slot: DesignRowSlot.LANGUAGE, share: 1 },
      { slot: DesignRowSlot.SPACE, share: 1 },
      { slot: DesignRowSlot.RETURN, share: 1 },
    ]);
    check(
      apart.every((item) => !item.besideReturn),
      "隔着空格时没有让位",
    );
    check(
      P.designRowItemKey(apart[2]) !==
        P.designRowItemKey({ slot: DesignRowSlot.RETURN, share: 1, besideReturn: true }),
      "让位在键里",
    );
  },
);

group(
  "九键外框底行逐个画的键：「右侧大回车」量到外框宽度后定宽，宽度、份额和让位都在 ForEach 的键里",
  () => {
    const keys = (row: readonly NineKeyActionItem[]) =>
      row.map((item) => NineKeyLayout.actionItemKey(item));
    const text = (row: readonly NineKeyActionItem[]) =>
      row
        .map(
          (item) =>
            `${item.kind}:${Math.round(item.share * 100) / 100}/${Math.round(item.width * 100) / 100}`,
        )
        .join(" ");
    // 标准九键：没有固定宽度，全按份额，中/英 和回车让位。
    for (const digitLayer of [false, true]) {
      for (const english of [false, true]) {
        for (const style of ["standard", "comma_period", "emoji"]) {
          const row = NineKeyLayout.actionRow(digitLayer, english, style);
          const items = NineKeyLayout.actionItems(
            row,
            [],
            NineKeyLayout.tallReturnFill(digitLayer),
          );
          const label = `${digitLayer ? "数字层" : "字母层"} ${english ? "英文" : "中文"} ${style}`;
          check(
            nine(items.map((item) => ({ kind: item.kind, weight: item.share }))) === nine(row),
            `份额原样：${label}`,
          );
          check(
            items.every((item) => item.width === 0),
            `没有固定宽度：${label}`,
          );
          check(new Set(keys(items)).size === items.length, `ForEach 的键不重复：${label}`);
          for (const item of items) {
            const yields =
              item.kind === NineKeyActionKind.LANGUAGE || item.kind === NineKeyActionKind.RETURN;
            check(item.besideReturn === yields, `${label} ${item.kind} 让位`);
          }
        }
      }
    }
    // 「右侧大回车」：一块 390vp 宽、键距 6vp 的外框。
    const faceWidth = 390;
    const spacing = 6;
    for (const digitLayer of [false, true]) {
      const row = NineKeyLayout.actionRow(digitLayer, false, "tall_return");
      const fill = NineKeyLayout.tallReturnFill(digitLayer);
      const fixed = NineKeyLayout.tallReturnRow(digitLayer, faceWidth, spacing);
      const label = digitLayer ? "数字层" : "字母层";
      // 没量到外框宽度时（视图交来空的 fixed）按份额排。
      const unmeasured = NineKeyLayout.actionItems(row, [], fill);
      check(
        unmeasured.every((item) => item.width === 0 && item.share > 0),
        `${label} 没量到时按份额：${text(unmeasured)}`,
      );
      const measured = NineKeyLayout.actionItems(row, fixed, fill);
      for (const item of measured) {
        const pinned = fixed.find((entry) => entry.kind === item.kind);
        if (item.kind === fill) {
          check(
            item.width === 0 && item.share > 0,
            `${label} ${item.kind} 占剩下的宽度：${text(measured)}`,
          );
        } else {
          check(
            pinned !== undefined && close(item.width, pinned.weight) && item.share === 0,
            `${label} ${item.kind} 按 tallReturnRow 定宽、份额为 0：${text(measured)}`,
          );
        }
        check(!item.besideReturn, `${label} 回车在右列，中/英 不让位`);
      }
      // 量到宽度后定了宽的键换了键，ForEach 重建它们；占剩余宽度的键留着。
      for (let index = 0; index < measured.length; index++) {
        const same =
          NineKeyLayout.actionItemKey(measured[index]) ===
          NineKeyLayout.actionItemKey(unmeasured[index]);
        check(
          same === (measured[index].kind === fill),
          `${label} ${measured[index].kind} 量到宽度后${same ? "留着" : "重建"}`,
        );
      }
    }
    // 字母层切到数字层：空格从占剩余宽度变成定宽 1.05 份，键随之不同，不会停在字母层的份额上。
    const space = (digitLayer: boolean) =>
      NineKeyLayout.actionItems(
        NineKeyLayout.actionRow(digitLayer, false, "tall_return"),
        NineKeyLayout.tallReturnRow(digitLayer, faceWidth, spacing),
        NineKeyLayout.tallReturnFill(digitLayer),
      ).find((item) => item.kind === NineKeyActionKind.SPACE);
    const letterSpace = space(false);
    const digitSpace = space(true);
    check(
      letterSpace !== undefined &&
        digitSpace !== undefined &&
        letterSpace.width === 0 &&
        digitSpace.width > 0 &&
        digitSpace.share === 0 &&
        NineKeyLayout.actionItemKey(letterSpace) !== NineKeyLayout.actionItemKey(digitSpace),
      "字母层与数字层的空格是两个不同的键",
    );
    // 外框宽度变了（转屏）定宽的键也跟着重建。
    const narrow = NineKeyLayout.actionItems(
      NineKeyLayout.actionRow(false, false, "tall_return"),
      NineKeyLayout.tallReturnRow(false, 360, 4),
      NineKeyActionKind.SPACE,
    );
    const wide = NineKeyLayout.actionItems(
      NineKeyLayout.actionRow(false, false, "tall_return"),
      NineKeyLayout.tallReturnRow(false, faceWidth, spacing),
      NineKeyActionKind.SPACE,
    );
    check(
      NineKeyLayout.actionItemKey(narrow[0]) !== NineKeyLayout.actionItemKey(wide[0]),
      "外框宽度变了，123 按新宽度重建",
    );
  },
);

group("这块键盘的中文键面是不是中文 26 键：按中文算，没有中/英这个参数", () => {
  const keyboard = (
    scheme: SchemeDefinition,
    extra: { nineKey?: boolean; letterGrid?: boolean; kanaGrid?: boolean; localMode?: string } = {},
  ): TouchKeyboard => ({
    scheme: scheme,
    localMode: extra.localMode ?? "none",
    nineKey: extra.nineKey ?? false,
    letterGrid: extra.letterGrid ?? false,
    kanaGrid: extra.kanaGrid ?? false,
  });
  for (const scheme of [
    KeyboardScheme.QUANPIN,
    KeyboardScheme.XIAOHE,
    KeyboardScheme.WUBI,
    KeyboardScheme.CANTONESE,
  ]) {
    const board = keyboard(scheme);
    check(P.chineseTwentySixKeyboard(board), `${scheme.id} 是中文 26 键`);
    // 停在英文 26 键上时字母层仍是 26 键一类，记下的键面和工具条都按中文 26 键算。
    check(P.lettersRowFace(P.letterFace(board, true)), `${scheme.id} 的英文 26 键也是字母层`);
  }
  for (const [label, board] of [
    ["韩文", keyboard(KeyboardScheme.KOREAN)],
    ["越南语", keyboard(KeyboardScheme.VIETNAMESE)],
    ["藏文", keyboard(KeyboardScheme.TIBETAN)],
    ["日语罗马字", keyboard(KeyboardScheme.JAPANESE)],
    ["日语假名", keyboard(KeyboardScheme.JAPANESE_NINE_KEY, { kanaGrid: true })],
    ["大千注音", keyboard(KeyboardScheme.ZHUYIN)],
    ["笔画", keyboard(KeyboardScheme.STROKE)],
    ["手写", keyboard(KeyboardScheme.HANDWRITING)],
    ["拼音九键", keyboard(KeyboardScheme.QUANPIN_NINE_KEY, { nineKey: true })],
    ["14 键", keyboard(KeyboardScheme.QUANPIN_FOURTEEN_KEY, { letterGrid: true })],
    ["18 键", keyboard(KeyboardScheme.QUANPIN_EIGHTEEN_KEY, { letterGrid: true })],
  ] as [string, TouchKeyboard][]) {
    check(!P.chineseTwentySixKeyboard(board), `${label} 不是中文 26 键`);
  }
  // 中文 26 键停在英文 26 键、选了长空格、工具栏隐藏：这一行照样留着（只是工具栏上没有 符），与停在中文时同高。
  const quanpin = keyboard(KeyboardScheme.QUANPIN);
  const kept = P.longSpaceActive(P.chineseTwentySixKeyboard(quanpin), "long_space", false);
  check(kept, "按键盘算，长空格留着工具栏那一行");
  const recorded = {
    lettersRow: P.lettersRowFace(P.letterFace(quanpin, true)),
    chineseTwentySix: P.chineseTwentySixKeyboard(quanpin),
  };
  const chineseRecorded = {
    lettersRow: P.lettersRowFace(P.letterFace(quanpin, false)),
    chineseTwentySix: P.chineseTwentySixKeyboard(quanpin),
  };
  for (const numberRow of [false, true]) {
    const onEnglish = P.firstPanel(recorded, numberRow, "long_space", true, false);
    const onChinese = P.firstPanel(chineseRecorded, numberRow, "long_space", true, false);
    check(
      onEnglish.strip &&
        onEnglish.strip === onChinese.strip &&
        onEnglish.extraRows === onChinese.extraRows,
      `停在英文 26 键时第一帧与中文同高（数字行${numberRow ? "开" : "关"}）`,
    );
  }
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

group(
  "「符」与「123」互换（#10056）：26 键底行里有 符 时挪到 123 左边，份额不变；没有 符 的行不变",
  () => {
    const standard = P.designRow(true, false, StyledFace.TWENTY_SIX, P.STANDARD);
    check(
      describe(P.symbolFirst(standard, true)) ===
        "symbol_panel:1.05 layer:1.25 space:4 comma:1 language:1.05 return:1.9",
      describe(P.symbolFirst(standard, true)),
    );
    check(describe(P.symbolFirst(standard, false)) === describe(standard), "关着时原样");
    const commaPeriod = P.designRow(true, false, StyledFace.TWENTY_SIX, P.COMMA_PERIOD);
    check(
      describe(P.symbolFirst(commaPeriod, true)) ===
        "symbol_panel:1.05 layer:1.25 comma:1 space:3 period:1 language:1.05 return:1.9",
      describe(P.symbolFirst(commaPeriod, true)),
    );
    const english = P.designRow(true, true, StyledFace.NONE, P.STANDARD);
    check(P.symbolFirst(english, true)[0].slot === DesignRowSlot.SYMBOL_PANEL, "英文 26 键也互换");
    for (const style of [P.LONG_SPACE, P.EMOJI]) {
      const row = P.designRow(true, false, StyledFace.TWENTY_SIX, style);
      check(describe(P.symbolFirst(row, true)) === describe(row), `${style} 没有 符，不变`);
      check(!P.symbolLeads(P.symbolFirst(row, true)), `${style} 借来的数字九宫格仍以返回打头`);
    }
    for (const style of [P.STANDARD, P.LONG_SPACE, P.EMOJI]) {
      const row = P.designRow(false, false, StyledFace.FOURTEEN, style);
      check(describe(P.symbolFirst(row, true)) === describe(row), `14/18 键 ${style} 不变`);
    }
    for (const style of [P.STANDARD, P.COMMA_PERIOD]) {
      const row = P.designRow(true, false, StyledFace.TWENTY_SIX, style);
      check(close(total(P.symbolFirst(row, true)), total(row)), `${style} 整行份额不变`);
      check(
        P.symbolLeads(P.symbolFirst(row, true)) && !P.symbolLeads(row),
        `${style} 互换后以 符 打头`,
      );
    }
    // 借来的数字九宫格按九键一族的次序排（`nineKeyDigits`）：前两键与互换后的字母层同序，都是 符 | 返回，各键宽度与九键数字层相同。
    const pad = NineKeyLayout.actionRow(true, false, P.STANDARD, true);
    const swapped = P.symbolFirst(standard, true);
    check(
      pad[0].kind === NineKeyActionKind.SYMBOL_PANEL &&
        pad[1].kind === NineKeyActionKind.LAYER_TOGGLE,
      nine(pad),
    );
    check(
      swapped[0].slot === DesignRowSlot.SYMBOL_PANEL && swapped[1].slot === DesignRowSlot.LAYER,
      describe(swapped),
    );
  },
);

group(
  "数字键盘空格位置（#10056）：缺省是鸿蒙原来的 空格 | 0，另两种是 0 | 空格 和空格在右列",
  () => {
    check(P.numberPadSpace(undefined) === P.BEFORE_ZERO, "缺键按鸿蒙原来的排法");
    check(
      P.numberPadSpace("platform_default") === P.BEFORE_ZERO,
      "platform_default 按鸿蒙原来的排法",
    );
    check(
      P.numberPadSpace("space_in_grid") === P.BEFORE_ZERO && P.numberPadSpace(3) === P.BEFORE_ZERO,
      "不认识的值",
    );
    check(
      P.numberPadSpace("right_column") === P.RIGHT_COLUMN &&
        P.numberPadSpace("after_zero") === P.AFTER_ZERO,
      "认识的值",
    );
    for (const nineKeyDigits of [true, false]) {
      const today = NineKeyLayout.actionRow(true, false, P.STANDARD, nineKeyDigits);
      check(
        nine(NineKeyLayout.actionRow(true, false, P.STANDARD, nineKeyDigits, P.BEFORE_ZERO)) ===
          nine(today),
        "before_zero 就是原来的数字层",
      );
      const after = NineKeyLayout.actionRow(true, false, P.STANDARD, nineKeyDigits, P.AFTER_ZERO);
      const column = NineKeyLayout.actionRow(
        true,
        false,
        P.STANDARD,
        nineKeyDigits,
        P.RIGHT_COLUMN,
      );
      const lead = nineKeyDigits ? "symbol_panel:1.05 layer_toggle:1.25" : "layer_toggle:1.25";
      const wide = nineKeyDigits ? 3.15 : 4.2;
      check(
        nine(today) === `${lead} space:${wide} zero:1.05 language:1.05 return:1.6`,
        nine(today),
      );
      check(
        nine(after) === `${lead} zero:${wide} space:1.05 language:1.05 return:1.6`,
        nine(after),
      );
      check(
        nine(column) ===
          `${lead} zero:${Math.round((wide + 1.05) * 100) / 100} language:1.05 return:1.6`,
        nine(column),
      );
      for (const row of [after, column]) {
        check(close(total(row), total(today)), `${nine(row)} 整行份额不变`);
      }
    }
    // 字母层和右侧大回车不受影响。
    for (const space of [P.RIGHT_COLUMN, P.AFTER_ZERO, P.BEFORE_ZERO]) {
      check(
        nine(NineKeyLayout.actionRow(false, false, P.STANDARD, false, space)) ===
          nine(NineKeyLayout.actionRow(false)),
        `${space} 不动字母层`,
      );
      check(
        nine(NineKeyLayout.actionRow(true, false, P.TALL_RETURN, true, space)) ===
          nine(NineKeyLayout.actionRow(true, false, P.TALL_RETURN, true)),
        `${space} 不动右侧大回车`,
      );
    }
  },
);

group("「中/英键放在空格左边」（#10228）偏好读法：只有字符串 left 算左，其余都按右", () => {
  check(P.languageKeyLeft("left"), "left 是左");
  for (const value of [undefined, null, "right", "", "LEFT", "middle", 1, true, false]) {
    check(!P.languageKeyLeft(value), `${String(value)} 按右`);
  }
});

group(
  "「中/英键放在空格左边」（#10228）：设计底行的 中/英 挪到空格和紧贴空格的标点左边，份额不变，回车不再让位",
  () => {
    const left = (row: DesignRowEntry[]) => P.languageBeforeSpace(row, true);
    const cases: [string, DesignRowEntry[], string][] = [
      [
        "中文 26 键标准",
        P.designRow(true, false, StyledFace.TWENTY_SIX, P.STANDARD),
        "layer:1.25 symbol_panel:1.05 language:1.05 space:4 comma:1 return:1.9",
      ],
      [
        "中文 26 键逗号句号",
        P.designRow(true, false, StyledFace.TWENTY_SIX, P.COMMA_PERIOD),
        "layer:1.25 symbol_panel:1.05 language:1.05 comma:1 space:3 period:1 return:1.9",
      ],
      [
        "中文 26 键长空格",
        P.designRow(true, false, StyledFace.TWENTY_SIX, P.LONG_SPACE),
        "layer:1.25 language:1.05 comma:1 space:5.05 return:1.9",
      ],
      [
        "中文 26 键表情键",
        P.designRow(true, false, StyledFace.TWENTY_SIX, P.EMOJI),
        "layer:1.25 emoji:1.05 language:1.05 space:4 comma:1 return:1.9",
      ],
      [
        "标准 + 互换",
        P.symbolFirst(P.designRow(true, false, StyledFace.TWENTY_SIX, P.STANDARD), true),
        "symbol_panel:1.05 layer:1.25 language:1.05 space:4 comma:1 return:1.9",
      ],
      [
        "逗号句号 + 互换",
        P.symbolFirst(P.designRow(true, false, StyledFace.TWENTY_SIX, P.COMMA_PERIOD), true),
        "symbol_panel:1.05 layer:1.25 language:1.05 comma:1 space:3 period:1 return:1.9",
      ],
      [
        "英文 26 键",
        P.designRow(true, true, StyledFace.NONE, P.STANDARD),
        "layer:1.25 symbol_panel:1.05 language:1.05 period:1 space:3 comma:1 return:1.9",
      ],
      [
        "14/18 键标准",
        P.designRow(false, false, StyledFace.FOURTEEN, P.STANDARD),
        "layer:1.25 language:1.05 comma:1 space:4 period:1 return:1.9",
      ],
      [
        "14/18 键长空格",
        P.designRow(false, false, StyledFace.FOURTEEN, P.LONG_SPACE),
        "layer:1.25 language:1.05 comma:1 space:5 return:1.9",
      ],
      [
        "14/18 键表情键",
        P.designRow(false, false, StyledFace.FOURTEEN, P.EMOJI),
        "layer:1.25 emoji:1 language:1.05 space:4 comma:1 return:1.9",
      ],
      [
        "大千注音",
        P.designRow(false, false, StyledFace.NONE, P.STANDARD),
        "layer:1.25 language:1.05 comma:1 space:4 period:1 return:1.9",
      ],
    ];
    for (const [name, row, expected] of cases) {
      const moved = left(row);
      check(describe(moved) === expected, `${name}：${describe(moved)}`);
      check(close(total(moved), total(row)), `${name} 整行份额不变`);
      check(describe(P.languageBeforeSpace(row, false)) === describe(row), `${name} 关着时原样`);
      // 已经在左边时再排一次不动。
      check(describe(left(moved)) === expected, `${name} 再排一次不变`);
      // 中/英 不再紧挨回车：两键都不让出键帽。
      check(!P.languageBesideReturn(moved), `${name} 回车不让位`);
      check(
        P.designRowItems(moved).every((item) => !item.besideReturn),
        `${name} 每个键都不让位`,
      );
      check(P.languageBesideReturn(row), `${name} 右边时仍紧挨回车`);
    }
    // 中文 26 键和英文 26 键的 中/英 停在同一格（第三格），切中/英时这个键不跳。
    for (const style of [P.STANDARD, P.COMMA_PERIOD]) {
      const chinese = left(P.designRow(true, false, StyledFace.TWENTY_SIX, style));
      const english = left(P.designRow(true, true, StyledFace.NONE, P.STANDARD));
      const slot = (row: DesignRowEntry[]) =>
        row.findIndex((item) => item.slot === DesignRowSlot.LANGUAGE);
      check(slot(chinese) === 2 && slot(english) === 2, `${style} 与英文 26 键同一格`);
    }
    // 行里没有 中/英（比如被别的设置去掉），或 中/英 左边不是空格和标点时原样返回。
    const noLanguage: DesignRowEntry[] = [
      { slot: DesignRowSlot.LAYER, share: 1.25 },
      { slot: DesignRowSlot.SPACE, share: 5.05 },
      { slot: DesignRowSlot.RETURN, share: 1.9 },
    ];
    check(describe(left(noLanguage)) === describe(noLanguage), "没有 中/英 时不变");
    const afterLayer: DesignRowEntry[] = [
      { slot: DesignRowSlot.LAYER, share: 1.25 },
      { slot: DesignRowSlot.LANGUAGE, share: 1.05 },
      { slot: DesignRowSlot.SPACE, share: 4 },
      { slot: DesignRowSlot.RETURN, share: 1.9 },
    ];
    check(describe(left(afterLayer)) === describe(afterLayer), "中/英 紧跟 123 时不变");
  },
);

group(
  "「中/英键放在空格左边」（#10228）：九键外框、数字层、借来的数字九宫格和右侧大回车的 中/英 都挪到空格左边",
  () => {
    const cases: [string, NineKeyAction[], NineKeyAction[], string][] = [
      [
        "九键标准",
        NineKeyLayout.actionRow(false),
        NineKeyLayout.actionRow(false, false, P.STANDARD, false, P.BEFORE_ZERO, true),
        "symbol_panel:1.05 layer_toggle:1.25 language:1.05 space:4.2 return:1.6",
      ],
      [
        "底栏带标点",
        NineKeyLayout.actionRow(false, false, P.COMMA_PERIOD),
        NineKeyLayout.actionRow(false, false, P.COMMA_PERIOD, false, P.BEFORE_ZERO, true),
        "symbol_panel:1.05 layer_toggle:1.25 language:1.05 comma:1 space:2.2 period:1 return:1.6",
      ],
      [
        "表情键",
        NineKeyLayout.actionRow(false, false, P.EMOJI),
        NineKeyLayout.actionRow(false, false, P.EMOJI, false, P.BEFORE_ZERO, true),
        "symbol_panel:1.05 layer_toggle:1.25 emoji:1.05 language:1.05 space:3.15 return:1.6",
      ],
      [
        "英文九键",
        NineKeyLayout.actionRow(false, true),
        NineKeyLayout.actionRow(false, true, P.STANDARD, false, P.BEFORE_ZERO, true),
        "symbol_panel:1.05 layer_toggle:1.25 language:1.05 period:1 space:2.2 comma:1 return:1.6",
      ],
      [
        "右侧大回车字母层",
        NineKeyLayout.actionRow(false, false, P.TALL_RETURN),
        NineKeyLayout.actionRow(false, false, P.TALL_RETURN, false, P.BEFORE_ZERO, true),
        "layer_toggle language space",
      ],
      [
        "右侧大回车数字层",
        NineKeyLayout.actionRow(true, false, P.TALL_RETURN, true),
        NineKeyLayout.actionRow(true, false, P.TALL_RETURN, true, P.BEFORE_ZERO, true),
        "layer_toggle language zero space",
      ],
    ];
    for (const nineKeyDigits of [true, false]) {
      const lead = nineKeyDigits ? "symbol_panel:1.05 layer_toggle:1.25" : "layer_toggle:1.25";
      const wide = nineKeyDigits ? 3.15 : 4.2;
      const pads: [string, string][] = [
        [P.BEFORE_ZERO, `${lead} language:1.05 space:${wide} zero:1.05 return:1.6`],
        [P.AFTER_ZERO, `${lead} language:1.05 zero:${wide} space:1.05 return:1.6`],
        [
          P.RIGHT_COLUMN,
          `${lead} language:1.05 zero:${Math.round((wide + 1.05) * 100) / 100} return:1.6`,
        ],
      ];
      for (const [space, expected] of pads) {
        cases.push([
          `${nineKeyDigits ? "九键一族" : "借来的"}数字九宫格 ${space}`,
          NineKeyLayout.actionRow(true, false, P.STANDARD, nineKeyDigits, space),
          NineKeyLayout.actionRow(true, false, P.STANDARD, nineKeyDigits, space, true),
          expected,
        ]);
      }
    }
    for (const [name, right, left, expected] of cases) {
      // 右侧大回车的份额是按外框换算的小数，只核对次序。
      const shown = name.startsWith("右侧大回车")
        ? left.map((item) => item.kind).join(" ")
        : nine(left);
      check(shown === expected, `${name}：${shown}`);
      check(close(total(left), total(right)), `${name} 整行份额不变`);
      for (const item of right) {
        const same = left.find((moved) => moved.kind === item.kind);
        check(
          same !== undefined && close(same.weight, item.weight),
          `${name} ${item.kind} 份额不变`,
        );
      }
      check(!NineKeyLayout.languageBesideReturn(left), `${name} 回车不让位`);
      check(
        NineKeyLayout.actionItems(left, [], NineKeyActionKind.SPACE).every(
          (item) => !item.besideReturn,
        ),
        `${name} 每个键都不让位`,
      );
      check(
        nine(NineKeyLayout.languageBeforeSpace(left, true)) === nine(left),
        `${name} 再排一次不变`,
      );
    }
    // 右侧大回车量到外框宽度后：每个键的固定宽度与右边时相同，只换次序，ForEach 按键位取宽度。
    for (const digitLayer of [false, true]) {
      const right = NineKeyLayout.tallReturnRow(digitLayer, 360, 6);
      const left = NineKeyLayout.tallReturnRow(digitLayer, 360, 6, true);
      check(
        left.map((item) => item.kind).join(" ") ===
          (digitLayer ? "layer_toggle language zero space" : "layer_toggle language space"),
        left.map((item) => item.kind).join(" "),
      );
      for (const item of right) {
        const same = left.find((moved) => moved.kind === item.kind);
        check(same !== undefined && close(same.weight, item.weight), `${item.kind} 宽度不变`);
      }
      const fill = NineKeyLayout.tallReturnFill(digitLayer);
      const row = NineKeyLayout.actionRow(
        digitLayer,
        false,
        P.TALL_RETURN,
        true,
        P.BEFORE_ZERO,
        true,
      );
      const items = NineKeyLayout.actionItems(row, left, fill);
      check(
        items.map((item) => item.kind).join(" ") === left.map((item) => item.kind).join(" "),
        "画出来的次序就是左边的次序",
      );
      for (const item of items) {
        const pinned = right.find((fixed) => fixed.kind === item.kind)!;
        check(
          item.kind === fill ? item.width === 0 : close(item.width, pinned.weight),
          `${item.kind} 取右边时的固定宽度`,
        );
      }
    }
    // 字母层与它借出的数字九宫格之间 中/英 不跳：九键一族字母层和数字层都在第三格。
    const letters = NineKeyLayout.actionRow(false, false, P.STANDARD, false, P.BEFORE_ZERO, true);
    const digits = NineKeyLayout.actionRow(true, false, P.STANDARD, true, P.BEFORE_ZERO, true);
    const slot = (row: NineKeyAction[]) =>
      row.findIndex((item) => item.kind === NineKeyActionKind.LANGUAGE);
    check(slot(letters) === 2 && slot(digits) === 2, "九键字母层和数字层同一格");
  },
);

if (failures > 0) {
  console.log(`${failures} touch layout style group(s) failed`);
  throw new Error("touch layout style checks failed");
}
