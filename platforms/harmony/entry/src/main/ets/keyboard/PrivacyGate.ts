/**
 * 隐私模式，以及键盘保存每条记录前首先要问的那个问题：现在可以保存吗？
 *
 * 移植自 Android 的 `ImePrivacyGate`。隐私模式开启时，或焦点编辑框是密码框时，什么都不保存。Android 还有第三种情况，即要求不做个性化学习的编辑框；HarmonyOS 的编辑框属性不携带这种信号（`EditorPolicy.excludesKeyStatistics`），所以这里这两种情况就是全部规则。在这两种情况之外，每类记录仍受各自的开关约束（统计开关、剪贴板历史、云剪贴板的账号状态），由调用方照旧检查。
 *
 * 本键盘保存的记录就是下面四类。Android 的另外四类在这里没有对应：本键盘不记录语音时长，不上传语音用于改进，也不写输入事件或诊断文件，只写不含文本的 hilog 行。
 *
 * 隐私模式还会在本次会话中关闭引擎的学习并把会话标为私密，这样 `host-api` 既不从输入内容中学习，也不统计候选位置（`MSIMEInputService.restartSessionForPrivacy` 和 `markPrivateSession`）。用户自己的「学习」开关保持存储的值：`sessionPreferences` 只改变交给当前会话的内容。
 */
export enum PrivacyRecord {
  /** 用于输入统计的已上屏文本（`record`）。 */
  TYPING = "typing",
  /** 用于按键热力图的按键次数（`record_keys`）。 */
  KEYS = "keys",
  /** 语音输入持续的时长，用于「统计」页的语音时长（`record_voice`）。 */
  VOICE_DURATION = "voice_duration",
  /** 存入剪贴板历史的系统剪贴板内容。 */
  CLIPBOARD_HISTORY = "clipboard_history",
  /** 从云剪贴板读取或发送到云剪贴板的任何内容。 */
  CLOUD_CLIPBOARD = "cloud_clipboard",
}

/** 会话所接收的已存储偏好文档：即 `loadPreferences` 返回、`updatePreferences` 接受的结构。 */
export interface PreferenceDocument {
  format_version: number;
  revision: number;
  preferences: Record<string, Object>;
}

export class PrivacyGate {
  static readonly DEFAULT: boolean = false;
  static readonly RECORDS: PrivacyRecord[] = [
    PrivacyRecord.TYPING,
    PrivacyRecord.KEYS,
    PrivacyRecord.VOICE_DURATION,
    PrivacyRecord.CLIPBOARD_HISTORY,
    PrivacyRecord.CLOUD_CLIPBOARD,
  ];
  /** 因隐私原因拒绝「保存」时剪贴板面板显示的文字，与 Android 的 toast 逐字一致。 */
  static readonly CLIPBOARD_REFUSED: string = "隐私模式或当前输入框下不保存剪贴板";

  /** 是否完全不保存任何记录：隐私模式开启，或编辑框是密码框。 */
  static suppressed(incognito: boolean, password: boolean): boolean {
    return incognito || password;
  }

  /** 某类记录是否可以保存；两种隐私情况下为 false，否则为 true（其自身的开关另行检查）。 */
  static allows(record: PrivacyRecord, incognito: boolean, password: boolean): boolean {
    return PrivacyGate.RECORDS.includes(record) && !PrivacyGate.suppressed(incognito, password);
  }

  /**
   * 交给当前会话的偏好文档：即已存储的文档，隐私模式开启时把 `learning` 关掉。磁盘上的文档保留用户自己的选择；只有会话拿到的副本会变，`snapshot` 本身保持不变。
   */
  static sessionPreferences(snapshot: PreferenceDocument, incognito: boolean): string {
    if (!incognito) {
      return JSON.stringify(snapshot);
    }
    const preferences: Record<string, Object> = {};
    for (const key of Object.keys(snapshot.preferences)) {
      preferences[key] = snapshot.preferences[key];
    }
    preferences["learning"] = false;
    const copy: PreferenceDocument = {
      format_version: snapshot.format_version,
      revision: snapshot.revision,
      preferences: preferences,
    };
    return JSON.stringify(copy);
  }
}
