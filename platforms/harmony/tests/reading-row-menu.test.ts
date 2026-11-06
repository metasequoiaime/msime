import {
  ReadingRowEditAction as Action,
  ReadingRowEditMenuPolicy as Menu,
  ReadingRowEditTarget,
} from "../entry/src/main/ets/keyboard/candidate/ReadingRowEditMenuPolicy";

let assertions = 0;
function check(value: boolean, message: string): void {
  assertions++;
  if (!value) throw new Error(message);
}

const start = Menu.items("nihao", 0);
check(
  start.map((item) => item.title).join("|") === "光标移到开头|光标移到末尾|删除光标后的字母",
  "与 iOS 同名同序",
);
check(
  !start[0].enabled && start[1].enabled && start[2].enabled,
  "开头不可再向前，允许到末尾及向后删除",
);
const end = Menu.items("nihao", 5);
check(end[0].enabled && !end[1].enabled && !end[2].enabled, "末尾禁用无效操作，保留三项位置");
check(
  Menu.items("nihao", 2).every((item) => item.enabled),
  "中间允许全部三项操作",
);
check(
  Menu.items("", 0).every((item) => !item.enabled),
  "空组字没有可用动作",
);
check(!Menu.items("nihao", Number.NaN)[2].enabled, "无效光标按末尾处理，不误删首字母");

// 用组字状态验证菜单分发，不把 Home/End 发成候选首尾选择。
class Spelling implements ReadingRowEditTarget {
  text: string = "nihao";
  caret: number = 5;
  moveSpellingHome(): void {
    this.caret = 0;
  }
  moveSpellingEnd(): void {
    this.caret = this.text.length;
  }
  deleteForward(): void {
    this.text = this.text.slice(0, this.caret) + this.text.slice(this.caret + 1);
  }
}
const spelling = new Spelling();
Menu.dispatch(Action.START, spelling);
check(spelling.text === "nihao" && spelling.caret === 0, "移到开头不提交、不改拼写");
Menu.dispatch(Action.DELETE_FORWARD, spelling);
check(spelling.text === "ihao" && spelling.caret === 0, "删除的是光标后的字母");
Menu.dispatch(Action.END, spelling);
check(spelling.text === "ihao" && Number(spelling.caret) === 4, "移到末尾不改拼写");
console.log(`reading-row-menu: ${assertions} assertions passed`);
