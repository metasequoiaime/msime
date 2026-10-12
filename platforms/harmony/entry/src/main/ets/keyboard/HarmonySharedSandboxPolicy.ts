/**
 * 这个进程有没有拿到设置应用与键盘共用的共享沙箱。
 *
 * data-group-id 由 AppGallery Connect 签发，签名 Profile 的 `data-group-ids` 带着它，键盘扩展在 `module.json5` 的 `dataGroupIds` 里声明它，两个进程才能用 `context.getGroupDir` 拿到同一个目录。签名不带这个 ID、在模拟器上运行（不做这项授权校验）或者调用失败时拿不到目录，两边各用各的 `files/state`，设置改动传不到键盘。
 */
export interface HarmonySharedSandboxStatus {
  available: boolean;
  /** 共享目录；不可用时为空串。 */
  directory: string;
  /** 不可用的原因；可用时为空串。 */
  reason: string;
}

export class HarmonySharedSandboxPolicy {
  /** `groupDirectory` 是 `getGroupDir` 的结果，拿不到时为 null 或空串。 */
  static status(groupDirectory: string | null): HarmonySharedSandboxStatus {
    if (groupDirectory === null || groupDirectory.length === 0) {
      return { available: false, directory: "", reason: "group-directory-unavailable" };
    }
    return { available: true, directory: groupDirectory, reason: "" };
  }
}
