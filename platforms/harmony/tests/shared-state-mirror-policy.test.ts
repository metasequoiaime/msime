import {
  ABSENT_FINGERPRINT,
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

const fp = SharedStateMirrorPolicy.fingerprintText;

function manifest(
  publisher: "settings" | "keyboard",
  entries: [string, string, number][],
  applied: [string, string][] = [],
): MirrorManifest {
  return {
    format: 1,
    publisher: publisher,
    entries: entries.map(([name, fingerprint, changedMs]) => ({ name, fingerprint, changedMs })),
    applied: applied.map(([name, fingerprint]) => ({ name, fingerprint })),
  };
}

console.log("Harmony shared state mirror policy");

group("preferences fingerprints ignore the revision and key order", () => {
  const a = SharedStateMirrorPolicy.preferencesFingerprint(
    JSON.stringify({
      format_version: 1,
      revision: 3,
      preferences: { scheme: "quanpin", learning: true },
    }),
  );
  const b = SharedStateMirrorPolicy.preferencesFingerprint(
    JSON.stringify({
      revision: 9,
      format_version: 1,
      preferences: { learning: true, scheme: "quanpin" },
    }),
  );
  const c = SharedStateMirrorPolicy.preferencesFingerprint(
    JSON.stringify({
      format_version: 1,
      revision: 3,
      preferences: { scheme: "shuangpin", learning: true },
    }),
  );
  check(a === b, "the same preferences written with another revision keep their fingerprint");
  check(a !== c, "a changed preference changes the fingerprint");
  check(
    SharedStateMirrorPolicy.preferencesFingerprint("not json") === fp("not json"),
    "an unreadable document still changes its fingerprint when its text does",
  );
});

group("directory fingerprints follow names, sizes and times but not listing order", () => {
  const one = SharedStateMirrorPolicy.directoryFingerprint([
    { path: "a/skin.toml", size: 10, mtime: 1 },
    { path: "b.png", size: 20, mtime: 2 },
  ]);
  const same = SharedStateMirrorPolicy.directoryFingerprint([
    { path: "b.png", size: 20, mtime: 2 },
    { path: "a/skin.toml", size: 10, mtime: 1 },
  ]);
  const grown = SharedStateMirrorPolicy.directoryFingerprint([
    { path: "a/skin.toml", size: 11, mtime: 1 },
    { path: "b.png", size: 20, mtime: 2 },
  ]);
  check(one === same, "listing order does not matter");
  check(one !== grown, "a resized file changes the fingerprint");
  check(
    SharedStateMirrorPolicy.directoryFingerprint([]) !== ABSENT_FINGERPRINT,
    "an empty directory is not the same as a missing one",
  );
});

group("publishing copies only what changed and keeps the time of each change", () => {
  const first = SharedStateMirrorPolicy.plan(
    null,
    "settings",
    [
      { name: "preferences", fingerprint: "p1" },
      { name: "skins", fingerprint: ABSENT_FINGERPRINT },
    ],
    100,
    [],
  );
  check(
    first.changed.length === 1 && first.changed[0] === "preferences",
    "a never-published missing entry is not announced",
  );
  check(first.manifest.entries.length === 1, "the manifest lists only what exists");
  const second = SharedStateMirrorPolicy.plan(
    first.manifest,
    "settings",
    [
      { name: "preferences", fingerprint: "p1" },
      { name: "skins", fingerprint: "s1" },
    ],
    200,
    [],
  );
  check(
    second.changed.length === 1 && second.changed[0] === "skins",
    "an unchanged entry is not copied again",
  );
  const kept = second.manifest.entries.find((entry) => entry.name === "preferences");
  check(
    kept !== undefined && kept.changedMs === 100,
    "an unchanged entry keeps the time it last changed",
  );
  const removed = SharedStateMirrorPolicy.plan(
    second.manifest,
    "settings",
    [
      { name: "preferences", fingerprint: "p1" },
      { name: "skins", fingerprint: ABSENT_FINGERPRINT },
    ],
    300,
    [],
  );
  check(
    removed.changed.length === 1 && removed.changed[0] === "skins",
    "a published entry that disappears is announced so the other side removes its copy",
  );
  const foreign = SharedStateMirrorPolicy.plan(
    manifest("keyboard", [["preferences", "p1", 1]]),
    "settings",
    [{ name: "preferences", fingerprint: "p1" }],
    400,
    [],
  );
  check(
    foreign.changed.length === 1,
    "another publisher's manifest is not taken as this one's history",
  );
});

group("importing takes entries whose fingerprint moved since the last import", () => {
  const theirs = manifest("settings", [
    ["preferences", "p2", 100],
    ["skins", "s1", 100],
  ]);
  const record = { format: 1, entries: [{ name: "skins", fingerprint: "s1" }] };
  const pending = SharedStateMirrorPolicy.pending(theirs, record, null, SETTINGS_ENTRIES, false);
  check(
    pending.length === 1 && pending[0].name === "preferences",
    "only the changed entry is imported",
  );
  const after = SharedStateMirrorPolicy.recordImport(record, "preferences", "p2");
  check(
    SharedStateMirrorPolicy.pending(theirs, after, null, SETTINGS_ENTRIES, false).length === 0,
    "once recorded, nothing is imported again",
  );
});

group("account sessions are only exchanged in full experience mode", () => {
  const theirs = manifest("settings", [
    ["account-session", "a1", 100],
    ["app-theme", "t1", 100],
  ]);
  const basic = SharedStateMirrorPolicy.pending(theirs, null, null, SETTINGS_ENTRIES, false);
  check(
    basic.length === 1 && basic[0].name === "app-theme",
    "a basic-mode keyboard cannot write back a rotated refresh token, so it does not take the session",
  );
  const full = SharedStateMirrorPolicy.pending(theirs, null, null, SETTINGS_ENTRIES, true);
  check(
    full.some((entry) => entry.name === "account-session"),
    "a full-mode keyboard takes the session",
  );
});

group("when both sides changed an entry the later change wins", () => {
  const keyboard = manifest("keyboard", [["preferences", "k1", 500]]);
  const settingsOlder = manifest("settings", [["preferences", "s1", 300]]);
  const settingsNewer = manifest("settings", [["preferences", "s2", 700]]);
  check(
    SharedStateMirrorPolicy.pending(keyboard, null, settingsOlder, KEYBOARD_ENTRIES, true)
      .length === 1,
    "settings takes the keyboard's later change",
  );
  check(
    SharedStateMirrorPolicy.pending(keyboard, null, settingsNewer, KEYBOARD_ENTRIES, true)
      .length === 0,
    "settings keeps its own later change",
  );
  const agreed = manifest("settings", [["preferences", "k1", 900]]);
  check(
    SharedStateMirrorPolicy.pending(keyboard, null, agreed, KEYBOARD_ENTRIES, true).length === 1,
    "identical content is still recorded as imported, which is what stops it bouncing back",
  );
});

group("an imported copy publishes the same fingerprint, so the two sides settle", () => {
  const settings = SharedStateMirrorPolicy.plan(
    null,
    "settings",
    [{ name: "preferences", fingerprint: "p1" }],
    100,
    [],
  );
  const keyboardRecord = SharedStateMirrorPolicy.recordImport(null, "preferences", "p1");
  const keyboard = SharedStateMirrorPolicy.plan(
    null,
    "keyboard",
    [{ name: "preferences", fingerprint: "p1" }],
    200,
    keyboardRecord.entries,
  );
  const settingsRecord = SharedStateMirrorPolicy.recordImport(null, "preferences", "p1");
  check(
    SharedStateMirrorPolicy.pending(
      keyboard.manifest,
      settingsRecord,
      settings.manifest,
      KEYBOARD_ENTRIES,
      true,
    ).length === 0,
    "settings does not import its own preferences back",
  );
  const again = SharedStateMirrorPolicy.plan(
    settings.manifest,
    "settings",
    [{ name: "preferences", fingerprint: "p1" }],
    300,
    [],
  );
  check(again.changed.length === 0, "and has nothing new to publish");
});

group("keyboard statistics are adopted only after the keyboard applied the current switch", () => {
  const switchText = JSON.stringify({ enabled: true, retention: "forever", resets: 1 });
  const current = fp(switchText);
  const stale = manifest(
    "keyboard",
    [["typing-statistics", "t1", 100]],
    [["statistics-switch", fp("old")]],
  );
  const caughtUp = manifest(
    "keyboard",
    [["typing-statistics", "t2", 200]],
    [["statistics-switch", current]],
  );
  check(
    !SharedStateMirrorPolicy.statisticsInStep(stale, current),
    "stale counts would undo a reset or a switch",
  );
  check(
    SharedStateMirrorPolicy.statisticsInStep(caughtUp, current),
    "counts recorded under the current switch are taken",
  );
  check(
    !SharedStateMirrorPolicy.statisticsInStep(null, current),
    "no keyboard manifest, nothing to adopt",
  );
});

group("adopting keyboard statistics keeps the skins and badges settings recorded", () => {
  const keyboard = JSON.stringify({
    enabled: true,
    total: 120,
    skinsTried: ["b"],
    achievements: { first: "2026-10-05" },
  });
  const settings = JSON.stringify({
    enabled: true,
    total: 3,
    skinsTried: ["a", "b"],
    achievements: { first: "2026-10-01", skins: "2026-10-02" },
  });
  const merged = SharedStateMirrorPolicy.mergeStatistics(keyboard, settings);
  check(merged !== null, "both documents are readable");
  const parsed = JSON.parse(merged as string);
  check(parsed.total === 120, "counts come from the keyboard, which records them");
  check(
    JSON.stringify(parsed.skinsTried) === JSON.stringify(["a", "b"]),
    "skins tried are the union",
  );
  check(parsed.achievements.first === "2026-10-01", "a badge keeps the earlier unlock day");
  check(parsed.achievements.skins === "2026-10-02", "a badge only settings knows survives");
  check(
    SharedStateMirrorPolicy.mergeStatistics("[1]", settings) === null,
    "an unreadable keyboard file is retried",
  );
  const alone = JSON.parse(
    SharedStateMirrorPolicy.mergeStatistics(JSON.stringify({ total: 1 }), null) as string,
  );
  check(
    alone.total === 1 && alone.skinsTried === undefined && alone.achievements === undefined,
    "nothing is invented when settings has no file",
  );
});

group("the statistics switch rejects anything but a well-formed document", () => {
  check(
    SharedStateMirrorPolicy.parseStatisticsSwitch(
      JSON.stringify({ enabled: false, retention: "30d", resets: 0 }),
    ) !== null,
    "a valid switch is read",
  );
  check(
    SharedStateMirrorPolicy.parseStatisticsSwitch(
      JSON.stringify({ enabled: true, retention: "7d", resets: 0 }),
    ) === null,
    "an unknown retention is refused",
  );
  check(
    SharedStateMirrorPolicy.parseStatisticsSwitch(
      JSON.stringify({ enabled: true, retention: "30d", resets: -1 }),
    ) === null,
    "a negative reset count is refused",
  );
  check(SharedStateMirrorPolicy.parseStatisticsSwitch("{") === null, "broken JSON is refused");
});

group("manifests and records from another format are ignored", () => {
  check(
    SharedStateMirrorPolicy.parseManifest(
      JSON.stringify({ format: 2, publisher: "settings", entries: [] }),
    ) === null,
    "a future manifest format is not misread",
  );
  check(
    SharedStateMirrorPolicy.parseManifest(
      JSON.stringify({ format: 1, publisher: "other", entries: [] }),
    ) === null,
    "an unknown publisher is refused",
  );
  const parsed = SharedStateMirrorPolicy.parseManifest(
    JSON.stringify({
      format: 1,
      publisher: "keyboard",
      entries: [{ name: "x", fingerprint: "f", changedMs: 1 }, { name: 3 }],
    }),
  );
  check(
    parsed !== null && parsed.entries.length === 1 && parsed.applied.length === 0,
    "malformed entries are dropped and a missing applied list reads as empty",
  );
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} shared state mirror policy checks failed`);
