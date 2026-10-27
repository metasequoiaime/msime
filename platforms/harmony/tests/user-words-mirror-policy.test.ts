import {
  KEYBOARD_MAX_CHANGES,
  KEYBOARD_SYNC_ATTEMPTS,
  KEYBOARD_PUBLISH_INTERVAL_MS,
  SETTINGS_MAX_CHANGES,
  UserWordsMirrorPolicy,
  UserWordsReply,
} from "../entry/src/main/ets/keyboard/UserWordsMirrorPolicy";

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

const STATE = "/data/storage/el2/base/haps/entry/files/state";
const GROUP = "/data/storage/el2/group/synthetic/msime";
const OPTIONS = '{"api_version":1,"resources":"/r","preferences":{"scheme":"quanpin"}}';

function reply(value: Record<string, unknown>): UserWordsReply {
  return UserWordsMirrorPolicy.parse(JSON.stringify({ ok: true, value: value }));
}

console.log("Harmony user words mirror policy");

group("both sides publish beside the existing mirror and keep state in their own sandbox", () => {
  check(
    UserWordsMirrorPolicy.directory(STATE) === `${STATE}/user-words`,
    "the private directory sits under files/state",
  );
  check(
    UserWordsMirrorPolicy.publication(GROUP, "settings") ===
      `${GROUP}/settings/user-words/words.json`,
    "the settings app publishes under its own publisher root",
  );
  check(
    UserWordsMirrorPolicy.publication(GROUP, "keyboard") ===
      `${GROUP}/keyboard/user-words/words.json`,
    "the keyboard publishes under its own publisher root",
  );
  const journal = UserWordsMirrorPolicy.journalFiles(STATE);
  check(
    journal[0] === `${STATE}/user/msime_user.db` &&
      journal[1] === `${STATE}/user/msime_user.db-wal`,
    "the journal and its WAL are what changes when words are learned",
  );
});

group("a sync request embeds the host options and names both files", () => {
  const theirs = UserWordsMirrorPolicy.publication(GROUP, "settings");
  const own = UserWordsMirrorPolicy.publication(GROUP, "keyboard");
  const text = UserWordsMirrorPolicy.request(
    "sync",
    "keyboard",
    OPTIONS,
    STATE,
    theirs,
    own,
    KEYBOARD_MAX_CHANGES,
  );
  const request = JSON.parse(text) as Record<string, unknown>;
  check(request.operation === "sync" && request.side === "keyboard", "operation and side");
  check(
    JSON.stringify(request.options) === OPTIONS,
    "the options document is passed through unchanged",
  );
  check(request.directory === `${STATE}/user-words`, "the private directory");
  check(request.theirs === theirs && request.publish === own, "both publications");
  check(request.max_changes === KEYBOARD_MAX_CHANGES, "the keyboard writes one batch at a time");
  check(
    Object.keys(request).length === 7,
    "nothing Rust's deny_unknown_fields would reject is added",
  );
});

group("the keyboard in basic access mode receives without publishing", () => {
  const request = JSON.parse(
    UserWordsMirrorPolicy.request(
      "sync",
      "keyboard",
      OPTIONS,
      STATE,
      `${GROUP}/settings/user-words/words.json`,
      null,
      KEYBOARD_MAX_CHANGES,
    ),
  ) as Record<string, unknown>;
  check(
    !("publish" in request),
    "no publication path, so nothing is written to the group directory",
  );
  check(typeof request.theirs === "string", "it still reads what the settings app published");
});

group("a publish request neither reads the other side nor writes the dictionary", () => {
  const request = JSON.parse(
    UserWordsMirrorPolicy.request(
      "publish",
      "keyboard",
      OPTIONS,
      STATE,
      `${GROUP}/settings/user-words/words.json`,
      `${GROUP}/keyboard/user-words/words.json`,
      KEYBOARD_MAX_CHANGES,
    ),
  ) as Record<string, unknown>;
  check(request.operation === "publish", "publish");
  check(!("theirs" in request), "publish never merges, so it is not handed the other file");
  check(!("max_changes" in request), "publish writes nothing to the dictionary");
  check(SETTINGS_MAX_CHANGES > KEYBOARD_MAX_CHANGES, "the settings app works on a worker thread");
});

group("replies are read strictly and an unreadable one is retried", () => {
  const ok = reply({
    captured: 2,
    applied: 3,
    rejected: 1,
    deferred: 0,
    held_deletions: 0,
    words: 9,
    theirs: "read",
    published: true,
  });
  check(ok.ok && ok.captured === 2 && ok.applied === 3 && ok.rejected === 1, "counts");
  check(ok.words === 9 && ok.theirs === "read" && ok.published, "the rest");
  const odd = reply({ applied: -1, deferred: 1.5, published: "yes" });
  check(
    odd.ok && odd.applied === 0 && odd.deferred === 0 && !odd.published,
    "bad numbers read as 0",
  );
  const refused = UserWordsMirrorPolicy.parse('{"ok":false,"error":"user_words_busy"}');
  check(!refused.ok && refused.error === "user_words_busy", "the error code is kept");
  const garbage = UserWordsMirrorPolicy.parse("not json");
  check(!garbage.ok && garbage.error === "user_words_storage", "garbage is a storage failure");
  check(!UserWordsMirrorPolicy.settled(garbage), "and is tried again");
});

group("only a complete exchange, or one that cannot succeed by retrying, is settled", () => {
  check(UserWordsMirrorPolicy.settled(reply({ deferred: 0 })), "done");
  check(!UserWordsMirrorPolicy.settled(reply({ deferred: 4 })), "words left for next time");
  for (const error of ["user_words_busy", "user_words_unavailable", "user_words_storage"]) {
    check(
      !UserWordsMirrorPolicy.settled(
        UserWordsMirrorPolicy.parse(JSON.stringify({ ok: false, error })),
      ),
      `${error} is retried`,
    );
  }
  for (const error of ["user_words_too_large", "invalid_user_words_mirror"]) {
    check(
      UserWordsMirrorPolicy.settled(
        UserWordsMirrorPolicy.parse(JSON.stringify({ ok: false, error })),
      ),
      `${error} is not retried in a loop`,
    );
  }
});

group("the keyboard stops rebuilding its session for an exchange that keeps failing", () => {
  const busy = UserWordsMirrorPolicy.parse('{"ok":false,"error":"user_words_storage"}');
  let outcome = UserWordsMirrorPolicy.keyboardOutcome(busy, 0);
  check(!outcome.settled && outcome.failures === 1, "the first failure is retried");
  outcome = UserWordsMirrorPolicy.keyboardOutcome(busy, outcome.failures);
  check(!outcome.settled && outcome.failures === 2, "and the second");
  outcome = UserWordsMirrorPolicy.keyboardOutcome(busy, outcome.failures);
  check(
    outcome.settled && outcome.failures === 0 && KEYBOARD_SYNC_ATTEMPTS === 3,
    "the third gives up until settings publishes again",
  );
  const partial = UserWordsMirrorPolicy.keyboardOutcome(reply({ applied: 128, deferred: 40 }), 2);
  check(!partial.settled && partial.failures === 0, "progress keeps going and clears the count");
  check(UserWordsMirrorPolicy.keyboardOutcome(reply({}), 2).settled, "done");
  check(
    UserWordsMirrorPolicy.keyboardOutcome(
      UserWordsMirrorPolicy.parse('{"ok":false,"error":"user_words_too_large"}'),
      0,
    ).settled,
    "an error retrying cannot fix is settled at once",
  );
});

group("the keyboard syncs at session start only when there is something to do", () => {
  check(UserWordsMirrorPolicy.keyboardSyncDue(true, "a", "a"), "a pass left unfinished");
  check(UserWordsMirrorPolicy.keyboardSyncDue(false, null, "-"), "never done in this process");
  check(
    !UserWordsMirrorPolicy.keyboardSyncDue(false, "1:2:3", "1:2:3"),
    "nothing new from settings",
  );
  check(UserWordsMirrorPolicy.keyboardSyncDue(false, "1:2:3", "4:5:6"), "settings published again");
});

group("the keyboard publishes on blur only in full mode, after a change, and not too often", () => {
  const now = 1_790_000_000_000;
  check(
    !UserWordsMirrorPolicy.keyboardPublishDue(false, false, null, "j1", 0, now),
    "basic access mode cannot write the group directory",
  );
  check(UserWordsMirrorPolicy.keyboardPublishDue(true, false, null, "j1", 0, now), "first time");
  check(!UserWordsMirrorPolicy.keyboardPublishDue(true, true, null, "j1", 0, now), "one at a time");
  check(
    !UserWordsMirrorPolicy.keyboardPublishDue(true, false, "j1", "j1", now - 600000, now),
    "nothing learned since the last publication",
  );
  check(
    !UserWordsMirrorPolicy.keyboardPublishDue(true, false, "j1", "j2", now - 1000, now),
    "too soon after the last one",
  );
  check(
    UserWordsMirrorPolicy.keyboardPublishDue(
      true,
      false,
      "j1",
      "j2",
      now - KEYBOARD_PUBLISH_INTERVAL_MS,
      now,
    ),
    "a change after the interval",
  );
});

group("the settings app syncs when its journal or the keyboard publication moved", () => {
  const mark = (journal: (number | null)[], keyboard: number | null): string =>
    `${UserWordsMirrorPolicy.mark(
      journal.map((size) => (size === null ? null : { size: size, mtime: 7, ino: "11" })),
    )}#${UserWordsMirrorPolicy.mark([keyboard === null ? null : { size: keyboard, mtime: 8, ino: "12" }])}`;
  const first = mark([100, null], null);
  check(UserWordsMirrorPolicy.settingsSyncDue(null, first), "the first time after start");
  check(!UserWordsMirrorPolicy.settingsSyncDue(first, first), "nothing moved");
  check(UserWordsMirrorPolicy.settingsSyncDue(first, mark([100, 32], null)), "the WAL grew");
  check(
    UserWordsMirrorPolicy.settingsSyncDue(first, mark([100, null], 50)),
    "the keyboard published",
  );
  check(first.includes("-"), "a missing file has its own mark");
});

group(
  "dictionary sync is told when the settings dictionary gained changes it has not recorded",
  () => {
    check(UserWordsMirrorPolicy.dictionaryChanged(reply({ applied: 1 })), "keyboard words written");
    check(
      UserWordsMirrorPolicy.dictionaryChanged(reply({ applied: 0, captured: 5 })),
      "words drained from the settings queue",
    );
    check(
      !UserWordsMirrorPolicy.dictionaryChanged(reply({ applied: 0, captured: 0 })),
      "nothing new",
    );
    check(!UserWordsMirrorPolicy.dictionaryChanged(reply({ rejected: 3 })), "all refused");
    check(
      !UserWordsMirrorPolicy.dictionaryChanged(
        UserWordsMirrorPolicy.parse('{"ok":false,"error":"user_words_busy"}'),
      ),
      "a failure",
    );
  },
);

group(
  "the settings queue is drained until nothing is pending and no collection batch follows",
  () => {
    const personal = (pending: number): string =>
      JSON.stringify({ ok: true, value: { pending_count: pending } });
    const flush = (sent: number): string => JSON.stringify({ ok: true, value: { sent: sent } });
    check(UserWordsMirrorPolicy.queueDrained(personal(0), flush(0)), "empty");
    check(!UserWordsMirrorPolicy.queueDrained(personal(3), flush(0)), "personal requests waiting");
    check(
      !UserWordsMirrorPolicy.queueDrained(personal(0), flush(128)),
      "a collection batch just queued",
    );
    check(
      UserWordsMirrorPolicy.queueDrained(personal(0), '{"ok":false,"error":"collections_io"}'),
      "no collections store has nothing to send",
    );
    check(
      !UserWordsMirrorPolicy.queueDrained(
        '{"ok":false,"error":"personal dictionary is busy"}',
        flush(0),
      ),
      "a busy queue is tried again",
    );
    check(!UserWordsMirrorPolicy.queueDrained("garbage", flush(0)), "garbage is tried again");
    const files = UserWordsMirrorPolicy.settingsQueueFiles(STATE);
    check(
      files.includes(`${STATE}/PersonalDictionary/sync.json`) &&
        files.includes(`${STATE}/DictionaryCollections/outbox.json`),
      "a queued import moves the mark even before the journal changes",
    );
  },
);

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} user words mirror policy checks failed`);
