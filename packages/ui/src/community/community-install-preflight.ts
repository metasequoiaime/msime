export type CommunityInstallPreflightResult = "stale" | "clear" | "conflict";

/** 读取本地目录并判断是否需要确认替换；目录扫描失败时交给安装接口决定。 */
export async function communityInstallPreflight<T>(
  loadCatalog: () => Promise<T>,
  isCurrent: () => boolean,
  hasConflict: (catalog: T) => boolean,
): Promise<CommunityInstallPreflightResult> {
  const catalog = await loadCatalog().catch(() => null);
  if (!isCurrent()) return "stale";
  return catalog !== null && hasConflict(catalog) ? "conflict" : "clear";
}
