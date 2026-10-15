export type InputSourceStartupStatus = {
  /** `login_required` 表示已安装但需重新登录后才能添加；`not_installed` 表示尚未安装，由安装窗口引导。 */
  action: "installed" | "updated" | "up_to_date" | "not_installed" | "login_required" | "failed";
  /** 输入源是否在系统设置列表中；无法读取列表时为 `null`。 */
  enabled: boolean | null;
  bundled_version: string | null;
  installed_version: string | null;
  /** `/Library/Input Methods` 中与用户副本冲突的安装，需管理员移除；每次请求重新读取。 */
  system_bundles?: string[];
};

/** 用户仍需在系统设置中添加输入源；安装失败、未安装和等待重新登录时不要求添加。 */
export function inputSourceNeedsAdding(status: InputSourceStartupStatus | null): boolean {
  return (
    status?.enabled === false &&
    status.action !== "failed" &&
    status.action !== "login_required" &&
    status.action !== "not_installed"
  );
}

/** 等用户添加输入源或移除系统目录副本时继续重查。 */
export function inputSourceNeedsRecheck(status: InputSourceStartupStatus | null): boolean {
  return inputSourceNeedsAdding(status) || Boolean(status?.system_bundles?.length);
}

/** 最新且没有重复副本、也不需要添加的状态无需显示提示。 */
export function inputSourceNoticeVisible(status: InputSourceStartupStatus | null): boolean {
  return Boolean(
    status &&
    (status.action !== "up_to_date" ||
      status.enabled === false ||
      Boolean(status.system_bundles?.length)),
  );
}
