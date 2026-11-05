/**
 * 手机长按候选打开的释义卡片（#10915）：查询文档怎么收窄到这一个候选、每种语言列哪个来源、两个按钮交出什么、什么时候显示「暂无译文」和设置提示、卡片什么时候算过期，以及原来长按菜单里的操作原样进卡片。
 *
 * 卡片的视图和会话接线是 ArkTS，只由 `hvigorw` 编译；这里检查它们取用的 `CandidateGlossCardPolicy`。
 */
import {
  CandidateGlossCardPolicy,
  GlossCard,
  GlossCardAnswer,
  GlossCardLookup,
  GLOSS_CARD_COPIED,
  GLOSS_CARD_EMPTY,
  GLOSS_CARD_PENDING,
} from "../entry/src/main/ets/keyboard/candidate/CandidateGlossCardPolicy";
import {
  CandidateManagementAction,
  ManagementAction,
} from "../entry/src/main/ets/keyboard/candidate/CandidateManagementAction";
import {
  TranslationCandidate,
  TranslationQuery,
} from "../entry/src/main/ets/keyboard/candidate/TranslationPolicy";
import {
  PASTEBOARD_BUSY_CODE,
  PASTEBOARD_PROHIBITED_CODE,
} from "../entry/src/main/ets/keyboard/emoji/MascotStickerPolicy";

let failures = 0;
let checks = 0;

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
  if (!condition) throw new Error(message);
}

const SUBJECT: TranslationCandidate = { text: "苹果", source: 0 };

function answer(
  target: string,
  values: { english?: string; offline?: string; online?: string; wanted?: boolean } = {},
): GlossCardAnswer {
  return {
    target: target,
    english: values.english ?? null,
    offline: values.offline ?? null,
    online: values.online ?? null,
    wanted: values.wanted ?? false,
  };
}

function lookup(provider: string, answers: GlossCardAnswer[]): GlossCardLookup {
  return { provider: provider, answers: answers };
}

function card(value: GlossCardLookup | null, actions: ManagementAction[] = []): GlossCard {
  return CandidateGlossCardPolicy.card("strip", 0, "苹果", SUBJECT, value, actions, 1);
}

function baseQuery(): TranslationQuery {
  return {
    generation: 7,
    target_language: "en",
    target_languages: ["en", "ja"],
    candidates: [
      { text: "苹果", source: 0 },
      { text: "平果", source: 0 },
    ],
    custom_translation: null,
    tencent_tmt: { enabled: true, secret_id: "id", secret_key: "key", region: "ap-guangzhou" },
    niutrans: null,
    english_gloss: false,
    resources: "/data/resources",
    offline_gloss_languages: ["ja"],
  };
}

group("the card asks about the long-pressed candidate alone, in every target language", () => {
  const query: TranslationQuery = baseQuery();
  const shaped: TranslationQuery | null = CandidateGlossCardPolicy.cardQuery(query, SUBJECT);
  check(shaped !== null, "a candidate with a gloss source gets a query");
  check(
    shaped!.candidates.length === 1 && shaped!.candidates[0].text === "苹果",
    "only the long-pressed candidate is asked about",
  );
  check(
    JSON.stringify(shaped!.target_languages) === '["en","ja"]',
    "the second target language stays: the phone strip drops it, the card lists both",
  );
  check(shaped!.tencent_tmt?.enabled === true, "the user's own service configuration is kept");
  check(
    shaped!.sentence === false && shaped!.gloss_query === null,
    "never a /fy sentence or a nested query",
  );
  check(query.candidates.length === 2, "the shared query is not modified");
});

group("the highlighted-only scope's offline glosses are folded back in", () => {
  const query: TranslationQuery = baseQuery();
  query.target_languages = ["en"];
  query.offline_gloss_languages = undefined;
  query.gloss_query = {
    generation: 7,
    target_language: "en",
    target_languages: ["en", "fr"],
    candidates: [{ text: "苹果", source: 0 }],
    english_gloss: true,
    resources: "/data/resources",
    offline_gloss_languages: ["fr"],
  };
  const shaped: TranslationQuery | null = CandidateGlossCardPolicy.cardQuery(query, SUBJECT);
  check(shaped !== null, "a query");
  check(
    JSON.stringify(shaped!.target_languages) === '["en","fr"]',
    "the gloss query's target joins",
  );
  check(
    JSON.stringify(shaped!.offline_gloss_languages) === '["fr"]',
    "its offline dictionary joins",
  );
  check(shaped!.english_gloss, "the offline English gloss switch joins");
});

group("candidates that never carry a gloss are not looked up", () => {
  check(
    CandidateGlossCardPolicy.cardQuery(baseQuery(), { text: "😀", source: 6 }) === null,
    "an emoji-like source gets no query",
  );
  check(
    CandidateGlossCardPolicy.cardQuery(baseQuery(), { text: "", source: 0 }) === null,
    "an empty candidate gets no query",
  );
});

group("the online service is asked by the strip's rules", () => {
  check(!CandidateGlossCardPolicy.asksProvider("", "ja", false, null), "no service, no request");
  check(
    CandidateGlossCardPolicy.asksProvider("tencent", "ja", false, null),
    "a service and no answer",
  );
  check(
    !CandidateGlossCardPolicy.asksProvider("tencent", "en", true, "apple"),
    "English already answered by the offline English gloss is not sent",
  );
  check(
    CandidateGlossCardPolicy.asksProvider("tencent", "en", true, null),
    "English the offline gloss did not cover is sent",
  );
});

group("each language lists one source, in the strip's order of precedence", () => {
  const english = CandidateGlossCardPolicy.section(
    answer("en", { english: "apple", online: "Apple" }),
    "tencent",
  );
  check(
    english !== null && english.sourceName === "离线释义",
    "the offline English gloss comes first",
  );
  const online = CandidateGlossCardPolicy.section(
    answer("ja", { offline: "りんご", online: "リンゴ" }),
    "niutrans",
  );
  check(
    online !== null && online.sourceName === "小牛翻译" && online.senses[0] === "リンゴ",
    "the user's own service outranks the offline dictionary",
  );
  check(online!.sourceLine === "日语 · 小牛翻译", "the source line is language · source");
  const filled = CandidateGlossCardPolicy.section(answer("ja", { offline: "りんご" }), "niutrans");
  check(
    filled !== null && filled.sourceName === "离线释义",
    "the dictionary fills what the service left",
  );
  check(
    CandidateGlossCardPolicy.section(answer("ja"), "niutrans") === null,
    "no answer, no section",
  );
});

group("several offline senses are listed one by one", () => {
  const section = CandidateGlossCardPolicy.section(
    answer("en", { offline: "apple; apple tree / pome" }),
    "",
  );
  check(section !== null, "a section");
  check(
    JSON.stringify(section!.senses) === '["apple","apple tree","pome"]',
    "split on semicolons and on the ` / ` that joins two glosses",
  );
  check(
    JSON.stringify(CandidateGlossCardPolicy.senseLines(section!)) ===
      '["1. apple","2. apple tree","3. pome"]',
    "numbered when there are several",
  );
  const single = CandidateGlossCardPolicy.section(answer("en", { online: "apple" }), "custom");
  check(
    JSON.stringify(CandidateGlossCardPolicy.senseLines(single!)) === '["apple"]',
    "one sense is not numbered",
  );
  check(
    section!.accessibilityText === "英语：apple；apple tree；pome。来源：离线释义",
    "the screen reader hears the language, every sense and the source",
  );
});

group("every source and language has its Chinese name", () => {
  const names: [string, string][] = [
    ["offline", "离线释义"],
    ["account", "水杉账号"],
    ["tencent", "腾讯翻译"],
    ["niutrans", "小牛翻译"],
    ["custom", "自定义服务"],
    ["ai", "AI 辅助"],
  ];
  for (const [source, name] of names) {
    check(CandidateGlossCardPolicy.sourceName(source) === name, `${source} is ${name}`);
  }
  const languages: [string, string][] = [
    ["en", "英语"],
    ["fr", "法语"],
    ["ja", "日语"],
    ["es", "西班牙语"],
    ["ru", "俄语"],
    ["de", "德语"],
    ["ko", "韩语"],
  ];
  for (const [code, name] of languages) {
    check(CandidateGlossCardPolicy.languageName(code) === name, `${code} is ${name}`);
  }
});

group("both buttons hand over the first target language's first translation", () => {
  const both = card(
    lookup("tencent", [
      answer("en", { offline: "apple; apple tree" }),
      answer("ja", { online: "りんご" }),
    ]),
  );
  check(both.sections.length === 2, "both target languages are listed");
  check(both.translation === "apple", "the first sense of the first target language");
  const secondOnly = card(lookup("tencent", [answer("en"), answer("ja", { online: "りんご" })]));
  check(secondOnly.sections.length === 1, "the second language is still shown");
  check(
    secondOnly.translation === null,
    "the buttons stay off rather than hand over the second language's translation",
  );
  check(card(null).translation === null, "no lookup, nothing to hand over");
});

group("the empty card says so, and points to the settings only when no service is on", () => {
  const off = card(null);
  check(CandidateGlossCardPolicy.emptyText(off) === GLOSS_CARD_EMPTY, "暂无译文");
  check(
    CandidateGlossCardPolicy.showsHint(off),
    "translation switched off: the settings hint shows",
  );
  const offline = card(lookup("", [answer("en")]));
  check(CandidateGlossCardPolicy.showsHint(offline), "no online service: the hint shows");
  const asking = card(lookup("tencent", [answer("en", { wanted: true })]));
  check(asking.pending, "a wanted language is pending");
  check(
    CandidateGlossCardPolicy.emptyText(asking) === GLOSS_CARD_PENDING,
    "while asking it says so",
  );
  check(!CandidateGlossCardPolicy.showsHint(asking), "no hint while asking");
  const unanswered = CandidateGlossCardPolicy.withLookup(
    asking,
    CandidateGlossCardPolicy.settled(lookup("tencent", [answer("en", { wanted: true })])),
  );
  check(!unanswered.pending, "a settled lookup is no longer pending");
  check(
    CandidateGlossCardPolicy.emptyText(unanswered) === GLOSS_CARD_EMPTY,
    "暂无译文 after asking",
  );
  check(
    !CandidateGlossCardPolicy.showsHint(unanswered),
    "a service is on, so telling the user to turn one on would be wrong",
  );
});

group("an online answer replaces the pending state on the same card", () => {
  const asking: GlossCardLookup = lookup("custom", [answer("en", { wanted: true }), answer("ja")]);
  const opened = card(asking);
  const fetched: GlossCardLookup = {
    provider: "custom",
    answers: [CandidateGlossCardPolicy.answered(asking.answers[0], "apple"), asking.answers[1]],
  };
  const updated = CandidateGlossCardPolicy.withLookup(opened, fetched);
  check(updated.serial === opened.serial && updated.index === opened.index, "the same card");
  check(!updated.pending, "nothing pending");
  check(
    updated.translation === "apple" && updated.sections[0].sourceName === "自定义服务",
    "the answer",
  );
  check(
    CandidateGlossCardPolicy.answered(answer("en", { online: "cached" }), null).online === "cached",
    "a failed request keeps what was already there",
  );
});

group("answers are read for the candidate and checked like any translation", () => {
  const entries = [
    { text: "平果", translation: "Pingguo" },
    { text: "苹果", translation: "  apple  " },
  ];
  check(CandidateGlossCardPolicy.translationOf(entries, "苹果") === "apple", "trimmed");
  check(CandidateGlossCardPolicy.translationOf(entries, "香蕉") === null, "absent");
  check(
    CandidateGlossCardPolicy.translationOf([{ text: "苹果", translation: "a\u0007b" }], "苹果") ===
      null,
    "control characters are refused",
  );
});

group("the card closes once its candidate is gone", () => {
  const opened = card(null);
  check(CandidateGlossCardPolicy.stillShowing(opened, "strip", ["苹果", "平果"]), "still there");
  check(
    !CandidateGlossCardPolicy.stillShowing(opened, "strip", ["平果", "苹果"]),
    "another candidate there",
  );
  check(
    !CandidateGlossCardPolicy.stillShowing(opened, "strip", []),
    "the strip emptied (committed)",
  );
  check(!CandidateGlossCardPolicy.stillShowing(opened, "expanded", ["苹果"]), "another list");
});

group("the old long-press menu stays reachable from the card", () => {
  const actions: ManagementAction[] = CandidateManagementAction.managementActions(
    "apple",
    true,
    0,
    true,
    true,
    "苹果",
    "apple",
  );
  const opened = card(null, actions);
  const ids: string[] = opened.actions.map((action: ManagementAction): string => action.id);
  for (const id of [
    "INSERT_GLOSS",
    "PROMOTE",
    "FIX_1",
    "FIX_5",
    "CLEAR_POSITION",
    "REMOVE",
    "READ_ALOUD",
    "ADD_TO_PERSONAL_WORDBOOK",
  ]) {
    check(ids.includes(id), `${id} is among the card's actions`);
  }
  check(
    JSON.stringify(ids) ===
      JSON.stringify(actions.map((action: ManagementAction): string => action.id)),
    "in the menu's own order",
  );
  const refreshed = CandidateGlossCardPolicy.withActions(opened, actions.slice(1));
  check(refreshed.actions.length === actions.length - 1, "the strip's redraw replaces them");
  check(refreshed.serial === opened.serial, "on the same card");
});

group(
  "the keys under the card, like the keys under the adjust overlay, are hidden from the screen reader",
  () => {
    check(
      CandidateGlossCardPolicy.keysAccessibilityLevel(false, false) === "auto",
      "nothing over the keys",
    );
    check(
      CandidateGlossCardPolicy.keysAccessibilityLevel(true, false) === "no-hide-descendants",
      "adjusting the keyboard",
    );
    check(
      CandidateGlossCardPolicy.keysAccessibilityLevel(false, true) === "no-hide-descendants",
      "the card is open",
    );
  },
);

group("the notices after the two buttons", () => {
  check(CandidateGlossCardPolicy.commitAnnouncement("apple") === "已输入 apple", "commit");
  check(GLOSS_CARD_COPIED === "已复制译文", "copied");
  check(
    CandidateGlossCardPolicy.copyFailureNotice(PASTEBOARD_BUSY_CODE).includes("剪贴板正忙"),
    "busy clipboard",
  );
  check(
    CandidateGlossCardPolicy.copyFailureNotice(PASTEBOARD_PROHIBITED_CODE).includes("不允许"),
    "prohibited",
  );
  check(
    CandidateGlossCardPolicy.copyFailureNotice(undefined) === "复制失败，请重试",
    "anything else",
  );
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} candidate gloss card checks failed`);
