import { FlickThresholdSettings } from "../entry/src/main/ets/keyboard/settings/FlickThresholdSettings";
import { NineKeyFlick, NineKeyFlickGesture } from "../entry/src/main/ets/keyboard/input/NineKeyFlickPolicy";

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
  let snapshot: Snapshot = { format_version: 1, revision: 4,
    preferences: { synthetic_other: { keep: true } }, extension: { synthetic: true } };
  let writes = 0;
  let failure = "";
  const load = (): string => JSON.stringify({ ok: true, value: snapshot });
  const save = (revision: number, document: string): string => {
    writes++;
    if (failure === "disk") throw new Error("synthetic_disk_failure");
    if (failure === "conflict") {
      snapshot.revision++;
      snapshot.preferences["touch_nine_key_flick_threshold"] = 32;
      snapshot.preferences["synthetic_concurrent"] = true;
      return JSON.stringify({ ok: false, error: "preferences_changed" });
    }
    check(revision === snapshot.revision, "保存使用刚读取的修订号");
    snapshot = JSON.parse(document) as Snapshot;
    // 模拟共享 Rust 的默认值省略；控件必须采用保存回复，不能假定字段仍存在。
    if (snapshot.preferences["touch_nine_key_flick_threshold"] === 24)
      delete snapshot.preferences["touch_nine_key_flick_threshold"];
    snapshot.revision++;
    return load();
  };
  return {
    store: new FlickThresholdSettings(load, save),
    reopen: (): FlickThresholdSettings => new FlickThresholdSettings(load, save),
    current: (): Snapshot => snapshot,
    writes: (): number => writes,
    fail: (kind: string): void => { failure = kind; },
  };
}

{
  const f = fixture();
  const initial = f.store.read();
  check(initial.ready && initial.threshold === 24 && f.writes() === 0,
    "首次打开只回读默认值，不把缺字段写成默认值");
  for (let value = 14; value <= 48; value += 2) {
    const result = f.store.write(value);
    check(result.ready && result.threshold === value && result.error === "", "保存所有合法滑块档位");
    check(f.reopen().read().threshold === value, "重开原生设置后从共享快照回读");
  }
  f.store.write(24);
  check(f.current().preferences["touch_nine_key_flick_threshold"] === undefined,
    "共享层省略默认字段时仍显示 24");
  check(f.current().format_version === 1 && f.current().extension.synthetic, "保留快照格式及扩展字段");
  check(JSON.stringify(f.current().preferences["synthetic_other"]) === '{"keep":true}', "不覆盖其他偏好");
}

{
  const f = fixture();
  f.store.read();
  f.current().preferences["touch_nine_key_flick_threshold"] = 48;
  f.current().preferences["synthetic_after_open"] = true;
  f.current().revision++;
  check(f.store.read().threshold === 48, "回前台重读外部同步的阈值");
  check(f.store.write(14).error === "", "保存前重新读取最新快照");
  check(f.current().preferences["synthetic_after_open"] === true, "不丢失设置页打开后出现的修改");
  const gestures = new NineKeyFlickGesture();
  for (const value of [48, 14]) {
    const saved = f.store.write(value);
    gestures.down("2", 1, 0, 0, "ABC", saved.threshold);
    const direction = gestures.move("2", 1, 0, -20);
    check(direction === (value === 14 ? NineKeyFlick.FIRST : NineKeyFlick.NONE),
      "保存后的共享阈值改变下一次手势判定");
    gestures.up("2", 1);
  }
}

for (const kind of ["disk", "conflict"]) {
  const f = fixture();
  f.store.write(48);
  f.fail(kind);
  const count = f.writes();
  const result = f.store.write(14);
  check(result.ready && result.error.length > 0, "失败保留可重试状态并提示");
  check(result.threshold === (kind === "disk" ? 48 : 32), "失败后回到权威值，不显示未保存的选择");
  check(f.writes() === count + 1, "冲突不自动覆盖重试");
  if (kind === "conflict")
    check(f.current().preferences["synthetic_concurrent"] === true, "保留并发写入的字段");
}

for (const value of [-1, 0, 12, 13, 15, 25, 49, 50, 14.5, Number.NaN, Infinity]) {
  const f = fixture();
  const result = f.store.write(value);
  check(result.error.length > 0 && result.threshold === 24 && f.writes() === 0, "非法值不能传给共享保存入口");
}

for (const reply of [
  "null", "{", '{"ok":false}', '{"ok":true,"value":null}',
  '{"ok":true,"value":{"format_version":2,"revision":1,"preferences":{}}}',
  '{"ok":true,"value":{"format_version":1,"revision":-1,"preferences":{}}}',
  '{"ok":true,"value":{"format_version":1,"revision":1.5,"preferences":{}}}',
  '{"ok":true,"value":{"format_version":1,"revision":1,"preferences":[]}}',
  '{"ok":true,"value":{"format_version":1,"revision":1,"preferences":null}}',
]) {
  let writes = 0;
  const store = new FlickThresholdSettings((): string => reply, (): string => { writes++; return reply; });
  check(!store.read().ready, "无效共享快照禁用滑块");
  check(!store.write(14).ready && writes === 0, "读取失败不能用空设置覆盖文档");
}

console.log("鸿蒙 flick 原生设置：全部档位、默认省略、重开、前台刷新、冲突及手势接入通过");
