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
