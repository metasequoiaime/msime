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
  linux = false,
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
  } = useAppInputModeRules({ value, windows, linux, pickApplication, applicationNames, onChange });
  return (
    <SettingsRowStack role="group" aria-label="应用例外">
      <SettingsGroupNote>
        切到这些应用时从指定的中文或英文开始，优先于上面的默认状态和记忆，「全局统一」下也生效；在应用里手动切换后，到下次切回这个应用之前不再套用。
        {linux &&
          " 按输入法框架报告的程序名匹配：Fcitx5 下经它的 GTK、Qt 输入法模块输入的程序会报告；IBus 1.5.27 起只有经 IBus 的 GTK 输入法模块输入的程序报告，通常是 X11 会话或设置了 GTK_IM_MODULE=ibus 的程序。GNOME Wayland 下的程序经 GNOME Shell 接入 IBus，和 Qt、XIM 程序一样没有程序名，不受应用例外影响。"}
      </SettingsGroupNote>
      {entries.length === 0 && (
        <SettingsGroupNote inline>还没有应用例外，所有应用都按上面的设置走。</SettingsGroupNote>
      )}
      {entries.map(([id, mode]) => {
        const name = names[id] ?? id;
        return (
          <Row key={id} title={name} detail={name === id ? undefined : id}>
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
          description="选择一个应用程序，新加的应用从中文开始，可在上面的列表里改成英文。"
          detail={full ? `已达 ${maxAppInputModeRules} 个上限，请先移除不再需要的。` : undefined}
        >
          <ActionButton action={pick} disabled={full || picking} label="添加应用…" />
        </Row>
      ) : (
        <Row title="添加应用例外">
          <input
            aria-label={windows ? "程序文件名" : linux ? "程序名" : "应用 Bundle ID"}
            value={draft}
            placeholder={
              windows ? "例如 code.exe" : linux ? "例如 firefox" : "例如 com.apple.Terminal"
            }
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
