import { useState } from "react";
import type { AppInputModeRule } from "../index";
import { Row, Select } from "../core/platform-controls";
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

export interface AppInputModeRulesSectionProps {
  value?: Record<string, AppInputModeRule>;
  /** Windows 按进程基名记规则，macOS 按 bundle id；决定提示文字和规范化方式。 */
  windows: boolean;
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

/** 应用例外：「中英文」组里的规则表。每条规则一行，可改模式或移除；末尾一行填应用标识后添加，新加的规则从中文开始，与 macOS 原生设置相同。 */
export function AppInputModeRulesSection({
  value,
  windows,
  onChange,
}: AppInputModeRulesSectionProps) {
  const [draft, setDraft] = useState("");
  const [problem, setProblem] = useState<AppInputModeRuleProblem | null>(null);
  const rules = value ?? {};
  const entries = sortedAppInputModeRules(rules);
  const add = () => {
    const id = normalizeAppInputModeRuleId(draft, windows);
    const found = appInputModeRuleProblem(id, rules, windows);
    setProblem(found);
    if (found) return;
    onChange({ ...rules, [id]: "chinese" });
    setDraft("");
  };
  return (
    <SettingsRowStack role="group" aria-label="应用例外">
      <SettingsGroupNote>
        切到这些应用时从指定的中文或英文开始，优先于上面的默认状态和记忆，「全局统一」下也生效；在应用里手动切换后，到下次切回这个应用之前不再套用。
        {entries.length === 0 && " 还没有应用例外，所有应用都按上面的设置走。"}
      </SettingsGroupNote>
      {entries.map(([id, mode]) => (
        <Row key={id} title={id}>
          <Select
            aria-label={`${id} 的输入模式`}
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
            ariaLabel={`移除 ${id} 的应用例外`}
            label="移除"
          />
        </Row>
      ))}
      <Row title="添加应用例外">
        <input
          aria-label={windows ? "程序文件名" : "应用 Bundle ID"}
          value={draft}
          placeholder={windows ? "例如 code.exe" : "例如 com.apple.Terminal"}
          autoCapitalize="off"
          spellCheck={false}
          onChange={(event) => {
            setDraft(event.target.value);
            setProblem(null);
          }}
          onKeyDown={(event) => {
            // 输入法组字时按回车是把组字串上屏，不是提交；WKWebView 在 compositionend 之后才送这个回车，keyCode 为 229。
            if (event.nativeEvent.isComposing || event.keyCode === 229) return;
            if (event.key === "Enter") add();
          }}
        />
        <ActionButton action={add} disabled={entries.length >= maxAppInputModeRules} label="添加" />
      </Row>
      {problem && <SettingsWarning role="alert">{problemText(problem, windows)}</SettingsWarning>}
    </SettingsRowStack>
  );
}
