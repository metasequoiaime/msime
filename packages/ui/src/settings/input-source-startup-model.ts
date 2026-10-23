export type InputSourceStartupStatus = {
  /** 等待登录、主动授权、尚未安装和授权取消均不能宣称已经完成。 */
  action:
    | "installed"
    | "updated"
    | "up_to_date"
    | "not_installed"
    | "login_required"
    | "authorization_required"
    | "cancelled"
    | "failed";
  /** 输入源是否在系统设置列表中；无法读取列表时为 `null`。 */
  enabled: boolean | null;
  bundled_version: string | null;
  installed_version: string | null;
  /** 当前生效安装的范围，旧宿主可以省略。 */
  installation_scope?: "user" | "system";
  /** 两个目录同时有本输入法时，与用户副本冲突的系统安装；每次请求重新读取。 */
  system_bundles?: string[];
};

/** 用户仍需在系统设置中添加输入源；安装失败、未安装和等待重新登录时不要求添加。 */
export function inputSourceNeedsAdding(status: InputSourceStartupStatus | null): boolean {
  return (
    status?.enabled === false && ["installed", "updated", "up_to_date"].includes(status.action)
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
