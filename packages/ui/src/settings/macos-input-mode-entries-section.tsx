import { type ReactNode, useId, useState } from "react";

import { Row } from "../core/platform-controls";
import { ActionButton } from "./action-button";
import { macosInputModeProductName, type MacosInputModeGroup } from "./macos-input-modes";
import { useMacosInputModes, type UseMacosInputModesOptions } from "./use-macos-input-modes";

export {
  macosInputModeEntries,
  macosInputModeEntriesFor,
  INPUT_MODE_RECHECK_MS,
  INPUT_MODE_RECHECK_WINDOW_MS,
  type MacosInputModesClient,
} from "./macos-input-modes";

export interface MacosInputModeEntriesSectionProps extends UseMacosInputModesOptions {}

/**
 * 「菜单栏入口」：列出还没加入输入法列表的模式，告诉用户在系统设置里去哪个语言下添加。
 *
 * macOS 27 不允许进程启用键盘输入模式（`TISEnableInputSource` 返回 noErr 而状态不变），输入法和设置应用都没法替用户加；而系统设置的「添加」对话框按语言分组，粤在「粤语」、注在「繁体中文」，只看「简体中文」会以为它们不存在。
 *
 * 「添加」对话框所在的键盘设置扩展读的是系统在登录时写下的输入源缓存，替换输入法 bundle 后不会刷新：0.51.1 升级上来时，新加的「藏」一直不出现在「藏语」下。设置应用装好或更新输入法后会删掉这份缓存并让扩展重新启动（`macos_input_source.rs` 的 `refresh_system_input_source_lists`），新入口不用重新登录就能在对话框里找到，所以提示里不再叫用户注销。
 */
export function MacosInputModeEntriesSection({
  client,
  scheme,
  inputSchemes,
  edition,
  onError,
}: MacosInputModeEntriesSectionProps) {
  const { enabled, missing, current, groups, opening, openSettings } = useMacosInputModes({
    client,
    scheme,
    inputSchemes,
    edition,
    onError,
  });
  if (!enabled) return null;

  let description: ReactNode;
  if (missing.length === 0) {
    description = `输入法菜单里已经有${edition?.display_name ?? macosInputModeProductName}的全部入口。`;
  } else {
    description = (
      <>
        <span className="block">
          {current
            ? `菜单栏里还没有「${current.name}」，到「${current.language}」下添加后才能从菜单栏切过去。`
            : `还有 ${missing.length} 个入口没加进输入法菜单。`}
        </span>
        <MissingEntries groups={groups} count={missing.length} />
      </>
    );
  }

  return (
    <Row title="菜单栏入口" description={description}>
      {missing.length > 0 && (
        <ActionButton
          action={openSettings}
          disabled={opening}
          className="secondary"
          label="打开键盘设置"
        />
      )}
    </Row>
  );
}

/** 还没加入的入口默认收起，展开后先说怎么添加，再按系统设置「添加」对话框里的语言分组，每组一行：语言在前，入口名排在后面，顺序与输入法菜单一致。全版本缺的入口可以有八九个，平铺开来会把这一行撑成一大段，而大多数人只用其中一两个。展开状态放在 React 里（`aria-expanded` 的按钮加 `hidden`），不用 `<details>`：装好的设置应用里曾看到它一打开就是展开的，原因没查清，React 状态的行为可以在测试里点按钮核对。 */
function MissingEntries({
  groups,
  count,
}: {
  groups: readonly MacosInputModeGroup[];
  count: number;
}) {
  const [open, setOpen] = useState(false);
  const listId = useId();
  return (
    <>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => setOpen(!open)}
        className="mt-1 inline-flex cursor-pointer items-center gap-1 border-0 bg-transparent p-0 [font:inherit] [color:var(--p-accent-text)]"
      >
        {open ? "收起" : `查看怎么添加和全部 ${count} 个入口`}
        <span
          aria-hidden="true"
          className={`transition-transform motion-reduce:transition-none ${open ? "-rotate-90" : "rotate-90"}`}
        >
          ›
        </span>
      </button>
      <span id={listId} hidden={!open} className="block">
        <span className="mt-1.5 block">
          {
            "macOS 只能手动添加：点「打开键盘设置」，在「输入法」一行点「编辑…」，再点左下角「+」，在左栏按下面的语言找。"
          }
        </span>
        <span role="list" aria-label="还没加入的入口" className="mt-2 flex flex-col gap-1.5">
          {groups.map(({ language, entries }) => (
            <span key={language} role="listitem" className="flex items-start gap-1.5">
              <span className="w-16 shrink-0 py-0.5">{language}</span>
              <span className="flex flex-wrap gap-1.5">
                {entries.map((entry) => (
                  <span
                    key={entry.mode}
                    className="rounded-[var(--p-seg-item-r)] bg-[var(--p-seg-bg)] px-2 py-0.5 whitespace-nowrap [color:var(--p-text)]"
                  >
                    {entry.name}
                  </span>
                ))}
              </span>
            </span>
          ))}
        </span>
      </span>
    </>
  );
}
