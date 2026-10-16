import { isComposingKeyEvent } from "../core/ime-key-event";
import { Row, Select } from "../core/platform-controls";
import { ActionButton } from "./action-button";
import { maxAppInputModeRules, type AppInputModeRule } from "./app-input-mode-rules";
import type { UseAppInputModeRulesOptions } from "./use-app-input-mode-rules";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsRowStack } from "./settings-row-stack";
import { SettingsWarning } from "./settings-warning";
import { useAppInputModeRules } from "./use-app-input-mode-rules";

export type { PickedApplication } from "./app-input-mode-rules";

export interface AppInputModeRulesSectionProps extends UseAppInputModeRulesOptions {}

/** 应用例外：「中英文」组里的规则表。每条规则一行，显示应用名称，可改模式或移除；末尾的「添加应用…」弹出系统的选应用对话框，新加的规则从中文开始，与 macOS 原生设置相同。宿主不能弹对话框时才退回手填应用标识。 */
export function AppInputModeRulesSection({
  value,
  windows,
  pickApplication,
  applicationNames,
  onChange,
}: AppInputModeRulesSectionProps) {
  const {
    draft,
    warning,
    names,
    entries,
    full,
    picking,
    edit,
    addTyped,
    pick,
    changeMode,
    remove,
  } = useAppInputModeRules({ value, windows, pickApplication, applicationNames, onChange });
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
              onChange={(event) => changeMode(id, event.target.value as AppInputModeRule)}
            >
              <option value="chinese">中文</option>
              <option value="english">英文</option>
            </Select>
            <ActionButton
              action={() => remove(id)}
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
          <ActionButton action={pick} disabled={full || picking} label="添加应用…" />
        </Row>
      ) : (
        <Row title="添加应用例外">
          <input
            aria-label={windows ? "程序文件名" : "应用 Bundle ID"}
            value={draft}
            placeholder={windows ? "例如 code.exe" : "例如 com.apple.Terminal"}
            autoCapitalize="off"
            spellCheck={false}
            onChange={(event) => edit(event.target.value)}
            onKeyDown={(event) => {
              // 输入法组字时按回车是把组字串上屏，不是提交；WKWebView 在 compositionend 之后才送这个回车，keyCode 为 229。
              if (isComposingKeyEvent(event)) return;
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
