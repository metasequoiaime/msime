import { useId, useState, type ReactNode } from "react";
import * as settings from "./settings-style";

interface SettingsTab<T extends string> {
  id: T;
  title: string;
}

/** 桌面分区保持挂载，手机继续按原顺序展示全部内容；标签选择不写入偏好。 */
export function useSettingsTabs<T extends string>(
  tabs: readonly SettingsTab<T>[],
  label: string,
  enabled: boolean,
) {
  const [selection, setSelection] = useState<T>(tabs[0].id);
  const selected = tabs.some((tab) => tab.id === selection) ? selection : tabs[0].id;
  const id = useId();
  const tabList = enabled ? (
    <div className={settings.candidateTabs} role="tablist" aria-label={label}>
      {tabs.map((tab, index) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          id={`${id}-${tab.id}-tab`}
          aria-controls={`${id}-${tab.id}-panel`}
          aria-selected={selected === tab.id}
          tabIndex={selected === tab.id ? 0 : -1}
          onClick={() => setSelection(tab.id)}
          onKeyDown={(event) => {
            let next: number;
            if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
            else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
            else if (event.key === "Home") next = 0;
            else if (event.key === "End") next = tabs.length - 1;
            else return;
            event.preventDefault();
            setSelection(tabs[next].id);
            document.getElementById(`${id}-${tabs[next].id}-tab`)?.focus();
          }}
        >
          {tab.title}
        </button>
      ))}
    </div>
  ) : null;
  const panel = (tab: T, children: ReactNode) =>
    enabled ? (
      <div
        role="tabpanel"
        id={`${id}-${tab}-panel`}
        aria-labelledby={`${id}-${tab}-tab`}
        hidden={selected !== tab}
        tabIndex={0}
      >
        <div className="flex flex-col gap-6">{children}</div>
      </div>
    ) : (
      children
    );
  // `select` 让外壳从页面之外切换分类，例如功能页上的「浏览更多词库」打开「社区」标签的「词库」。
  return { tabList, panel, selected, select: setSelection };
}
