import { asciiLowercase, utf8ByteLength } from "../core/text";

/** 对应 `client-core::preferences::AppInputModeRule`。 */
export type AppInputModeRule = "chinese" | "english";

/** 宿主选中的应用：`id` 是规则用的标识（macOS 的 Bundle ID，Windows 的程序文件名，Linux 的程序名），`name` 是给人看的名称。 */
export interface PickedApplication {
  id: string;
  name: string;
}

/** 与 `client-core::preferences` 的 `MAX_APP_INPUT_MODE_RULES`、`MAX_APP_INPUT_MODE_RULE_ID_BYTES` 一致。 */
export const maxAppInputModeRules = 32;
export const maxAppInputModeRuleIdBytes = 64;

export type AppInputModeRuleProblem =
  | "empty"
  | "not_exe"
  | "invalid_character"
  | "too_long"
  | "duplicate"
  | "too_many";

/** 用户填的应用标识。Windows 上是进程的可执行文件名：粘贴了完整路径时只取文件名，资源管理器「复制文件地址」（Ctrl+Shift+C）给路径加的一对双引号也去掉，ASCII 字母转小写，和 Server 不分大小写的比较一致；macOS 上是 bundle id，原样保留大小写；Linux 上是输入法框架报告的程序名，粘贴了 `/usr/bin/firefox` 这样的路径时只取文件名，大小写原样保留（宿主比较时不分 ASCII 大小写）。两端都去掉首尾空白（含不换行空格和全角空格）。 */
export function normalizeAppInputModeRuleId(raw: string, windows: boolean, linux = false): string {
  const trimmed = raw.trim();
  if (linux) return trimmed.slice(trimmed.lastIndexOf("/") + 1).trim();
  if (!windows) return trimmed;
  const unquoted =
    trimmed.length >= 2 && trimmed.startsWith('"') && trimmed.endsWith('"')
      ? trimmed.slice(1, -1).trim()
      : trimmed;
  const base = unquoted.slice(Math.max(unquoted.lastIndexOf("\\"), unquoted.lastIndexOf("/")) + 1);
  return asciiLowercase(base.trim());
}

/** 规范化后的标识能否加进 `rules`，规则与偏好库的校验相同；Windows 另外要求 `.exe` 结尾，因为 Server 只拿进程基名去比。 */
export function appInputModeRuleProblem(
  id: string,
  rules: Readonly<Record<string, AppInputModeRule>>,
  windows: boolean,
): AppInputModeRuleProblem | null {
  if (!id) return "empty";
  if (utf8ByteLength(id) > maxAppInputModeRuleIdBytes) return "too_long";
  // 与 Rust 的 `char::is_control` 相同（U+0000–U+001F、U+007F–U+009F），再加上两个路径分隔符。
  for (const character of id) {
    const code = character.codePointAt(0) ?? 0;
    if (code < 0x20 || (code >= 0x7f && code <= 0x9f) || character === "\\" || character === "/")
      return "invalid_character";
  }
  if (windows && !id.toLowerCase().endsWith(".exe")) return "not_exe";
  const folded = asciiLowercase(id);
  if (Object.keys(rules).some((existing) => asciiLowercase(existing) === folded))
    return "duplicate";
  if (Object.keys(rules).length >= maxAppInputModeRules) return "too_many";
  return null;
}

/** 规则表里的条目，按标识排序，两次打开页面顺序不变。 */
export function sortedAppInputModeRules(
  rules: Readonly<Record<string, AppInputModeRule>> | undefined,
): [string, AppInputModeRule][] {
  return Object.entries(rules ?? {}).sort(([left], [right]) =>
    left < right ? -1 : left > right ? 1 : 0,
  );
}

/** 应用标识校验失败时的说明，手填和系统选择共用。 */
export function appInputModeRuleProblemText(
  problem: AppInputModeRuleProblem,
  windows: boolean,
  linux = false,
): string {
  switch (problem) {
    case "empty":
      return windows
        ? "请填写程序文件名，例如 code.exe。"
        : linux
          ? "请填写程序名，例如 firefox。"
          : "请填写应用的 Bundle ID，例如 com.apple.Terminal。";
    case "not_exe":
      return "程序文件名要以 .exe 结尾，例如 code.exe。";
    case "invalid_character":
      return "不能包含控制字符，也不能带 \\ 或 /。";
    case "too_long":
      return `应用标识不能超过 ${maxAppInputModeRuleIdBytes} 个字节。`;
    case "duplicate":
      return "这个应用已经有例外了，在下面的列表里改它的模式即可。";
    case "too_many":
      return `最多 ${maxAppInputModeRules} 个应用例外，请先移除不再需要的。`;
  }
}
