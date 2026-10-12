/** 内置的四种双拼方案；`custom` 用的是偏好里的 `shuangpin_custom_profile`。 */
export type BuiltinShuangpinProfile = "xiaohe" | "ziranma" | "shoudao" | "microsoft";

/**
 * 自定义双拼方案的键位表，与 client-core 的 `ShuangpinCustomProfile` 一一对应：`initials` 只放 `zh` `ch` `sh`，`finals` 放 33 个韵母，`zero_initials` 放 12 个零声母音节的两键编码；键是小写字母或 `;`，`ü` 写作 `v`。导入导出的 JSON 也是这个形状，与 MCP 的 `update_preferences` 相同。
 */
export type ShuangpinCustomProfile = {
  initials?: Record<string, string>;
  finals?: Record<string, string>;
  zero_initials?: Record<string, string>;
};

export type ShuangpinCustomTablePart = "initials" | "finals" | "zero_initials";

/** 表为什么不能用，由宿主经 host-api 的 `custom_shuangpin_profile_problem` 回答。Engine 在第一个错误处就停，所以一次只有一条，改好一处再看下一处。 */
export type ShuangpinCustomProblem = {
  kind:
    | "unknown_unit"
    | "duplicate_unit"
    | "missing_unit"
    | "invalid_key"
    | "initial_key_taken"
    | "ambiguous_code";
  /** 编码冲突不属于某一部分，没有这一项。 */
  part?: ShuangpinCustomTablePart;
  unit?: string;
  key?: string;
  code?: string;
  first?: string;
  second?: string;
};

/** 宿主提供的两项计算，都没有副作用。两项都有时设置页才提供自定义双拼的编辑、导入和导出。 */
export interface ShuangpinCustomProfileClient {
  validate: (profile: ShuangpinCustomProfile) => Promise<ShuangpinCustomProblem | null>;
  builtin: (name: BuiltinShuangpinProfile) => Promise<ShuangpinCustomProfile | null>;
}

/** 与 Engine 的 `MULTI_LETTER_INITIALS`、`FINALS`、`ZERO_INITIALS` 同序：编辑器按这个顺序画格子。 */
export const shuangpinCustomUnits: Readonly<Record<ShuangpinCustomTablePart, readonly string[]>> = {
  initials: ["zh", "ch", "sh"],
  finals: [
    "a",
    "o",
    "e",
    "i",
    "u",
    "v",
    "ai",
    "ei",
    "ui",
    "ao",
    "ou",
    "iu",
    "ie",
    "ve",
    "ue",
    "an",
    "en",
    "in",
    "un",
    "ang",
    "eng",
    "ing",
    "ong",
    "ia",
    "ua",
    "uo",
    "uai",
    "ian",
    "uan",
    "iao",
    "iang",
    "uang",
    "iong",
  ],
  zero_initials: ["a", "ai", "an", "ang", "ao", "e", "ei", "en", "eng", "er", "o", "ou"],
};

export const shuangpinCustomParts: readonly ShuangpinCustomTablePart[] = [
  "initials",
  "finals",
  "zero_initials",
];

export const builtinShuangpinProfiles: readonly [BuiltinShuangpinProfile, string][] = [
  ["xiaohe", "小鹤双拼"],
  ["ziranma", "自然码双拼"],
  ["shoudao", "首道双拼"],
  ["microsoft", "微软双拼"],
];

const partTitles: Record<ShuangpinCustomTablePart, string> = {
  initials: "声母",
  finals: "韵母",
  zero_initials: "零声母",
};

/** 一张表的规范写法：三部分都在，键转成小写，空键不写。导出和比较都用它。 */
export function normalizedShuangpinCustomProfile(
  profile: ShuangpinCustomProfile | undefined,
): Required<ShuangpinCustomProfile> {
  const part = (entries: Record<string, string> | undefined) =>
    Object.fromEntries(
      Object.entries(entries ?? {})
        .filter(([, key]) => key !== "")
        .map(([unit, key]) => [unit, key.toLowerCase()]),
    );
  return {
    initials: part(profile?.initials),
    finals: part(profile?.finals),
    zero_initials: part(profile?.zero_initials),
  };
}

export function sameShuangpinCustomProfile(
  left: ShuangpinCustomProfile | undefined,
  right: ShuangpinCustomProfile | undefined,
): boolean {
  const a = normalizedShuangpinCustomProfile(left);
  const b = normalizedShuangpinCustomProfile(right);
  return shuangpinCustomParts.every((part) => {
    const keys = Object.keys(a[part]);
    return (
      keys.length === Object.keys(b[part]).length &&
      keys.every((unit) => a[part][unit] === b[part][unit])
    );
  });
}

/** 一张表一个键都没填。 */
export function emptyShuangpinCustomProfile(profile: ShuangpinCustomProfile | undefined): boolean {
  const normalized = normalizedShuangpinCustomProfile(profile);
  return shuangpinCustomParts.every((part) => Object.keys(normalized[part]).length === 0);
}

/** 导出文件的内容：规范写法的 JSON，结尾带换行。 */
export function shuangpinCustomProfileExport(profile: ShuangpinCustomProfile): string {
  return `${JSON.stringify(normalizedShuangpinCustomProfile(profile), null, 2)}\n`;
}

export const SHUANGPIN_CUSTOM_EXPORT_NAME = "msime-shuangpin-custom.json";
/** 一张合法的表导出来不到 1 KB；导入时远超这个大小的文件一定不是键位表。 */
export const MAX_SHUANGPIN_CUSTOM_IMPORT_BYTES = 64 * 1024;

/**
 * 读导入文件的 JSON：只认上面那个形状，多出来的键（偏好的 `deny_unknown_fields`）、不是字符串的键位都拒绝，抛出的 `Error` 是给用户看的话。合不合法另由宿主校验。
 */
export function parseShuangpinCustomProfile(text: string): ShuangpinCustomProfile {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error("文件不是有效的 JSON。");
  }
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("文件里应当是一个包含 initials、finals、zero_initials 的对象。");
  const profile: ShuangpinCustomProfile = {};
  for (const [name, entries] of Object.entries(value)) {
    if (!(shuangpinCustomParts as readonly string[]).includes(name))
      throw new Error(`文件里有不认识的部分「${name}」，只能有 initials、finals、zero_initials。`);
    if (!entries || typeof entries !== "object" || Array.isArray(entries))
      throw new Error(`「${name}」应当是从单位到键的对象。`);
    const part: Record<string, string> = {};
    for (const [unit, key] of Object.entries(entries)) {
      if (typeof key !== "string") throw new Error(`「${name}」里 ${unit} 的键应当是字符串。`);
      part[unit] = key;
    }
    profile[name as ShuangpinCustomTablePart] = part;
  }
  return profile;
}

/** 宿主报告的问题写成一句给用户看的话。 */
export function shuangpinCustomProblemMessage(problem: ShuangpinCustomProblem): string {
  const part = problem.part ? partTitles[problem.part] : "";
  const unit = problem.unit ?? "";
  switch (problem.kind) {
    case "unknown_unit":
      return `${part}里没有「${unit}」这一项。`;
    case "duplicate_unit":
      return `${part}「${unit}」写了两次。`;
    case "missing_unit":
      return `${part}「${unit}」还没有指定键。`;
    case "invalid_key":
      return problem.part === "zero_initials"
        ? `零声母「${unit}」的编码「${problem.key ?? ""}」不能用：要两个键，第一个是字母，第二个是字母或 ;。`
        : problem.part === "initials"
          ? `声母「${unit}」的键「${problem.key ?? ""}」不能用：要一个字母。`
          : `韵母「${unit}」的键「${problem.key ?? ""}」不能用：要一个字母或 ;。`;
    case "initial_key_taken":
      return `声母「${unit}」不能放在 ${problem.key ?? ""} 键上：这个键已经是单字母声母或另一个声母的键。`;
    case "ambiguous_code":
      return `编码「${problem.code ?? ""}」既是「${problem.first ?? ""}」又是「${problem.second ?? ""}」，两个音节不能用同一个编码。`;
  }
}
