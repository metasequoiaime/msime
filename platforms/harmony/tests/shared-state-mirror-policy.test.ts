import {
  ABSENT_FINGERPRINT,
  FingerprintCache,
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

group("one cloud clipboard device identity per installation, made by the settings app", () => {
  // 两个沙箱各用一份身份，同一台手机就会在空间里登记成两台、要批准两次；键盘只用设置应用这一份。
  const relay = SETTINGS_ENTRIES.find((entry) => entry.name === "cloud-clipboard-relay-keystore");
  const account = SETTINGS_ENTRIES.find((entry) => entry.name === "cloud-clipboard-keystore");
  check(
    relay !== undefined &&
      relay.kind === "file" &&
      relay.base === "state" &&
      relay.path === "cloud-clipboard-relay-keystore",
    "settings publishes the relay and WebDAV identity next to the account session",
  );
  check(
    account !== undefined &&
      account.base === "state" &&
      account.path === "cloud-clipboard-keystore",
    "settings publishes the account identity",
  );
  check(
    !KEYBOARD_ENTRIES.some((entry) => entry.name.includes("keystore")),
    "the keyboard never publishes an identity of its own",
  );
  const theirs = manifest("settings", [
    ["cloud-clipboard-relay-keystore", "r1", 100],
    ["cloud-clipboard-keystore", "k1", 100],
  ]);
  const basic = SharedStateMirrorPolicy.pending(theirs, null, null, SETTINGS_ENTRIES, false);
  check(
    basic.length === 1 && basic[0].name === "cloud-clipboard-relay-keystore",
    "a basic-mode keyboard reads the relay identity; the account one follows the session",
  );
  check(
    SharedStateMirrorPolicy.pending(theirs, null, null, SETTINGS_ENTRIES, true).length === 2,
    "a full-mode keyboard takes both",
  );
  for (const name of ["cloud-clipboard-relay-keystore", "cloud-clipboard-keystore"]) {
    check(
      SharedStateMirrorPolicy.keepsLocalWhenAbsent(name),
      `${name} is not deleted before settings has made one`,
    );
    check(SharedStateMirrorPolicy.importDecision(name) === "copy", `${name} is copied whole`);
  }
  const cache = SETTINGS_ENTRIES.find((entry) => entry.name === "cloud-clipboard-webdav-state");
  check(
    cache !== undefined && !cache.fullModeOnly && cache.path === "cloud-clipboard-webdav-state",
    "the drive cache follows settings, so trusting the drive again reaches the keyboard",
  );
  for (const name of [
    "key-feedback",
    "account-session",
    "privacy-consent",
    "cloud-clipboard-webdav-state",
  ]) {
    check(!SharedStateMirrorPolicy.keepsLocalWhenAbsent(name), `${name} still follows a deletion`);
  }
});

group("the keyboard publishes its anonymous identity in full experience mode", () => {
  const published = KEYBOARD_ENTRIES.find((entry) => entry.name === "anonymous-account");
  check(
    published !== undefined &&
      published.path === "anonymous-account.json" &&
      published.base === "state" &&
      published.fullModeOnly,
    "settings can only take the identity of the anonymous account it adopted if the keyboard publishes it",
  );
});

group(
  "sessions and the anonymous identity are left to session_adopt, everything else is copied",
  () => {
    // 身份随匿名会话由 Rust 采用（迁移差异 M40），不再按文件复制：复制会让键盘拿着自己的匿名会话、换上设置应用的身份，或者设置应用采用了键盘的会话、留着自己的身份。
    for (const name of ["account-session", "anonymous-session", "anonymous-account"]) {
      check(
        SharedStateMirrorPolicy.importDecision(name) === "handled",
        `${name} is neither copied nor deleted by the mirror`,
      );
    }
    check(
      SharedStateMirrorPolicy.importDecision("key-feedback") === "copy",
      "other files are still copied (and deleted when the publisher has none)",
    );
    check(
      SharedStateMirrorPolicy.importDecision("app-theme") === "copy",
      "other files are still copied",
    );
  },
);

group("the anonymous identity has no special case for which side changed it later", () => {
  // 键盘先注册了 K、还没发布时设置应用注册了 S 并发布（300）；键盘随后获得焦点，先发布自己的 K（500），再导入。身份交给 Rust 以后与其他条目一样按变化时刻判断，留不留身份由 session_adopt 随会话决定。
  const settings = manifest("settings", [
    ["anonymous-account", "s-identity", 300],
    ["preferences", "s-preferences", 300],
  ]);
  const keyboard = manifest("keyboard", [
    ["anonymous-account", "k-identity", 500],
    ["preferences", "k-preferences", 500],
  ]);
  const pending = SharedStateMirrorPolicy.pending(settings, null, keyboard, SETTINGS_ENTRIES, true);
  check(
    !pending.some((entry) => entry.name === "anonymous-account") &&
      !pending.some((entry) => entry.name === "preferences"),
    "the later change wins for the identity as for any other entry",
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

group("local fingerprints are reused while the file identity is unchanged", () => {
  const cache = new FingerprintCache();
  let reads = 0;
  const read = (text: string) => (): string => {
    reads++;
    return fp(text);
  };
  const stamp = FingerprintCache.stamp("12", 40, "1700000000123456789");
  check(stamp === "12:40:1700000000123456789", "the stamp joins inode, size and nanosecond mtime");
  check(
    cache.fingerprint("file:/a", stamp, read("one")) === fp("one"),
    "the first lookup reads the file",
  );
  check(
    cache.fingerprint("file:/a", stamp, read("two")) === fp("one"),
    "an unchanged identity reuses the fingerprint",
  );
  check(reads === 1, "an unchanged identity does not read the file again");
  // 暂存加改名换入：inode 变了，哪怕大小和修改时间碰巧相同。
  const renamed = FingerprintCache.stamp("13", 40, "1700000000123456789");
  check(
    cache.fingerprint("file:/a", renamed, read("two")) === fp("two"),
    "a replaced file is read again",
  );
  check(reads === 2, "a replaced file costs one read");
  check(
    cache.fingerprint("file:/b", renamed, read("three")) === fp("three"),
    "paths are cached separately",
  );
  check(reads === 3, "another path is read on its own");
});

group("local fingerprints are not cached without a nanosecond mtime or when unreadable", () => {
  const cache = new FingerprintCache();
  let reads = 0;
  const read = (fingerprint: string) => (): string => {
    reads++;
    return fingerprint;
  };
  check(
    FingerprintCache.stamp("12", 40, undefined) === null,
    "second-resolution mtimes give no stamp",
  );
  cache.fingerprint("file:/a", null, read("f1"));
  cache.fingerprint("file:/a", null, read("f1"));
  check(reads === 2, "without a stamp every lookup reads the file");
  const stamp = FingerprintCache.stamp("12", 40, "5");
  check(
    cache.fingerprint("file:/a", stamp, read(ABSENT_FINGERPRINT)) === ABSENT_FINGERPRINT,
    "a failed read reports the file as absent",
  );
  check(
    cache.fingerprint("file:/a", stamp, read("f2")) === "f2",
    "a failed read is retried next time",
  );
  check(reads === 4, "a failed read is not cached");
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
group("遥测授权属于设置应用，键盘旧镜像不能恢复统计", () => {
  const local = JSON.stringify({
    revision: 9,
    preferences: {
      usage_reporting: false,
      usage_reporting_consent: { epoch: "local" },
      keyboard_height: 40,
    },
  });
  const incoming = JSON.stringify({
    revision: 3,
    preferences: {
      usage_reporting: true,
      usage_reporting_consent: { epoch: "remote" },
      keyboard_height: 50,
    },
  });
  const text = SharedStateMirrorPolicy.preferencesImport(incoming, local, true);
  check(text !== null, "有效偏好可以导入");
  const merged = JSON.parse(text ?? "{}");
  check(
    merged.revision === 9 && merged.preferences.keyboard_height === 50,
    "继续采用键盘改动与本地修订号",
  );
  check(merged.preferences.usage_reporting === false, "键盘旧开关不能重新授权");
  check(merged.preferences.usage_reporting_consent === undefined, "不复制另一沙箱的代次");
  const keyboard = JSON.parse(
    SharedStateMirrorPolicy.preferencesImport(local, incoming, false) ?? "{}",
  );
  check(keyboard.preferences.usage_reporting === false, "设置应用的开关仍然导入键盘");
  const same = JSON.stringify({
    preferences: {
      usage_reporting: false,
      usage_reporting_consent: { epoch: "other" },
      keyboard_height: 40,
    },
  });
  check(
    SharedStateMirrorPolicy.preferencesFingerprint(local) ===
      SharedStateMirrorPolicy.preferencesFingerprint(same),
    "独立代次不触发镜像往返",
  );
});

if (failures !== 0) throw new Error(`${failures} shared state mirror policy checks failed`);
