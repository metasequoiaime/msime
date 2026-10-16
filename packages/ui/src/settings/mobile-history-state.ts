/** 合并宿主的历史状态，默认标记属于设置页，再由本次路由字段覆盖。 */
export function createMobileSettingsHistoryState(
  current: unknown,
  patch: Record<string, unknown>,
): Record<string, unknown> {
  return {
    ...(current && typeof current === "object" ? current : {}),
    msimeSettings: true,
    ...patch,
  };
}
