/**
 * 「防止截屏」（#10194，Android 在 #10159）的切换规则和存放它的文件。
 *
 * 默认关闭；第一次打开不需要认证，打开过一次以后，再开或再关都要先通过系统认证（锁屏密码或生物识别），只有在系统设置里清除应用数据才能绕过。设备上没有可用的系统认证时一律不许切换：第一次打开也不行，否则打开之后就再也关不掉。与 Android 的 `ScreenCaptureProtectionPolicy.decide` 逐条相同。
 *
 * 开关只属于这台设备：写在设置应用的 `files/state/screen-capture-protection.json`，不在共享偏好里，不随账号同步，鸿蒙也没有本地备份。键盘扩展是独立沙箱，读不到设置应用的文件，所以经共享沙箱镜像（`SharedStateMirrorPolicy` 的 `screen-capture-protection` 条目）单向传给键盘，键盘不发布它。
 *
 * 本文件只有纯规则，读写文件、认证和窗口在 `ScreenCaptureProtection.ets`。
 */

/** 状态目录里的文件名。 */
export const SCREEN_CAPTURE_PROTECTION_FILE: string = "screen-capture-protection.json";
/** 共享沙箱镜像里这条记录的条目名。 */
export const SCREEN_CAPTURE_PROTECTION_ENTRY: string = "screen-capture-protection";
/** 文档只有两个布尔字段；更大的内容在解析前就拒绝。 */
export const SCREEN_CAPTURE_PROTECTION_MAX_BYTES: number = 4096;

/** 一次切换该怎么处理。 */
export enum ScreenCaptureProtectionDecision {
  /** 下一值与当前值相同，什么也不做。 */
  UNCHANGED = "unchanged",
  /** 直接保存：只有从未打开过时的第一次打开。 */
  SAVE = "save",
  /** 先通过系统认证再保存。 */
  AUTHENTICATE = "authenticate",
  /** 设备没有可用的系统认证，拒绝切换并保持原值。 */
  UNAVAILABLE = "unavailable",
}

/** 开关的状态。 */
export interface ScreenCaptureProtectionState {
  /** 现在是否开着。 */
  enabled: boolean;
  /** 是否曾经打开过：打开时写入，之后关闭也不清除。 */
  enabledBefore: boolean;
}

interface ScreenCaptureProtectionDocument {
  enabled?: boolean;
  enabled_before?: boolean;
}

export class ScreenCaptureProtectionPolicy {
  /**
   * @param state 当前状态
   * @param next 用户要切换到的值
   * @param authenticationAvailable 系统认证此刻是否可用（`userAuth.getAvailableStatus` 对锁屏密码不报错）
   */
  static decide(
    state: ScreenCaptureProtectionState,
    next: boolean,
    authenticationAvailable: boolean,
  ): ScreenCaptureProtectionDecision {
    if (next === state.enabled) return ScreenCaptureProtectionDecision.UNCHANGED;
    if (!authenticationAvailable) return ScreenCaptureProtectionDecision.UNAVAILABLE;
    // 开着却没有「打开过」的标记只可能来自被改过的文件；按打开过处理，关闭仍要认证。
    return state.enabledBefore || state.enabled
      ? ScreenCaptureProtectionDecision.AUTHENTICATE
      : ScreenCaptureProtectionDecision.SAVE;
  }

  /** 保存 `next` 之后的状态：打开时同一次记下「打开过」，关闭时保留它。 */
  static after(state: ScreenCaptureProtectionState, next: boolean): ScreenCaptureProtectionState {
    return { enabled: next, enabledBefore: state.enabledBefore || state.enabled || next };
  }

  /**
   * 读文件内容，不信任它。没有文件、过大、格式不对时按从未打开过处理；开着却没有「打开过」标记时按打开过处理，这样改文件只能让保护更难关，不能绕过认证。
   */
  static parse(document: string | null): ScreenCaptureProtectionState {
    const off: ScreenCaptureProtectionState = { enabled: false, enabledBefore: false };
    if (
      document === null ||
      document.length === 0 ||
      document.length > SCREEN_CAPTURE_PROTECTION_MAX_BYTES
    ) {
      return off;
    }
    try {
      const raw: ScreenCaptureProtectionDocument = JSON.parse(
        document,
      ) as ScreenCaptureProtectionDocument;
      if (raw === null || typeof raw !== "object" || Array.isArray(raw)) return off;
      const enabled: boolean = raw.enabled === true;
      return { enabled: enabled, enabledBefore: enabled || raw.enabled_before === true };
    } catch (error) {
      return off;
    }
  }

  static serialize(state: ScreenCaptureProtectionState): string {
    const document: ScreenCaptureProtectionDocument = {
      enabled: state.enabled,
      enabled_before: state.enabledBefore || state.enabled,
    };
    return JSON.stringify(document);
  }
}
