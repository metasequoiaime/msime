import { isComposingKeyEvent } from "../core/ime-key-event";
import { SearchGlyph } from "../core/search-glyph";
import type { SettingsPageGroup } from "./settings-page-projections";
import type { SettingsPageId } from "./settings-page-registry";
import * as settings from "./settings-style";

export interface SettingsNavigationSearchProps {
  groups: readonly SettingsPageGroup[];
  value: string;
  onChange: (value: string) => void;
  onSelectPage: (page: SettingsPageId) => void;
  resultNavigation?: {
    active: number;
    count: number;
    listId: string;
    setActive: (index: number) => void;
    select: (index: number) => void;
  };
}

/** 设置侧栏与标题栏共用的搜索框；组词期间不抢回车和 Escape。 */
export function SettingsNavigationSearch({
  groups,
  value,
  onChange,
  onSelectPage,
  resultNavigation,
}: SettingsNavigationSearchProps) {
  return (
    <>
      <SearchGlyph className={settings.searchGlyph} />
      <input
        type="search"
        aria-controls={resultNavigation?.listId}
        aria-activedescendant={
          resultNavigation?.count
            ? `${resultNavigation.listId}-${resultNavigation.active}`
            : undefined
        }
        className={settings.searchInput}
        aria-label="搜索设置"
        placeholder="搜索"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (isComposingKeyEvent(event)) return;
          if (event.key === "Escape") onChange("");
          if (resultNavigation && ["ArrowDown", "ArrowUp", "Enter"].includes(event.key)) {
            event.preventDefault();
            const { active, count, setActive, select } = resultNavigation;
            if (count && event.key === "Enter") select(active);
            else if (count)
              setActive((active + (event.key === "ArrowDown" ? 1 : -1) + count) % count);
            return;
          }
          if (event.key !== "Enter") return;
          const first = groups[0]?.pages[0];
          if (first) onSelectPage(first.id);
        }}
      />
    </>
  );
}
