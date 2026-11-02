import { ClipboardSelectionPolicy } from "../entry/src/main/ets/keyboard/clipboard/ClipboardSelectionPolicy";

let failures = 0;
let checks = 0;

/** 放在本地定义，这样测试套件不需要 node 类型定义，本仓库也没有这些定义。 */
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

console.log("ClipboardSelectionPolicy");

group("caps mirror Android: twenty selected, one to four inserted", () => {
  check(ClipboardSelectionPolicy.MAX_SELECTIONS === 20, "selection cap");
  check(ClipboardSelectionPolicy.MAX_INSERTIONS === 4, "insertion cap");
  check(
    ClipboardSelectionPolicy.canSelect(19) && !ClipboardSelectionPolicy.canSelect(20),
    "select boundary",
  );
  check(!ClipboardSelectionPolicy.canInsert(0), "nothing to insert");
  check(
    ClipboardSelectionPolicy.canInsert(1) && ClipboardSelectionPolicy.canInsert(4),
    "one to four insert",
  );
  check(!ClipboardSelectionPolicy.canInsert(5), "five do not insert");
});

group("ordinals are circled digits up to the selection cap", () => {
  check(ClipboardSelectionPolicy.ordinalLabel(0) === "①", "first");
  check(ClipboardSelectionPolicy.ordinalLabel(3) === "④", "fourth");
  check(ClipboardSelectionPolicy.ordinalLabel(19) === "⑳", "twentieth");
  check(ClipboardSelectionPolicy.ordinalLabel(-1) === "", "negative");
  check(ClipboardSelectionPolicy.ordinalLabel(20) === "", "past the cap");
});

group("deselecting moves the later ordinals forward and reselecting appends", () => {
  let selected: string[] = [];
  for (const text of ["合成甲", "合成乙", "合成丙", "合成丁"]) {
    selected = ClipboardSelectionPolicy.toggled(selected, text);
  }
  check(ClipboardSelectionPolicy.ordinal(selected, "合成丁") === 3, "tap order");
  check(ClipboardSelectionPolicy.ordinal(selected, "未选") === -1, "unselected");
  selected = ClipboardSelectionPolicy.toggled(selected, "合成甲");
  check(selected.join("|") === "合成乙|合成丙|合成丁", `after deselect: ${selected.join("|")}`);
  selected = ClipboardSelectionPolicy.toggled(selected, "合成甲");
  check(ClipboardSelectionPolicy.ordinal(selected, "合成甲") === 3, "reselect appends");
  check(
    ClipboardSelectionPolicy.insertionText(selected) === "合成乙合成丙合成丁合成甲",
    "joined without separators",
  );
});

group("the selection cap holds", () => {
  const full: string[] = [];
  for (let index = 0; index < 20; index++) {
    full.push(`合成 ${index}`);
  }
  check(
    ClipboardSelectionPolicy.toggled(full, "第二十一") === full,
    "capped selection is unchanged",
  );
});

group("notice and spoken prefix", () => {
  check(ClipboardSelectionPolicy.notice(4) === "点按选择，按 ①②③④ 的顺序插入", "usage at the cap");
  check(
    ClipboardSelectionPolicy.notice(5) === "一次最多插入 4 项，取消几项后再插入",
    "hint past the cap",
  );
  check(ClipboardSelectionPolicy.selectedDescription(1) === "已选择第 2 项；", "spoken ordinal");
  check(ClipboardSelectionPolicy.selectedDescription(-1) === "", "unselected");
});

console.log("");
if (failures > 0) {
  throw new Error(`${failures} group(s) failed`);
}
console.log(`all groups passed (${checks} assertions)`);
