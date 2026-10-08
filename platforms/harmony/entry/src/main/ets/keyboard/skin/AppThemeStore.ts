/**
 * 设备的应用主题选择（水杉四季 / 春芽 / 夏荫 / 秋杉 / 冬雪），以及存放它的文件。
 *
 * 与按键反馈一样，它不在共享偏好里：在 Android 上它同样是本地设置文件，因为它给本宿主自己的页面着色，而不是账号同步的内容。设置页写入、键盘读取，两者都使用 `KeyboardFeedback` 文件所在的状态目录，因此两个进程对 `system` 键盘所用的季节色调看法一致。
 *
 * 本文件只负责解析、校验和序列化；读写留给调用方的文件辅助函数，与 `KeyboardFeedback` 和 `KeyboardSession.feedbackIn` / `saveFeedbackIn` 的分工相同。id 是共享层的 `AppTheme` id（`crates/client-core/src/skin/app_theme.rs`），`msime_client_resolve_app_theme` 接受这些 id。
 */

interface AppThemeDocument {
  app_theme?: string;
}

const IDS: string[] = ["siji", "chunya", "xiayin", "qiushan", "dongxue"];

export class AppThemeStore {
  /** 状态目录里的文件名。 */
  static readonly FILE_NAME: string = "app-theme.json";
  /** 文档只有一个短字段；更大的内容在解析前就拒绝。 */
  static readonly MAX_BYTES: number = 4096;
  /** 水杉四季，跟随月份变化：共享层的默认值。 */
  static readonly DEFAULT_ID: string = "siji";

  /** `value` 是否恰好是五个应用主题 id 之一。 */
  static isId(value: string): boolean {
    return IDS.includes(value);
  }

  /**
   * 存储的 id，读取时不信任文档内容。值缺失、过大、格式错误或未知时取默认值而不是报错：不值得因为一个配色选择让键盘失败。
   */
  static parse(document: string | null): string {
    if (document === null || document.length === 0 || document.length > AppThemeStore.MAX_BYTES) {
      return AppThemeStore.DEFAULT_ID;
    }
    try {
      const raw: AppThemeDocument = JSON.parse(document) as AppThemeDocument;
      if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
        return AppThemeStore.DEFAULT_ID;
      }
      const id: string | undefined = raw.app_theme;
      return typeof id === "string" && AppThemeStore.isId(id) ? id : AppThemeStore.DEFAULT_ID;
    } catch (error) {
      return AppThemeStore.DEFAULT_ID;
    }
  }

  /** `id` 对应的文档。不在五个 id 之内的会写成默认值，这样文件里永远不会出现 `parse` 会拒绝的值。 */
  static serialize(id: string): string {
    const document: AppThemeDocument = {
      app_theme: AppThemeStore.isId(id) ? id : AppThemeStore.DEFAULT_ID,
    };
    return JSON.stringify(document);
  }
}
