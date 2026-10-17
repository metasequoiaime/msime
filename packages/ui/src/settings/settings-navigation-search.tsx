import { isComposingKeyEvent } from "../core/ime-key-event";
import type { SettingsPageGroup } from "./settings-page-projections";
import type { SettingsPageId } from "./settings-page-registry";
import * as settings from "./settings-style";

export interface SettingsNavigationSearchProps {
  groups: readonly SettingsPageGroup[];
  value: string;
  onChange: (value: string) => void;
  onSelectPage: (page: SettingsPageId) => void;
}

/** 设置侧栏与标题栏共用的搜索框；组词期间不抢回车和 Escape。 */
export function SettingsNavigationSearch({
  groups,
  value,
  onChange,
  onSelectPage,
}: SettingsNavigationSearchProps) {
  return (
    <>
      <svg
        className={settings.searchGlyph}
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        aria-hidden="true"
      >
        <circle cx="7" cy="7" r="5" />
        <path d="m11 11 3.5 3.5" />
      </svg>
      <input
        type="search"
        className={settings.searchInput}
        aria-label="搜索设置"
        placeholder="搜索"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (isComposingKeyEvent(event)) return;
          if (event.key === "Escape") onChange("");
          if (event.key !== "Enter") return;
          const first = groups[0]?.pages[0];
          if (first) onSelectPage(first.id);
        }}
      />
    </>
  );
}
