import { LetterWidthSettings } from "../entry/src/main/ets/keyboard/settings/LetterWidthSettings";

function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}

interface Snapshot {
  format_version: number;
  revision: number;
  preferences: Record<string, Object>;
  extension: { synthetic: boolean };
}

function fixture() {
  let snapshot: Snapshot = {
    format_version: 1,
    revision: 4,
    preferences: { synthetic_other: { keep: true } },
    extension: { synthetic: true },
  };
  let writes = 0;
  let fail = false;
  let conflict = false;
  const load = (): string => JSON.stringify({ ok: true, value: snapshot });
  const save = (revision: number, document: string): string => {
    writes++;
    if (fail) throw new Error("synthetic_disk_failure");
    if (conflict) {
      snapshot.revision++;
      snapshot.preferences["synthetic_concurrent"] = true;
      return JSON.stringify({ ok: false, error: "preferences_changed" });
    }
    check(revision === snapshot.revision, "保存必须使用刚读取的修订号");
    snapshot = JSON.parse(document) as Snapshot;
    snapshot.revision++;
    return load();
  };
  return {
    store: new LetterWidthSettings(load, save),
    reopen: (): LetterWidthSettings => new LetterWidthSettings(load, save),
    current: (): Snapshot => snapshot,
    writes: (): number => writes,
    fail: (): void => {
      fail = true;
    },
    conflict: (): void => {
      conflict = true;
    },
  };
}

{
  const f = fixture();
  const initial = f.store.read();
  check(
    initial.ready && initial.mode === "equal" && f.writes() === 0,
    "缺字段只显示等宽，不主动写默认值",
  );
  check(f.store.write("varied").mode === "varied", "原生选择应保存宽窄");
  check(f.reopen().read().mode === "varied", "重开面板从落盘快照回读");
  const saved = f.current();
  check(saved.format_version === 1 && saved.extension.synthetic, "保留快照版本及扩展字段");
  check(JSON.stringify(saved.preferences["synthetic_other"]) === '{"keep":true}', "保留不相关偏好");
  check(
    f.store.write("equal").mode === "equal" && f.reopen().read().mode === "equal",
    "切回等宽并回读",
  );
}

{
  const f = fixture();
  f.store.read();
  f.current().preferences["synthetic_after_open"] = true;
  f.current().revision++;
  check(f.store.write("varied").error === "", "点击时重新读权威快照");
  check(f.current().preferences["synthetic_after_open"] === true, "不覆盖面板打开后新增的偏好");
}

for (const kind of ["disk", "conflict"]) {
  const f = fixture();
  if (kind === "disk") f.fail();
  else f.conflict();
  const result = f.store.write("varied");
  check(result.error.length > 0 && result.mode === "equal", "失败不能显示为保存成功");
  check(f.current().preferences["touch_fourteen_key_letter_widths"] === undefined, "失败不改原值");
  if (kind === "conflict")
    check(f.current().preferences["synthetic_concurrent"] === true, "冲突保留另一端修改");
}

for (const reply of [
  "null",
  "{",
  '{"ok":false}',
  '{"ok":true,"value":null}',
  '{"ok":true,"value":{"revision":-1,"preferences":{}}}',
  '{"ok":true,"value":{"revision":1,"preferences":[]}}',
]) {
  let writes = 0;
  const store = new LetterWidthSettings(
    (): string => reply,
    (): string => {
      writes++;
      return reply;
    },
  );
  check(!store.read().ready, "无效快照禁止操作");
  check(store.write("varied").error.length > 0 && writes === 0, "无效快照不能写回空设置");
}

console.log("鸿蒙原生宽窄设置：默认回读、单字段保存、重开、并发与失败边界通过");
