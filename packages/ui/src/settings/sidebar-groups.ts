import { macosSidebarGroups } from "./macos-sidebar-groups";

export function groupSidebarPages<T extends { id: string }>(
  pages: readonly T[],
  macos: boolean,
  groups: readonly (readonly string[])[],
): T[][] {
  if (!macos) return [Array.from(pages)];
  const remaining = new Map(pages.map((item) => [item.id, item]));
  const result = groups
    .map((ids) =>
      ids.flatMap((id) => {
        const item = remaining.get(id);
        if (!item) return [];
        remaining.delete(id);
        return [item];
      }),
    )
    .filter((group) => group.length > 0);
  const extra = [...remaining.values()];
  if (extra.length > 0) result.splice(Math.max(result.length - 1, 0), 0, extra);
  return result;
}

export interface SettingsSidebarGroupsOptions {
  mobile: boolean;
  hiddenPageIds: readonly string[];
  macos: boolean;
}

/** Filters pages unavailable in the sidebar and applies the platform's grouping order. */
export function settingsSidebarGroups<T extends { id: string }>(
  availablePages: readonly T[],
  { mobile, hiddenPageIds, macos }: SettingsSidebarGroupsOptions,
): T[][] {
  const sidebarPages = availablePages.filter(
    (item) => item.id !== "more" && !(mobile && hiddenPageIds.includes(item.id)),
  );
  return groupSidebarPages(sidebarPages, macos, macosSidebarGroups);
}
