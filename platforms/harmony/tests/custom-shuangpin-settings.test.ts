import {
  CustomShuangpinSettings,
  ShuangpinTable,
} from "../entry/src/main/ets/keyboard/settings/CustomShuangpinSettings";
function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}
const table: ShuangpinTable = {
  initials: { zh: "v" },
  finals: { ang: "h" },
  zero_initials: { a: "aa" },
};
const clone = (value: ShuangpinTable): ShuangpinTable =>
  JSON.parse(JSON.stringify(value)) as ShuangpinTable;
function fixture() {
  let current = {
    format_version: 1,
    revision: 4,
    preferences: {
      synthetic_other: "keep",
      shuangpin_profile: "xiaohe",
      shuangpin_custom_profile: table,
      touch_shuangpin_key_hints: false,
    },
  };
  let writes: number = 0;
  let conflict: boolean = false;
  let disk: boolean = false;
  let rejectImport: boolean = false;
  let rejectLoad: boolean = false;
  const requests: string[] = [];
  const model = new CustomShuangpinSettings(
    {
      load: (): string =>
        rejectLoad ? '{"ok":false}' : JSON.stringify({ ok: true, value: current }),
      save: (revision: number, document: string): string => {
        writes++;
        check(revision === current.revision, "保存使用最新修订号");
        if (conflict) {
          current.revision++;
          return '{"ok":false,"error":"conflict"}';
        }
        if (disk) throw new Error("合成磁盘失败");
        current = JSON.parse(document) as typeof current;
        current.revision++;
        return JSON.stringify({ ok: true, value: current });
      },
      plan: (request: string): string => {
        const parsed = JSON.parse(request) as {
          operation: string;
          preferences: typeof current.preferences;
          table?: ShuangpinTable;
          text?: string;
          hints?: boolean;
        };
        requests.push(parsed.operation);
        let draft: ShuangpinTable = clone(parsed.table ?? table);
        if (parsed.operation === "import") {
          if (rejectImport) return '{"ok":false,"error":"custom_shuangpin_import_invalid"}';
          draft = JSON.parse(parsed.text ?? "{}") as ShuangpinTable;
        }
        if (parsed.operation === "use") {
          parsed.preferences.shuangpin_profile = "custom";
          parsed.preferences.shuangpin_custom_profile = draft;
        }
        if (parsed.operation === "delete") {
          parsed.preferences.shuangpin_custom_profile = {
            initials: {},
            finals: {},
            zero_initials: {},
          };
          parsed.preferences.shuangpin_profile = "xiaohe";
        }
        if (parsed.operation === "hints")
          parsed.preferences.touch_shuangpin_key_hints = parsed.hints === true;
        const problem =
          draft.finals["ang"] === "" ? { kind: "missing_unit", part: "finals", unit: "ang" } : null;
        return JSON.stringify({
          ok: true,
          value: {
            preferences: parsed.preferences,
            draft: draft,
            layout: {
              effective: problem === null ? "custom" : "xiaohe",
              table: table,
              problem: problem,
              keys: [{ key: "h", hint: "ang" }],
            },
          },
        });
      },
    },
    "full",
  );
  return {
    model: model,
    current: (): typeof current => current,
    writes: (): number => writes,
    requests: requests,
    fail: (kind: string): void => {
      conflict = kind === "conflict";
      disk = kind === "disk";
      rejectImport = kind === "import";
      rejectLoad = kind === "load";
    },
  };
}
{
  const f = fixture();
  f.model.load();
  check(f.model.ready && f.writes() === 0 && !f.model.dirty, "打开不保存或改默认值");
  check(f.model.keys[0].hint === "ang", "提示使用共享响应");
  f.model.edit({ part: "finals", unit: "ang", key: "h" }, "");
  check(
    f.model.draft.finals["ang"] === "" && f.model.problem.includes("ang"),
    "非法编辑保留原草稿并显示位置",
  );
  check(
    f.model.keys.length === 0 && f.model.exportText() === undefined,
    "非法草稿不展示回退提示也不能导出",
  );
  check(!f.model.use() && f.writes() === 0, "非法草稿不保存");
  f.model.load();
  check(f.model.draft.finals["ang"] === "", "前台刷新保留未保存草稿");
  f.model.template("xiaohe");
  check(f.model.problem === "" && f.model.dirty, "模板替换草稿但不保存");
  f.current().revision = 8;
  check(
    f.model.use() && f.current().preferences.shuangpin_profile === "custom" && !f.model.dirty,
    "保存按最新快照并使用共享事务",
  );
  check(f.current().preferences.synthetic_other === "keep", "无关设置保留");
  check(f.requests.includes("use"), "保存交由共享层计算");
}
{
  const f = fixture();
  f.model.load();
  const before = JSON.stringify(f.model.draft);
  f.fail("import");
  f.model.importText("{}");
  check(
    JSON.stringify(f.model.draft) === before && f.model.message.includes("JSON"),
    "非法导入不替换现有草稿",
  );
  f.fail("");
  f.model.importText(JSON.stringify({ ...table, finals: { ang: "" } }));
  check(
    JSON.stringify(f.model.draft) === before && f.model.message.includes("ang"),
    "具体校验失败的导入也保留草稿",
  );
  f.model.importText(JSON.stringify(table));
  check(f.model.dirty && f.writes() === 0, "合法导入只改草稿");
  check(f.model.setHints(true) && f.model.hints && f.model.dirty, "提示开关独立保存且不丢草稿");
  f.fail("conflict");
  check(
    !f.model.use() && f.model.dirty && f.model.message.includes("变化"),
    "冲突只回读，不自动覆盖重试",
  );
  check(f.writes() === 2, "冲突只写一次");
  f.fail("disk");
  check(!f.model.use() && f.model.dirty, "磁盘失败保留草稿");
  f.fail("load");
  f.model.load();
  check(!f.model.ready && !f.model.remove(), "读取失败不写盘");
}
for (const problem of [
  { kind: "duplicate_unit", part: "initials", unit: "zh" },
  { kind: "invalid_key", part: "finals", unit: "ang", key: "1" },
  { kind: "ambiguous_code", code: "ab", first: "synthetic-a", second: "synthetic-b" },
])
  check(CustomShuangpinSettings.problemMessage(problem).length > 0, "共享校验问题有可读说明");
console.log("自定义双拼草稿、共享校验、CAS 冲突、导入隔离、提示和失败保留通过");
