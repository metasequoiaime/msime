export interface ShuangpinTable {
  initials: Record<string, string>;
  finals: Record<string, string>;
  zero_initials: Record<string, string>;
}
export interface ShuangpinProblem {
  kind: string;
  part?: string;
  unit?: string;
  key?: string;
  code?: string;
  first?: string;
  second?: string;
}
export interface ShuangpinKey {
  key: string;
  hint: string;
}
interface Layout {
  effective: string;
  table: ShuangpinTable;
  problem: ShuangpinProblem | null;
  keys: ShuangpinKey[];
}
interface Snapshot {
  format_version: number;
  revision: number;
  preferences: Record<string, Object>;
}
interface SnapshotReply {
  ok: boolean;
  value?: Snapshot;
  error?: string;
}
interface Value {
  preferences: Record<string, Object>;
  draft: ShuangpinTable;
  layout: Layout;
}
interface Reply {
  ok: boolean;
  value?: Value;
  error?: string;
}
interface Request {
  preferences: Object;
  edition: string;
  operation: string;
  table?: ShuangpinTable;
  text?: string;
  profile?: string;
  hints?: boolean;
}
export interface CustomShuangpinBridge {
  load(): string;
  save(revision: number, document: string): string;
  plan(request: string): string;
}
export interface ShuangpinField {
  part: string;
  unit: string;
  key: string;
}

/** 草稿与引擎回退表分开持有；校验失败不能把用户正在编辑的内容换成小鹤。 */
export class CustomShuangpinSettings {
  draft: ShuangpinTable = { initials: {}, finals: {}, zero_initials: {} };
  keys: ShuangpinKey[] = [];
  problem: string = "";
  message: string = "";
  ready: boolean = false;
  dirty: boolean = false;
  saved: boolean = false;
  hints: boolean = false;
  active: boolean = false;

  constructor(
    private bridge: CustomShuangpinBridge,
    private edition: string,
  ) {}

  private snapshot(): Snapshot {
    const reply: SnapshotReply = JSON.parse(this.bridge.load()) as SnapshotReply;
    if (
      reply.ok !== true ||
      reply.value === undefined ||
      reply.value.format_version !== 1 ||
      !Number.isSafeInteger(reply.value.revision) ||
      reply.value.revision < 0 ||
      typeof reply.value.preferences !== "object" ||
      reply.value.preferences === null ||
      Array.isArray(reply.value.preferences)
    )
      throw new Error("读取设置失败，请重试");
    return reply.value;
  }

  private calculate(
    snapshot: Snapshot,
    operation: string,
    table?: ShuangpinTable,
    text?: string,
    profile?: string,
    hints?: boolean,
  ): Value {
    const request: Request = {
      preferences: snapshot.preferences,
      edition: this.edition,
      operation: operation,
    };
    if (table !== undefined) request.table = table;
    if (text !== undefined) request.text = text;
    if (profile !== undefined) request.profile = profile;
    if (hints !== undefined) request.hints = hints;
    const reply: Reply = JSON.parse(this.bridge.plan(JSON.stringify(request))) as Reply;
    if (reply.ok !== true || reply.value === undefined) {
      if (reply.error === "custom_shuangpin_import_too_large")
        throw new Error("码表文件不能超过 16 KB");
      if (reply.error === "custom_shuangpin_import_invalid")
        throw new Error("码表须为 JSON，只接受 initials、finals 和 zero_initials 分组");
      if (reply.error === "custom_shuangpin_unavailable") throw new Error("当前版本不提供双拼");
      throw new Error("码表操作失败，请重试");
    }
    return reply.value;
  }

  static problemMessage(problem: ShuangpinProblem | null): string {
    if (problem === null) return "";
    const title: string =
      problem.part === "initials"
        ? "声母"
        : problem.part === "finals"
          ? "韵母"
          : problem.part === "zero_initials"
            ? "零声母音节"
            : "";
    const place: string = title + (problem.unit === undefined ? "" : `「${problem.unit}」`);
    switch (problem.kind) {
      case "unknown_unit":
        return place + "不是这张表认识的单位";
      case "duplicate_unit":
        return place + "写了两次";
      case "missing_unit":
        return place + "还没有放到字母键上";
      case "invalid_key":
        return place + `的键「${problem.key ?? ""}」不符合规则`;
      case "initial_key_taken":
        return place + `占用了单字母声母的键「${problem.key ?? ""}」`;
      case "ambiguous_code":
        return `编码「${problem.code ?? ""}」同时对应「${problem.first ?? ""}」和「${problem.second ?? ""}」`;
      default:
        return "这张表不能用";
    }
  }

  private accept(value: Value): void {
    this.draft = value.draft;
    this.keys = value.layout.problem === null ? value.layout.keys : [];
    this.problem = CustomShuangpinSettings.problemMessage(value.layout.problem);
  }

  load(): void {
    this.message = "";
    try {
      const snapshot: Snapshot = this.snapshot();
      const value: Value = this.calculate(snapshot, "read");
      this.saved =
        Object.keys((value.preferences["shuangpin_custom_profile"] as Object) ?? {}).length > 0;
      this.active = value.preferences["shuangpin_profile"] === "custom";
      this.hints = value.preferences["touch_shuangpin_key_hints"] === true;
      if (!this.dirty) this.accept(value);
      this.ready = true;
    } catch (error) {
      this.ready = false;
      this.message = (error as Error).message;
    }
  }

  fields(part: string): ShuangpinField[] {
    const values: Record<string, string> =
      part === "initials"
        ? this.draft.initials
        : part === "finals"
          ? this.draft.finals
          : this.draft.zero_initials;
    return Object.keys(values)
      .sort()
      .map((unit: string): ShuangpinField => ({ part: part, unit: unit, key: values[unit] }));
  }

  edit(field: ShuangpinField, key: string): void {
    const draft: ShuangpinTable = JSON.parse(JSON.stringify(this.draft)) as ShuangpinTable;
    const values: Record<string, string> =
      field.part === "initials"
        ? draft.initials
        : field.part === "finals"
          ? draft.finals
          : draft.zero_initials;
    values[field.unit] = key.toLowerCase();
    this.draft = draft;
    this.dirty = true;
    this.message = "";
    try {
      this.accept(this.calculate(this.snapshot(), "check", draft));
    } catch (error) {
      this.keys = [];
      this.problem = (error as Error).message;
    }
  }

  importText(text: string): void {
    this.message = "";
    try {
      const value: Value = this.calculate(this.snapshot(), "import", undefined, text);
      const problem: string = CustomShuangpinSettings.problemMessage(value.layout.problem);
      if (problem.length > 0) throw new Error(problem);
      this.accept(value);
      this.dirty = true;
    } catch (error) {
      this.message = (error as Error).message;
    }
  }

  template(profile: string): void {
    try {
      const value: Value = this.calculate(
        this.snapshot(),
        "template",
        undefined,
        undefined,
        profile,
      );
      this.accept(value);
      this.dirty = true;
      this.message = "";
    } catch (error) {
      this.message = (error as Error).message;
    }
  }

  exportText(): string | undefined {
    try {
      const checked: Value = this.calculate(this.snapshot(), "check", this.draft);
      if (checked.layout.problem !== null)
        throw new Error(CustomShuangpinSettings.problemMessage(checked.layout.problem));
      return JSON.stringify(this.draft, null, 2);
    } catch (error) {
      this.message = (error as Error).message;
      return undefined;
    }
  }

  private write(operation: string, hints?: boolean): boolean {
    if (!this.ready) return false;
    try {
      const snapshot: Snapshot = this.snapshot();
      const value: Value = this.calculate(
        snapshot,
        operation,
        operation === "use" ? this.draft : undefined,
        undefined,
        undefined,
        hints,
      );
      snapshot.preferences = value.preferences;
      const saved: SnapshotReply = JSON.parse(
        this.bridge.save(snapshot.revision, JSON.stringify(snapshot)),
      ) as SnapshotReply;
      if (saved.ok !== true || saved.value === undefined)
        throw new Error("保存失败或设置已变化，请重试");
      if (operation !== "hints") this.dirty = false;
      this.load();
      return true;
    } catch (error) {
      this.load();
      this.message = (error as Error).message;
      return false;
    }
  }

  use(): boolean {
    if (this.problem.length > 0) {
      this.message = this.problem;
      return false;
    }
    return this.write("use");
  }
  remove(): boolean {
    return this.write("delete");
  }
  setHints(value: boolean): boolean {
    return this.write("hints", value);
  }
}
