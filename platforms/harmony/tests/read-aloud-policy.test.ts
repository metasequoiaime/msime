import { ReadAloudPolicy } from "../entry/src/main/ets/speech/ReadAloudPolicy";

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

group("the engine is the offline built-in Chinese voice and never a downloaded one", () => {
  check(ReadAloudPolicy.OFFLINE === 1, "online 1 is the offline mode");
  check(
    ReadAloudPolicy.ENGINE_LANGUAGE === "zh-CN" && ReadAloudPolicy.ENGINE_PERSON === 13,
    "聆小珊 ships with the system",
  );
  check(ReadAloudPolicy.MENU_TITLE === "朗读", "the menu title matches the other hosts");
  check(
    ReadAloudPolicy.spokenText("interview") === "[h2]interview",
    "the word is read as a word, not spelled out",
  );
});

group("the shared layer's answer is read only when it names something to read", () => {
  check(
    ReadAloudPolicy.textRequest("面试", "interview") === '{"candidate":"面试","gloss":"interview"}',
    "the request carries the candidate and the shown gloss",
  );
  check(
    ReadAloudPolicy.textFromReply('{"ok":true,"value":"interview"}') === "interview",
    "an English answer",
  );
  check(ReadAloudPolicy.textFromReply('{"ok":true,"value":null}') === null, "nothing to read");
  check(
    ReadAloudPolicy.textFromReply('{"ok":false,"error":"invalid read aloud request"}') === null,
    "a refused request",
  );
  check(ReadAloudPolicy.textFromReply('{"ok":true,"value":""}') === null, "an empty answer");
  check(ReadAloudPolicy.textFromReply("not json") === null, "a broken reply");
});

group("the settings page's request carries one short word", () => {
  check(ReadAloudPolicy.requestText('{"text":"ubiquitous"}') === "ubiquitous", "a headword");
  check(ReadAloudPolicy.requestText('{"text":""}') === null, "an empty word");
  check(
    ReadAloudPolicy.requestText(`{"text":"${"a".repeat(65)}"}`) === null,
    "longer than a headword",
  );
  check(ReadAloudPolicy.requestText('{"text":1}') === null, "not text");
  check(ReadAloudPolicy.requestText("[") === null, "not JSON");
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} read aloud policy checks failed`);
