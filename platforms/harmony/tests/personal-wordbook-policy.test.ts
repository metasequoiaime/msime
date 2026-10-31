import {
  KEYBOARD_PERSONAL_WORDBOOK_DRAINED_PATH,
  KEYBOARD_PERSONAL_WORDBOOK_PATH,
  PERSONAL_WORDBOOK_BASIC_MODE_HINT,
  PERSONAL_WORDBOOK_FAILURE,
  PERSONAL_WORDBOOK_UNSHARED_HINT,
  PersonalWordbookPolicy,
} from "../entry/src/main/ets/keyboard/PersonalWordbookPolicy";
import {
  ABSENT_FINGERPRINT,
  ImportedEntry,
  KEYBOARD_ENTRIES,
  MirrorManifest,
  SETTINGS_ENTRIES,
  SharedStateMirrorPolicy,
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

group(
  "the shared layer's notice is shown, with a hint that matches why settings cannot see it",
  () => {
    check(
      PersonalWordbookPolicy.notice(reply("added", "已加入生词本"), "settings") === "已加入生词本",
      "shared",
    );
    check(
      PersonalWordbookPolicy.notice(reply("added", "已加入生词本"), "basic_mode") ===
        `已加入生词本${PERSONAL_WORDBOOK_BASIC_MODE_HINT}`,
      "basic mode",
    );
    check(
      PersonalWordbookPolicy.notice(
        reply("already_present", "已在生词本里"),
        "no_shared_sandbox",
      ) === `已在生词本里${PERSONAL_WORDBOOK_UNSHARED_HINT}`,
      "no shared sandbox",
    );
    check(
      !PERSONAL_WORDBOOK_UNSHARED_HINT.includes("完整体验模式"),
      "without a shared sandbox full mode does not help, so the hint must not promise it",
    );
    check(
      PersonalWordbookPolicy.notice(reply("full", "生词本已满"), "basic_mode") === "生词本已满",
      "a refusal gets no hint",
    );
    check(
      PersonalWordbookPolicy.notice(JSON.stringify({ ok: false, error: "x" }), "settings") ===
        PERSONAL_WORDBOOK_FAILURE,
      "storage failure",
    );
    check(
      PersonalWordbookPolicy.notice("not json", "settings") === PERSONAL_WORDBOOK_FAILURE,
      "malformed",
    );
    check(PersonalWordbookPolicy.route(false, true) === "no_shared_sandbox", "no sandbox wins");
    check(PersonalWordbookPolicy.route(true, false) === "basic_mode", "basic mode");
    check(PersonalWordbookPolicy.route(true, true) === "settings", "reaches settings");
  },
);

group(
  "only the keyboard publishes its personal wordbook and drain record, and only in full mode",
  () => {
    const names = KEYBOARD_ENTRIES.map((item) => item.name);
    const entry = KEYBOARD_ENTRIES.find((item) => item.name === "personal-wordbook");
    const drained = KEYBOARD_ENTRIES.find((item) => item.name === "personal-wordbook-drained");
    check(entry !== undefined && drained !== undefined, "missing keyboard entry");
    check(
      entry!.kind === "file" &&
        entry!.base === "state" &&
        entry!.path === KEYBOARD_PERSONAL_WORDBOOK_PATH,
      JSON.stringify(entry),
    );
    check(
      drained!.kind === "file" &&
        drained!.base === "state" &&
        drained!.path === KEYBOARD_PERSONAL_WORDBOOK_DRAINED_PATH,
      JSON.stringify(drained),
    );
    check(entry!.fullModeOnly && drained!.fullModeOnly, "basic mode is read-only for the keyboard");
    check(
      names.indexOf("personal-wordbook") < names.indexOf("personal-wordbook-drained"),
      "the book is copied before the drain record, so a new record means a new book",
    );
    check(
      !SETTINGS_ENTRIES.some((item) => item.name.startsWith("personal-wordbook")),
      "settings never publishes them",
    );
  },
);

group(
  "the merge request carries file paths and the previous version only when there is one",
  () => {
    const first = JSON.parse(
      PersonalWordbookPolicy.mergeRequest("/state", "/state/staging.tmp", null),
    ) as Record<string, string>;
    check(
      first.directory === "/state" &&
        first.published === "/state/staging.tmp" &&
        !("previous" in first),
      JSON.stringify(first),
    );
    const second = JSON.parse(
      PersonalWordbookPolicy.mergeRequest("/state", "/b.json", "/a.json"),
    ) as Record<string, string>;
    check(second.previous === "/a.json", JSON.stringify(second));
    const merged = (overLimit: number): string =>
      JSON.stringify({ ok: true, value: { added: 1, overLimit: overLimit } });
    check(PersonalWordbookPolicy.mergeAccepted(merged(0)), "ok");
    check(
      !PersonalWordbookPolicy.mergeAccepted(merged(2)),
      "words dropped because settings is full are not recorded as merged",
    );
    check(!PersonalWordbookPolicy.mergeAccepted(JSON.stringify({ ok: false })), "refused");
    check(!PersonalWordbookPolicy.mergeAccepted("broken"), "malformed");
  },
);

group("the keyboard drains its own book with a request it can read back", () => {
  const request = JSON.parse(
    PersonalWordbookPolicy.drainRequest("/state", "/group/msime/keyboard/state/x.json"),
  ) as Record<string, string>;
  check(
    request.directory === "/state/keyboard-vocabulary" &&
      request.published === "/group/msime/keyboard/state/x.json",
    JSON.stringify(request),
  );
  check(
    PersonalWordbookPolicy.drained(JSON.stringify({ ok: true, value: { removed: 3 } })) === 3,
    "removed",
  );
  check(PersonalWordbookPolicy.drained(JSON.stringify({ ok: false })) === -1, "refused");
  check(PersonalWordbookPolicy.drained("broken") === -1, "malformed");
});

group(
  "every drain writes a different record, and settings drops its previous copy on a match",
  () => {
    const first = PersonalWordbookPolicy.nextDrainedMarker(null, "fp-a");
    const second = PersonalWordbookPolicy.nextDrainedMarker(first, "fp-a");
    check(first !== second, "draining the same content twice still changes the record");
    check(PersonalWordbookPolicy.parseDrainedMarker(second)?.drains === 2, second);
    check(
      PersonalWordbookPolicy.parseDrainedMarker(
        PersonalWordbookPolicy.nextDrainedMarker("junk", "fp"),
      )?.drains === 1,
      "an unreadable record starts over",
    );
    check(PersonalWordbookPolicy.unseenDrain(second, first), "a newer record is news");
    check(PersonalWordbookPolicy.unseenDrain(first, null), "the first record is news");
    check(
      !PersonalWordbookPolicy.unseenDrain(second, second),
      "a record already used is not compared again, even if an equal book comes back later",
    );
    check(
      !PersonalWordbookPolicy.unseenDrain(null, null) &&
        !PersonalWordbookPolicy.unseenDrain("junk", null),
      "no record or an unreadable one",
    );
    check(PersonalWordbookPolicy.previousDrained(second, "fp-a"), "matching record");
    check(!PersonalWordbookPolicy.previousDrained(second, "fp-b"), "another version");
    check(!PersonalWordbookPolicy.previousDrained(null, "fp-a"), "no record");
    check(
      !PersonalWordbookPolicy.previousDrained('{"drains":-1,"fingerprint":"fp-a"}', "fp-a"),
      "malformed record",
    );
  },
);

const manifest = (
  publisher: "settings" | "keyboard",
  entries: Array<[string, string]>,
  applied: ImportedEntry[] = [],
): MirrorManifest => ({
  format: 1,
  publisher: publisher,
  entries: entries.map(([name, fingerprint]) => ({ name, fingerprint, changedMs: 1 })),
  applied: applied,
});

group("the keyboard drains only what settings confirmed, after settings saw the last drain", () => {
  const applied = (book: string, marker?: string): ImportedEntry[] =>
    [
      { name: "preferences", fingerprint: "p" },
      { name: "personal-wordbook", fingerprint: book },
    ].concat(
      marker === undefined ? [] : [{ name: "personal-wordbook-drained", fingerprint: marker }],
    );
  const keyboard = manifest("keyboard", [["personal-wordbook", "b1"]]);
  check(
    SharedStateMirrorPolicy.personalWordbookDrainable(
      keyboard,
      manifest("settings", [], applied("b1")),
    ) === "b1",
    "settings merged the version the keyboard publishes",
  );
  check(
    SharedStateMirrorPolicy.personalWordbookDrainable(
      keyboard,
      manifest("settings", [], applied("b0")),
    ) === null,
    "settings merged an older version",
  );
  check(
    SharedStateMirrorPolicy.personalWordbookDrainable(keyboard, null) === null &&
      SharedStateMirrorPolicy.personalWordbookDrainable(
        null,
        manifest("settings", [], applied("b1")),
      ) === null,
    "no manifest",
  );
  check(
    SharedStateMirrorPolicy.personalWordbookDrainable(
      manifest("keyboard", [["personal-wordbook", ABSENT_FINGERPRINT]]),
      manifest("settings", [], applied(ABSENT_FINGERPRINT)),
    ) === null,
    "an empty keyboard book has nothing to drain",
  );
  // 清走以后又加回同样的词：生词本的指纹和上一轮相同，但设置应用还没看到这次清走，不能再清。
  const afterDrain = manifest("keyboard", [
    ["personal-wordbook", "b1"],
    ["personal-wordbook-drained", "m2"],
  ]);
  check(
    SharedStateMirrorPolicy.personalWordbookDrainable(
      afterDrain,
      manifest("settings", [], applied("b1", "m1")),
    ) === null,
    "settings has not seen the latest drain",
  );
  check(
    SharedStateMirrorPolicy.personalWordbookDrainable(
      afterDrain,
      manifest("settings", [], applied("b1")),
    ) === null,
    "settings has never seen a drain",
  );
  check(
    SharedStateMirrorPolicy.personalWordbookDrainable(
      afterDrain,
      manifest("settings", [], applied("b1", "m2")),
    ) === "b1",
    "settings saw the latest drain and merged this version",
  );
  const record = {
    format: 1,
    entries: [
      { name: "preferences", fingerprint: "p" },
      { name: "personal-wordbook", fingerprint: "b1" },
      { name: "personal-wordbook-drained", fingerprint: "m2" },
    ],
  };
  check(
    SharedStateMirrorPolicy.personalWordbookApplied(record)
      .map((item) => item.name)
      .join(",") === "personal-wordbook,personal-wordbook-drained",
    "settings acknowledges only the personal wordbook entries",
  );
  check(SharedStateMirrorPolicy.personalWordbookApplied(null).length === 0, "nothing imported yet");
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} personal wordbook policy checks failed`);
