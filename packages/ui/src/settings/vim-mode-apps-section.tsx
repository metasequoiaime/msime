import { isComposingKeyEvent } from "../core/ime-key-event";
import { Row } from "../core/platform-controls";
import { ActionButton } from "./action-button";
import { maxAppInputModeRules } from "./app-input-mode-rules";
import type { UseVimModeAppsOptions } from "./use-vim-mode-apps";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsRowStack } from "./settings-row-stack";
import { SettingsWarning } from "./settings-warning";
import { useVimModeApps } from "./use-vim-mode-apps";

export interface VimModeAppsSectionProps extends UseVimModeAppsOptions {}

/** Vim 模式应用名单：「中英文」组里的应用列表。名单里的前台应用收到无修饰 Esc 时输入法同时切到英文，Vim 用户可以立刻输入 hjkl；组字开着时 Esc 仍只取消组字。与应用例外同一张选应用对话框和手填入口，只是没有模式下拉。 */
export function VimModeAppsSection({
  value,
  windows,
  pickApplication,
  applicationNames,
  onChange,
}: VimModeAppsSectionProps) {
  const { draft, warning, names, entries, full, picking, edit, addTyped, pick, remove } =
    useVimModeApps({ value, windows, pickApplication, applicationNames, onChange });
  return (
    <SettingsRowStack role="group" aria-label="Vim 模式">
      <SettingsGroupNote>
        在这些应用里，不带修饰键的 Esc 在交给应用的同时把水杉切到英文；组字或候选开着时 Esc
        仍只取消组字，再按一次才切。
      </SettingsGroupNote>
      {entries.length === 0 && (
        <SettingsGroupNote inline>名单是空的，Esc 的行为不变。</SettingsGroupNote>
      )}
      {entries.map((id) => {
        const name = names[id] ?? id;
        return (
          <Row key={id} title={name} detail={name === id ? undefined : id}>
            <ActionButton
              action={() => remove(id)}
              ariaLabel={`把 ${name} 从 Vim 模式名单移除`}
              label="移除"
            />
          </Row>
        );
      })}
      {pick ? (
        <Row
          title="添加应用"
          description="选择一个应用程序，加入 Vim 模式名单。"
          detail={full ? `已达 ${maxAppInputModeRules} 个上限，请先移除不再需要的。` : undefined}
        >
          <ActionButton action={pick} disabled={full || picking} label="添加应用…" />
        </Row>
      ) : (
        <Row title="添加应用">
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
