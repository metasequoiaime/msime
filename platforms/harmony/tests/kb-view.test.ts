/**
 * 检查 `KeyboardView.ets` 所依赖、且能在普通 node 下运行的部分：新工具栏、候选栏和底行提供给读屏器的名称，以及触控时绘制的框架，其高度必须恰好等于 `KeyboardMetrics.totalHeightVp` 为面板设定的高度。
 *
 * 视图本身是 ArkTS，只由 `hvigorw` 编译，所以这些测试检查的是视图从旁边普通 TypeScript 中取用的部分，而不是视图本身。
 */
import { KeyAccessibilityPolicy } from "../entry/src/main/ets/keyboard/input/KeyAccessibilityPolicy";
import { KeyboardGeometry } from "../entry/src/main/ets/keyboard/KeyboardGeometry";
import { KeyboardMetrics } from "../entry/src/main/ets/keyboard/KeyboardMetrics";
import {
  EmojiCatalogModel,
  EMOJI_MIN_CELL_VP,
  EMOJI_RECENTS_ICON,
} from "../entry/src/main/ets/keyboard/emoji/EmojiCatalogModel";
import { ClipboardHistoryPolicy } from "../entry/src/main/ets/keyboard/clipboard/ClipboardHistoryPolicy";
import { CandidateWidthPolicy } from "../entry/src/main/ets/keyboard/candidate/CandidateWidthPolicy";
import {
  VoiceRecognitionPolicy,
  VOICE_DONE_NOTICE_MS,
  VOICE_SILENCE_STOP_MS,
} from "../entry/src/main/ets/keyboard/input/VoiceRecognitionPolicy";

let failures = 0;
let checks = 0;

/** 放在本地定义，这样测试套件不需要 node 类型定义，本仓库也没有带这些定义。 */
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
  checks++;
  if (!condition) {
    throw new Error(message);
  }
}

/**
 * 视图在触控时堆叠的内容，按 `keyboard()` 的布局方式累加：顶部内边距、候选条、其下的固定间距、四行按键（`KeyboardGeometry.adjustedRowHeight`，与 `rowHeight()` 的取法相同）及行间距，以及底部内边距。
 */
function drawnTouchHeight(rowSpacingTenths: number, adjustment: number): number {
  let keys: number = 0;
  for (let row: number = 0; row < KeyboardMetrics.KEY_ROWS; row++) {
    keys += KeyboardGeometry.adjustedRowHeight(
      KeyboardMetrics.ROW_HEIGHT_VP,
      adjustment,
      KeyboardMetrics.KEY_ROWS,
      row,
    );
  }
  const gaps: number = (rowSpacingTenths / 10) * (KeyboardMetrics.KEY_ROWS - 1);
  return (
    KeyboardMetrics.TOUCH_ROOT_TOP_PADDING_VP +
    KeyboardMetrics.STRIP_HEIGHT_VP +
    KeyboardMetrics.STRIP_GAP_VP +
    keys +
    gaps +
    KeyboardMetrics.TOUCH_ROOT_BOTTOM_PADDING_VP
  );
}

console.log("KeyboardView touch frame and labels");

group("the toolbar's tools are read as the panels they open", () => {
  check(KeyAccessibilityPolicy.phrase() === "常用语", "常用语 reads its name");
  check(KeyAccessibilityPolicy.clipboard() === "剪贴板", "剪贴板 reads its name");
  check(KeyAccessibilityPolicy.skin() === "皮肤", "皮肤 reads its name");
  check(
    KeyAccessibilityPolicy.scheme() === "选择输入方案",
    "输入方式 keeps the scheme picker's name",
  );
  check(KeyAccessibilityPolicy.collapse() === "收起键盘", "the chevron puts the keyboard away");
  check(
    KeyAccessibilityPolicy.collapse() === KeyAccessibilityPolicy.dismiss(),
    "and says so in the words the old dismiss icon used",
  );
});

group("the candidate bar's chevron says what the next tap does", () => {
  check(KeyAccessibilityPolicy.expandCandidates(false) === "展开候选", "closed, it opens the grid");
  check(
    KeyAccessibilityPolicy.expandCandidates(true) === "收起候选",
    "open, it puts the grid away",
  );
});

group("the bottom row's new key and the pickers' current entry have words", () => {
  check(KeyAccessibilityPolicy.period() === "句号", "the full stop key is a word, not a dot");
  check(KeyAccessibilityPolicy.current(true) === "当前使用", "the scheme in use says so");
  check(KeyAccessibilityPolicy.current(false) === "", "the others read as their names alone");
  check(
    KeyAccessibilityPolicy.returnKey("换行") === "换行",
    "a return key drawn as an arrow still reads the word for what it does",
  );
});

group("the touch frame the view draws is the panel height the ability asks for", () => {
  const spacings: number[] = [
    KeyboardGeometry.MIN_ROW_SPACING_TENTHS,
    KeyboardMetrics.ROW_SPACING_VP * 10,
    KeyboardGeometry.MAX_ROW_SPACING_TENTHS,
  ];
  const adjustments: number[] = [
    KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_VP,
    -5,
    0,
    7,
    30,
    KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP,
  ];
  for (const spacing of spacings) {
    for (const adjustment of adjustments) {
      const drawn: number = drawnTouchHeight(spacing, adjustment);
      const sized: number = KeyboardMetrics.totalHeightVp(spacing, adjustment);
      check(
        Math.abs(drawn - sized) < 1e-9,
        `row spacing ${spacing}, adjustment ${adjustment}: drawn ${drawn}, panel ${sized}`,
      );
    }
  }
});

group("the strip keeps one height whatever the candidate fonts and glosses", () => {
  const base: number = KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, false);
  check(
    KeyboardMetrics.totalHeightVp(70, 0, 2, 18, 15, false) === base,
    "gloss rows live inside the 50vp candidate bar",
  );
  check(
    KeyboardMetrics.totalHeightVp(70, 0, 0, 32, 24, false) === base,
    "and so do larger candidate and pre-edit fonts",
  );
  check(base === 8 + 50 + 10 + 4 * 44 + 3 * 7 + 6, "the design's frame at the default spacing");
});

group("the height bar's caption spans the range the shared validation allows", () => {
  check(
    KeyboardGeometry.heightPercent(KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP) === 100,
    "重置 reads 100%",
  );
  check(
    KeyboardGeometry.heightPercent(KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_VP) === 93,
    "the lowest the bar reaches",
  );
  check(
    KeyboardGeometry.heightPercent(KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP) === 127,
    "the highest the bar reaches",
  );
});

group("the emoji panel's bottom row has a glyph for every tab", () => {
  const icons: string[] = EmojiCatalogModel.categories().map((category) => category.icon);
  check(icons.length === 9, "nine Unicode categories, as Android has");
  check(
    icons.join(" ") ===
      "\u{1F600} \u{1F44B} \u{1F43E} \u{1F34E} \u{1F697} \u26BD \u{1F4A1} \u{1F523} \u{1F3F3}\uFE0F",
    "Android's category glyphs, in Unicode group order",
  );
  check(new Set(icons).size === icons.length, "no two categories share a glyph");
  check(EMOJI_RECENTS_ICON === "\u{1F558}", "recents is the clock, as on Android");
  check(!icons.includes(EMOJI_RECENTS_ICON), "and no category uses it");
});

group("the emoji grid shows three rows, none under the design's 40", () => {
  // 默认按键区 4 x 44 + 3 x 7，减去 40vp 的底行和其上方 6vp 的间距。
  const grid: number = 4 * 44 + 3 * 7 - 40 - 6;
  check(
    EmojiCatalogModel.cellHeightVp(grid, 2) === Math.floor((grid - 4) / 3),
    "three rows fill the grid",
  );
  check(
    3 * EmojiCatalogModel.cellHeightVp(grid, 2) + 2 * 2 <= grid,
    "and fit inside it with their gaps",
  );
  check(
    EmojiCatalogModel.cellHeightVp(90, 2) === EMOJI_MIN_CELL_VP,
    "a short keyboard keeps 40vp cells",
  );
  check(
    EmojiCatalogModel.cellHeightVp(Number.NaN, 2) === EMOJI_MIN_CELL_VP,
    "a broken height falls back to 40",
  );
  check(
    EmojiCatalogModel.cellHeightVp(200, -5) === Math.floor(200 / 3),
    "a negative gap counts as none",
  );
});

group("clipboard cards say when an entry was saved, as Android's do", () => {
  const now: number = Date.UTC(2026, 9, 8, 12, 0, 0);
  check(ClipboardHistoryPolicy.relativeTime(0, now) === "", "no time, no label");
  check(ClipboardHistoryPolicy.relativeTime(now - 30 * 1000, now) === "刚刚", "under a minute");
  check(ClipboardHistoryPolicy.relativeTime(now - 2 * 60000, now) === "2 分钟前", "minutes");
  check(ClipboardHistoryPolicy.relativeTime(now - 3 * 3600000, now) === "3 小时前", "hours");
  check(ClipboardHistoryPolicy.relativeTime(now - 2 * 86400000, now) === "2 天前", "days");
  check(
    ClipboardHistoryPolicy.relativeTime(Math.floor(now / 1000) - 120, now) === "2 分钟前",
    "a timestamp in seconds is read as seconds",
  );
  check(
    ClipboardHistoryPolicy.relativeTime(now + 3600000, now) === "刚刚",
    "a clock gone backwards reads as just now",
  );
  check(
    ClipboardHistoryPolicy.localMeta(true, now - 2 * 60000, now) === "已固定 · 本机 · 2 分钟前",
    "a pinned local entry",
  );
  check(
    ClipboardHistoryPolicy.localMeta(false, 0, now) === "本机",
    "an entry without a time names only where it is",
  );
  check(
    ClipboardHistoryPolicy.cloudMeta("2026-10-08T11:00:00Z", now) === "云端 · 1 小时前",
    "a cloud entry's ISO time",
  );
  check(
    ClipboardHistoryPolicy.cloudMeta("yesterday", now) === "云端",
    "an unreadable time is left out",
  );
});

group("a voice result goes straight in only where listening began", () => {
  const target = { editor: 4, context: 9 };
  check(
    VoiceRecognitionPolicy.insertsDirectly(target, { editor: 4, context: 9 }, false),
    "nothing moved",
  );
  check(
    !VoiceRecognitionPolicy.insertsDirectly(target, { editor: 4, context: 10 }, false),
    "the caret moved or the app changed the text",
  );
  check(
    !VoiceRecognitionPolicy.insertsDirectly(target, { editor: 5, context: 9 }, false),
    "another field, or a restarted session",
  );
  check(
    !VoiceRecognitionPolicy.insertsDirectly(target, { editor: 4, context: 9 }, true),
    "a composition began",
  );
  check(
    !VoiceRecognitionPolicy.insertsDirectly(null, { editor: 4, context: 9 }, false),
    "no editor was attached when listening began",
  );
  check(VOICE_DONE_NOTICE_MS === 1200 && VOICE_SILENCE_STOP_MS === 1500, "Android's timings");
});

group("只有触屏上的流式录音免审阅上屏，上传式服务商的结果等「提交」", () => {
  check(VoiceRecognitionPolicy.skipsReview(true, true, true), "流式录音、聆听界面仍在");
  check(
    !VoiceRecognitionPolicy.skipsReview(true, false, true),
    "上传式服务商（OpenAI 兼容）的录音在审阅界面上结束，结果留给「提交」",
  );
  check(!VoiceRecognitionPolicy.skipsReview(true, true, false), "语音界面已关闭");
  check(!VoiceRecognitionPolicy.skipsReview(false, true, true), "2in1 总是审阅");
});

group("触屏离开语音界面时放弃正在进行的录音", () => {
  check(
    VoiceRecognitionPolicy.cancelsOnLeave(true, true, true),
    "点工具栏离开聆听界面，麦克风不能继续开着",
  );
  check(!VoiceRecognitionPolicy.cancelsOnLeave(true, false, true), "没有在录音");
  check(!VoiceRecognitionPolicy.cancelsOnLeave(true, true, false), "并不是从语音界面离开");
  check(!VoiceRecognitionPolicy.cancelsOnLeave(false, true, true), "2in1 的语音面板另有规则");
});

group("the 2in1 candidate window is sized for the card's 8vp padding", () => {
  check(KeyboardMetrics.CANDIDATE_CARD_PADDING_VP === 8, "the design's card padding");
  check(
    KeyboardMetrics.candidateHeightVp("horizontal", 1, false, 0, 0, 18, 15) ===
      KeyboardMetrics.candidateRowHeightVp(18, true) +
        2 * KeyboardMetrics.CANDIDATE_CARD_PADDING_VP +
        2 * KeyboardMetrics.ROOT_VERTICAL_PADDING_VP,
    "the window is the row, the card padding and the frame",
  );
  check(
    KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, true) ===
      KeyboardMetrics.compositionRowHeightVp(15) +
        KeyboardMetrics.candidateRowHeightVp(18, true) +
        2 * KeyboardMetrics.CANDIDATE_CARD_PADDING_VP +
        KeyboardMetrics.ROW_HEIGHT_VP * KeyboardMetrics.KEY_ROWS +
        7 * KeyboardMetrics.KEY_ROWS +
        2 * KeyboardMetrics.ROOT_VERTICAL_PADDING_VP,
    "the tall 2in1 surfaces count the card padding too, or their last row is clipped",
  );
  check(
    CandidateWidthPolicy.widthVp([], "", 18, 15, 0, CandidateWidthPolicy.MAX_WIDTH_VP) ===
      CandidateWidthPolicy.MIN_WIDTH_VP,
    "an empty window keeps its minimum width",
  );
  check(
    CandidateWidthPolicy.EXTRA_WIDTH_VP >= 48 + 2 * KeyboardMetrics.CANDIDATE_CARD_PADDING_VP,
    "the width estimate leaves room for the padding on both sides",
  );
});

console.log("");
if (failures > 0) {
  throw new Error(`${failures} group(s) failed`);
}
console.log(`all groups passed (${checks} assertions)`);
