/**
 * 「剪贴板删除数据验证」（#10408，与 Android 的 `ClipboardDeleteAuthenticationPolicy` 同一套规则）的切换规则和存放它的文件。
 *
 * 默认关闭。打开后，键盘剪贴板面板里删除本机记录（管理行的「删除」、2in1 行尾的「删除」和「清空」）都要先通过系统认证（锁屏密码或生物识别）。打开和关闭都要认证，第一次打开也不例外，这是它和「防止截屏」唯一的区别。设备上没有可用的系统认证时一律不许切换；打开以后锁屏密码被移除，删除和关闭都做不了，只有重新设锁屏密码才能解开，与「防止截屏」的先例相同。鸿蒙键盘没有访客剪贴板，也没有删除撤销，所以只看开关。
 *
 * 开关只属于这台设备：写在设置应用的 `files/state/clipboard-delete-authentication.json`，不在共享偏好里，不随账号同步，鸿蒙也没有本地备份。键盘扩展是独立沙箱，经共享沙箱镜像（`SharedStateMirrorPolicy` 的 `clipboard-delete-authentication` 条目）单向传给键盘，键盘不发布它，也改不回去。
 *
 * 本文件只有纯规则，读写文件在 `ClipboardDeleteAuthentication.ets`，认证用 `ScreenCaptureAuthentication`。
 */

/** 状态目录里的文件名。 */
export const CLIPBOARD_DELETE_AUTHENTICATION_FILE: string = "clipboard-delete-authentication.json";
/** 共享沙箱镜像里这条记录的条目名。 */
export const CLIPBOARD_DELETE_AUTHENTICATION_ENTRY: string = "clipboard-delete-authentication";
/** 文档只有一个布尔字段；更大的内容在解析前就拒绝。 */
export const CLIPBOARD_DELETE_AUTHENTICATION_MAX_BYTES: number = 4096;
/** 系统认证不可用时给用户的提示：设置页拒绝切换和键盘里拒绝删除都用这一句，与 Android 相同。 */
export const CLIPBOARD_DELETE_AUTHENTICATION_UNAVAILABLE_MESSAGE: string =
  "请先在系统设置中开启锁屏验证（指纹、图案、密码等）后再使用";

/** 一次切换该怎么处理。 */
export enum ClipboardDeleteAuthenticationDecision {
  /** 下一值与当前值相同，什么也不做。 */
  UNCHANGED = "unchanged",
  /** 先通过系统认证再保存。 */
  AUTHENTICATE = "authenticate",
  /** 设备没有可用的系统认证，拒绝切换并保持原值。 */
  UNAVAILABLE = "unavailable",
}

/** 删除一条记录或清空之前该怎么处理。 */
export enum ClipboardDeleteGate {
  /** 开关关着，直接删。 */
  DELETE = "delete",
  /** 先通过系统认证再删。 */
  AUTHENTICATE = "authenticate",
  /** 开关开着但设备没有可用的系统认证：不删，提示先开启锁屏。 */
  UNAVAILABLE = "unavailable",
}

interface ClipboardDeleteAuthenticationDocument {
  enabled?: boolean;
}

export class ClipboardDeleteAuthenticationPolicy {
  /**
   * @param enabled 当前是否开着
   * @param next 用户要切换到的值
   * @param authenticationAvailable 系统认证此刻是否可用（`userAuth.getAvailableStatus` 对锁屏密码不报错）
   */
  static decide(
    enabled: boolean,
    next: boolean,
    authenticationAvailable: boolean,
  ): ClipboardDeleteAuthenticationDecision {
    if (next === enabled) return ClipboardDeleteAuthenticationDecision.UNCHANGED;
    return authenticationAvailable
      ? ClipboardDeleteAuthenticationDecision.AUTHENTICATE
      : ClipboardDeleteAuthenticationDecision.UNAVAILABLE;
  }

  /** 删除前的判断：开关关着直接删，开着时按系统认证此刻是否可用决定认证还是拒绝。 */
  static gate(enabled: boolean, authenticationAvailable: boolean): ClipboardDeleteGate {
    if (!enabled) return ClipboardDeleteGate.DELETE;
    return authenticationAvailable
      ? ClipboardDeleteGate.AUTHENTICATE
      : ClipboardDeleteGate.UNAVAILABLE;
  }

  /** 读文件内容，不信任它：没有文件、过大、格式不对时按关闭处理。键盘那一份只来自设置应用的镜像，改不了设置应用里那一份。 */
  static parse(document: string | null): boolean {
    if (
      document === null ||
      document.length === 0 ||
      document.length > CLIPBOARD_DELETE_AUTHENTICATION_MAX_BYTES
    ) {
      return false;
    }
    try {
      const raw: ClipboardDeleteAuthenticationDocument = JSON.parse(
        document,
      ) as ClipboardDeleteAuthenticationDocument;
      if (raw === null || typeof raw !== "object" || Array.isArray(raw)) return false;
      return raw.enabled === true;
    } catch (error) {
      return false;
    }
  }

  static serialize(enabled: boolean): string {
    const document: ClipboardDeleteAuthenticationDocument = { enabled: enabled };
    return JSON.stringify(document);
  }
}
