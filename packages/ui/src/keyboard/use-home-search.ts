import { useState, type KeyboardEvent } from "react";
import type { Preferences } from "../core/preferences-model";
import { rootSettingsGroups, type RootPage } from "./root-settings-model";

/** 每个首页独立持有查询，分组和筛选值随宿主页面与偏好更新。 */
export function useHomeSearch(preferences: Preferences, rootPages?: readonly RootPage[]) {
  const [query, setQuery] = useState("");
  const groups = rootPages ? rootSettingsGroups(rootPages, preferences) : [];
  const needle = query.trim().toLowerCase();
  const matches = needle
    ? groups
        .flat()
        .filter(
          (row) =>
            row.title.toLowerCase().includes(needle) ||
            Boolean(row.value?.toLowerCase().includes(needle)),
        )
    : [];

  const searchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing || event.keyCode === 229) return;
    if (event.key !== "Escape" || !query) return;
    event.preventDefault();
    event.stopPropagation();
    setQuery("");
  };

  return { query, setQuery, groups, needle, matches, searchKeyDown };
}
