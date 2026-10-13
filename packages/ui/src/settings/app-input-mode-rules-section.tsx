import { useEffect, useState } from "react";
import type { AppInputModeRule } from "../index";
import { Row, Select } from "../core/platform-controls";
import { errorCode } from "../core/error-code";
import { ActionButton } from "./action-button";
import {
  appInputModeRuleProblem,
  maxAppInputModeRuleIdBytes,
  maxAppInputModeRules,
  normalizeAppInputModeRuleId,
  sortedAppInputModeRules,
  type AppInputModeRuleProblem,
} from "./app-input-mode-rules";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsRowStack } from "./settings-row-stack";
import { SettingsWarning } from "./settings-warning";

/** 宿主选中的应用：`id` 是规则用的标识（macOS 的 Bundle ID，Windows 的程序文件名），`name` 是给人看的名称。 */
export interface PickedApplication {
  id: string;
  name: string;
}

export interface AppInputModeRulesSectionProps {
  value?: Record<string, AppInputModeRule>;
  /** Windows 按进程基名记规则，macOS 按 bundle id；决定提示文字和规范化方式。 */
  windows: boolean;
  /** 宿主弹出系统的选应用对话框，用户取消时为 null。有它时只提供「添加应用…」，没有时才退回手填标识。 */
  pickApplication?: () => Promise<PickedApplication | null>;
  /** 按规则标识查本机装着的应用名称，查不到的不返回；没有它或查不到时列表显示标识本身。 */
  applicationNames?: (ids: string[]) => Promise<Record<string, string>>;
  onChange: (rules: Record<string, AppInputModeRule>) => void;
}

function problemText(problem: AppInputModeRuleProblem, windows: boolean): string {
  switch (problem) {
    case "empty":
      return windows
        ? "请填写程序文件名，例如 code.exe。"
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

/** 应用例外：「中英文」组里的规则表。每条规则一行，显示应用名称，可改模式或移除；末尾的「添加应用…」弹出系统的选应用对话框，新加的规则从中文开始，与 macOS 原生设置相同。宿主不能弹对话框时才退回手填应用标识。 */
export function AppInputModeRulesSection({
  value,
  windows,
  pickApplication,
  applicationNames,
  onChange,
}: AppInputModeRulesSectionProps) {
  const [draft, setDraft] = useState("");
  const [warning, setWarning] = useState<string | null>(null);
  // 选应用时就知道名称；之前就有的规则（包括别的机器同步来的）向宿主查一次。
  const [names, setNames] = useState<Record<string, string>>({});
  const rules = value ?? {};
  const entries = sortedAppInputModeRules(rules);
  const unnamed = entries.map(([id]) => id).filter((id) => !(id in names));
  const unnamedKey = unnamed.join("\n");
  useEffect(() => {
    if (!applicationNames || unnamedKey === "") return;
    let current = true;
    const ids = unnamedKey.split("\n");
    applicationNames(ids).then(
      (found) => {
        if (!current) return;
        // 查不到的也记下（名称就是标识），同一批标识不再反复去问。
        setNames((known) => ({
          ...known,
          ...Object.fromEntries(ids.map((id) => [id, found[id] || id])),
        }));
      },
      () => {
        if (current)
          setNames((known) => ({ ...known, ...Object.fromEntries(ids.map((id) => [id, id])) }));
      },
    );
    return () => {
      current = false;
    };
  }, [applicationNames, unnamedKey]);

  const addRule = (raw: string): boolean => {
    const id = normalizeAppInputModeRuleId(raw, windows);
    const found = appInputModeRuleProblem(id, rules, windows);
    setWarning(found && problemText(found, windows));
    if (found) return false;
    onChange({ ...rules, [id]: "chinese" });
    return true;
  };
  const addTyped = () => {
    if (addRule(draft)) setDraft("");
  };
  const pick =
    pickApplication &&
    (async () => {
      setWarning(null);
      let picked: PickedApplication | null;
      try {
        picked = await pickApplication();
      } catch (error) {
        setWarning(
          errorCode(error) === "not_application"
            ? "所选项目不是应用程序，无法加入应用例外。"
            : "无法打开选择应用的对话框，请稍后再试。",
        );
        return;
      }
      if (!picked) return;
      const id = normalizeAppInputModeRuleId(picked.id, windows);
      const name = picked.name || id;
      if (addRule(picked.id)) setNames((known) => ({ ...known, [id]: name }));
    });
  const full = entries.length >= maxAppInputModeRules;
  return (
    <SettingsRowStack role="group" aria-label="应用例外">
      <SettingsGroupNote>
        切到这些应用时从指定的中文或英文开始，优先于上面的默认状态和记忆，「全局统一」下也生效；在应用里手动切换后，到下次切回这个应用之前不再套用。
        {entries.length === 0 && " 还没有应用例外，所有应用都按上面的设置走。"}
      </SettingsGroupNote>
      {entries.map(([id, mode]) => {
        const name = names[id] ?? id;
        return (
          <Row key={id} title={name} description={name === id ? undefined : id}>
            <Select
              aria-label={`${name} 的输入模式`}
              value={mode}
              onChange={(event) =>
                onChange({ ...rules, [id]: event.target.value as AppInputModeRule })
              }
            >
              <option value="chinese">中文</option>
              <option value="english">英文</option>
            </Select>
            <ActionButton
              action={() => {
                const next = { ...rules };
                delete next[id];
                onChange(next);
              }}
              ariaLabel={`移除 ${name} 的应用例外`}
              label="移除"
            />
          </Row>
        );
      })}
      {pick ? (
        <Row
          title="添加应用例外"
          description={
            full
              ? `已达 ${maxAppInputModeRules} 个上限，请先移除不再需要的。`
              : "选择一个应用程序，新加的应用从中文开始，可在上面的列表里改成英文。"
          }
        >
          <ActionButton action={pick} disabled={full} label="添加应用…" />
        </Row>
      ) : (
        <Row title="添加应用例外">
          <input
            aria-label={windows ? "程序文件名" : "应用 Bundle ID"}
            value={draft}
            placeholder={windows ? "例如 code.exe" : "例如 com.apple.Terminal"}
            autoCapitalize="off"
            spellCheck={false}
            onChange={(event) => {
              setDraft(event.target.value);
              setWarning(null);
            }}
            onKeyDown={(event) => {
              // 输入法组字时按回车是把组字串上屏，不是提交；WKWebView 在 compositionend 之后才送这个回车，keyCode 为 229。
              if (event.nativeEvent.isComposing || event.keyCode === 229) return;
              if (event.key === "Enter") addTyped();
            }}
          />
          <ActionButton action={addTyped} disabled={full} label="添加" />
        </Row>
      )}
      {warning && <SettingsWarning role="alert">{warning}</SettingsWarning>}
    </SettingsRowStack>
  );
}
