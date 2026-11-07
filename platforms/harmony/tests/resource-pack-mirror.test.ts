import {
  ABSENT_FINGERPRINT,
  KEYBOARD_ENTRIES,
  SETTINGS_ENTRIES,
  SharedStateMirrorPolicy,
} from "../entry/src/main/ets/keyboard/SharedStateMirrorPolicy";

function check(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

for (const id of ["japanese", "wubi"]) {
  const name = `resource-pack-${id}`;
  const entry = SETTINGS_ENTRIES.find((item) => item.name === name);
  check(
    entry?.kind === "directory" && entry.base === "state" && entry.path === `resource-packs/${id}`,
    `${id} 必须从设置私有目录发布到键盘私有目录，只复制已发布的包目录`,
  );
  check(!KEYBOARD_ENTRIES.some((item) => item.name === name), "键盘不能反向发布资源包");
  const installed = SharedStateMirrorPolicy.plan(
    null,
    "settings",
    [{ name, fingerprint: "synthetic-installed-pack" }],
    100,
    [],
  );
  for (const fullMode of [false, true]) {
    const pending = SharedStateMirrorPolicy.pending(
      installed.manifest,
      null,
      null,
      SETTINGS_ENTRIES,
      fullMode,
    );
    check(
      pending.some((item) => item.name === name),
      `${id} 在两种访问模式下都能导入`,
    );
    const imported = SharedStateMirrorPolicy.recordImport(null, name, "synthetic-installed-pack");
    check(
      SharedStateMirrorPolicy.pending(
        installed.manifest,
        imported,
        null,
        SETTINGS_ENTRIES,
        fullMode,
      ).length === 0,
      "已经导入的包不重复复制",
    );
    const removed = SharedStateMirrorPolicy.plan(
      installed.manifest,
      "settings",
      [{ name, fingerprint: ABSENT_FINGERPRINT }],
      200,
      [],
    );
    const deletion = SharedStateMirrorPolicy.pending(
      removed.manifest,
      imported,
      null,
      SETTINGS_ENTRIES,
      fullMode,
    );
    check(
      deletion.length === 1 && deletion[0].fingerprint === ABSENT_FINGERPRINT,
      "卸载后键盘必须收到删除副本的通知",
    );
  }
}
console.log("鸿蒙日文和五笔资源：发布、两种访问模式导入、重复聚焦与卸载传播通过");
