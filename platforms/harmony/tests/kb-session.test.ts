/**
 * 检查 `KeyboardSession.ets` 和 `EntryAbility.ets` 所依据规则的纯 TypeScript 部分：键盘可以直接打开哪些设置页，常用语何时补入初始条目，以及固定标点怎样压过中文标点开关。
 *
 * 会话和 ability 是 ArkTS，只由 `hvigorw` 编译，所以这里检查的是它们从旁边纯 TypeScript 文件里取用的部分，而不是这两个文件本身。
 */
import { CommonPhraseStarters } from "../entry/src/main/ets/keyboard/CommonPhraseStarters";
import { SettingsPageLink } from "../entry/src/main/ets/keyboard/SettingsPageLink";
import { PunctuationLockPolicy } from "../entry/src/main/ets/keyboard/input/PunctuationLockPolicy";

let failures = 0;
let checks = 0;

/** 放在本地定义，测试套件就不需要 node 类型定义，本仓库没有引入它。 */
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

console.log("KeyboardSession settings links and common phrases");

// 皮肤选择器的「更多皮肤」打开「皮肤」页：键盘读不到设置应用下载的社区皮肤，那一页能用全部皮肤。长按工具栏按钮打开「屏幕键盘」页，工具栏的显示方式和工具开关在那里。
group("添加语言、功能面板、皮肤选择器与长按工具栏都能打开对应的设置页", () => {
  check(
    SettingsPageLink.PAGES.length === 6 &&
      SettingsPageLink.accepts("input") &&
      SettingsPageLink.accepts("dictionary") &&
      SettingsPageLink.accepts("feedback") &&
      SettingsPageLink.accepts("about") &&
      SettingsPageLink.accepts("skin") &&
      SettingsPageLink.accepts("screen-keyboard"),
    "exactly the five packages/ui page ids",
  );
  check(!SettingsPageLink.accepts("account"), "another real page is not linked");
  check(!SettingsPageLink.accepts(""), "nor is an empty id");
  check(!SettingsPageLink.accepts("About"), "and ids are matched exactly");
});

group("a Want's page parameter is honoured only for a linked page", () => {
  check(SettingsPageLink.fromParameter("input") === "input", "添加语言的输入页通过 Want 校验");
  check(SettingsPageLink.fromParameter("feedback") === "feedback", "a linked id passes through");
  check(SettingsPageLink.fromParameter(undefined) === null, "no parameter is no page");
  check(SettingsPageLink.fromParameter(null) === null, "nor is a null one");
  check(SettingsPageLink.fromParameter(7) === null, "a number is refused");
  check(SettingsPageLink.fromParameter(["about"]) === null, "and so is an array");
  check(SettingsPageLink.fromParameter("developer") === null, "an unlinked id is refused");
});

group("the starters are Android's, in its order", () => {
  check(CommonPhraseStarters.TEXTS.length === 8, "eight starters");
  check(
    CommonPhraseStarters.TEXTS.join("|") ===
      "好的，收到|我在开会，稍后回复你|马上到|辛苦了，谢谢！|稍等，我马上回来|方便的时候回个电话|周末一起吃饭吗？|已处理，请查收",
    "the same texts as Android's CommonPhrasesStore.STARTER_PHRASES",
  );
  check(
    CommonPhraseStarters.TEXTS.every((text: string): boolean => !text.includes("@")),
    "no sample e-mail address",
  );
  check(
    CommonPhraseStarters.TEXTS.every((text: string): boolean => !/[\r\n]/.test(text)),
    "none breaks a line, so the marker can list them one per line",
  );
  check(
    new Set<string>(CommonPhraseStarters.TEXTS).size === CommonPhraseStarters.TEXTS.length,
    "no duplicates, which the shared layer would refuse",
  );
});

group("only the first load of an empty list seeds", () => {
  check(CommonPhraseStarters.seeds(false, 0, 0), "no marker and nothing stored seeds");
  check(!CommonPhraseStarters.seeds(true, 0, 0), "a list the user emptied stays empty");
  check(
    !CommonPhraseStarters.seeds(false, 1, 0),
    "a list with the user's own phrases is left alone",
  );
  check(!CommonPhraseStarters.seeds(false, 0, 1), "and so is one with only a phrase pack");
});

group("the marker records the starters that were added", () => {
  check(CommonPhraseStarters.MARKER_FILE_NAME === "common-phrases-seeded", "its file name");
  check(CommonPhraseStarters.marker([]) === "", "nothing seeded is an empty marker");
  check(
    CommonPhraseStarters.marker(["马上到", "已处理，请查收"]) === "马上到\n已处理，请查收",
    "one starter per line",
  );
});

// 工具栏标点按钮、功能面板开关显示的是实际输出的标点：锁定时是锁定值，不是开关（KeyboardSession 的 usesChinesePunctuation、refreshToolbar）。
group("a pinned punctuation lock is what the toolbar face and the switch show", () => {
  check(PunctuationLockPolicy.effectiveChinese(true, "follow"), "following shows the switch (on)");
  check(
    !PunctuationLockPolicy.effectiveChinese(false, "follow"),
    "following shows the switch (off)",
  );
  check(
    !PunctuationLockPolicy.effectiveChinese(true, "english"),
    "始终英文标点 shows English even with the switch on",
  );
  check(
    PunctuationLockPolicy.effectiveChinese(false, "chinese"),
    "始终中文标点 shows Chinese even with the switch off",
  );
});

// 锁定时工具栏和 Ctrl+. 的切换什么都不做，也就不写会上云的 chinese_punctuation（KeyboardSession 的 togglePunctuation），与 Windows TIP、fcitx5 相同。
group("the punctuation toggle does nothing while the lock pins it", () => {
  check(PunctuationLockPolicy.toggles("follow"), "following toggles");
  check(!PunctuationLockPolicy.toggles("chinese"), "a Chinese lock does not");
  check(!PunctuationLockPolicy.toggles("english"), "an English lock does not");
});

console.log("");
if (failures > 0) {
  throw new Error(`${failures} group(s) failed`);
}
console.log(`all groups passed (${checks} assertions)`);
