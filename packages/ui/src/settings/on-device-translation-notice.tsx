import { ActionButton } from "./action-button";

export interface OnDeviceTranslationNoticeProps {
  languages: readonly string[];
  openSettings?: () => Promise<void>;
  onError?: (message: string) => void;
}

const openSettingsError = "无法打开系统设置，请手动前往 系统设置 > 通用 > 语言与地区 > 翻译语言。";

/** Explains missing macOS on-device translation downloads and opens their settings page. */
export function OnDeviceTranslationNotice({
  languages,
  openSettings,
  onError,
}: OnDeviceTranslationNoticeProps) {
  return (
    <div role="status" className="notice" aria-label="系统翻译语言未下载">
      <p>
        整句候选暂时没有翻译：macOS 还没有下载「中文（简体）→ {languages.join("、")}
        」翻译语言。离线词库只收词语，「现在几点了」这样的整句要靠系统在本机翻译，不联网。
      </p>
      <p>
        请在 系统设置 &gt; 通用 &gt; 语言与地区 &gt; 翻译语言
        中下载，然后回到输入框继续输入即可生效。也可以在下方选择一个在线翻译服务。
        {openSettings && (
          <ActionButton
            action={() => openSettings().catch(() => onError?.(openSettingsError))}
            label="打开语言与地区"
          />
        )}
      </p>
    </div>
  );
}
