import {
  KEYBOARD_PERSONAL_WORDBOOK_PATH,
  PERSONAL_WORDBOOK_FAILURE,
  PERSONAL_WORDBOOK_UNSHARED_HINT,
  PersonalWordbookPolicy,
} from "../entry/src/main/ets/keyboard/PersonalWordbookPolicy";
import {
  KEYBOARD_ENTRIES,
  SETTINGS_ENTRIES,
} from "../entry/src/main/ets/keyboard/SharedStateMirrorPolicy";

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

const reply = (outcome: string, notice: string): string =>
  JSON.stringify({ ok: true, value: { outcome: outcome, notice: notice } });

group("the keyboard writes into its own keyboard-vocabulary directory", () => {
  const request = JSON.parse(
    PersonalWordbookPolicy.addRequest("/state", "面试", "interview"),
  ) as Record<string, string>;
  check(request.directory === "/state/keyboard-vocabulary", request.directory);
  check(request.candidate === "面试" && request.gloss === "interview", JSON.stringify(request));
  check(
    KEYBOARD_PERSONAL_WORDBOOK_PATH === "keyboard-vocabulary/vocabulary-wordbooks/personal.json",
    KEYBOARD_PERSONAL_WORDBOOK_PATH,
  );
});

group("the shared layer's notice is shown, with a hint when settings cannot see it yet", () => {
  check(
    PersonalWordbookPolicy.notice(reply("added", "已加入生词本"), true) === "已加入生词本",
    "shared",
  );
  check(
    PersonalWordbookPolicy.notice(reply("added", "已加入生词本"), false) ===
      `已加入生词本${PERSONAL_WORDBOOK_UNSHARED_HINT}`,
    "unshared",
  );
  check(
    PersonalWordbookPolicy.notice(reply("full", "生词本已满"), false) === "生词本已满",
    "a refusal gets no hint",
  );
  check(
    PersonalWordbookPolicy.notice(JSON.stringify({ ok: false, error: "x" }), true) ===
      PERSONAL_WORDBOOK_FAILURE,
    "storage failure",
  );
  check(PersonalWordbookPolicy.notice("not json", true) === PERSONAL_WORDBOOK_FAILURE, "malformed");
});

group("only the keyboard publishes its personal wordbook, and only in full mode", () => {
  const entry = KEYBOARD_ENTRIES.find((item) => item.name === "personal-wordbook");
  check(entry !== undefined, "missing keyboard entry");
  check(
    entry!.kind === "file" &&
      entry!.base === "state" &&
      entry!.path === KEYBOARD_PERSONAL_WORDBOOK_PATH,
    JSON.stringify(entry),
  );
  check(entry!.fullModeOnly === true, "basic mode is read-only for the keyboard");
  check(
    !SETTINGS_ENTRIES.some((item) => item.name === "personal-wordbook"),
    "settings never publishes it",
  );
});

group("the merge request carries the previous version only when there is one", () => {
  const first = JSON.parse(PersonalWordbookPolicy.mergeRequest("/state", "{}", null)) as Record<
    string,
    string
  >;
  check(
    first.directory === "/state" && first.published === "{}" && !("previous" in first),
    JSON.stringify(first),
  );
  const second = JSON.parse(PersonalWordbookPolicy.mergeRequest("/state", "{b}", "{a}")) as Record<
    string,
    string
  >;
  check(second.previous === "{a}", JSON.stringify(second));
  check(PersonalWordbookPolicy.mergeAccepted(JSON.stringify({ ok: true, value: {} })), "ok");
  check(!PersonalWordbookPolicy.mergeAccepted(JSON.stringify({ ok: false })), "refused");
  check(!PersonalWordbookPolicy.mergeAccepted("broken"), "malformed");
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} personal wordbook policy checks failed`);
