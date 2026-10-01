export type HandwritingPlatform = "ios" | "harmony" | "android";

export interface HandwritingPlatformNoticeProps {
  platform: HandwritingPlatform;
  onOpenExternalUrl?: (url: string) => void;
}

/** Google ML Kit's terms and data disclosure; iOS and Android recognise handwriting with it. */
export const handwritingSdkPrivacyUrl = "https://developers.google.com/ml-kit/terms";

/** The one copy of each platform's handwriting privacy text: the legacy input panel shows it through `HandwritingPlatformNotice`, and the settings window's 手写输入 page places it inside the platform's group in `HandwritingSettingsSection`. */
export function handwritingPrivacyText(platform: HandwritingPlatform): string {
  switch (platform) {
    case "android":
      return "首次在 Android 键盘中切换到手写时，可能需要下载 Google ML Kit 中文手写模型。模型下载完成后可离线识别；笔迹和识别结果只用于当前输入，不会上传。Google ML Kit 可能发送性能及使用统计。";
    case "harmony":
      return "手写使用系统的文字识别能力，笔迹留在本机、不上传。设备未提供该能力时手写方案会明确提示，不会改用其他识别方式。";
    case "ios":
      return "首次在键盘中使用手写时下载中文模型，需要完全访问权限。下载后可离线识别，笔迹和识别结果不会上传。Google ML Kit 会发送性能及使用统计。";
  }
}

/** Platform-specific handwriting disclosure shown beside the input scheme settings. */
export function HandwritingPlatformNotice({
  platform,
  onOpenExternalUrl,
}: HandwritingPlatformNoticeProps) {
  const android = platform === "android";
  const harmony = platform === "harmony";
  return (
    <div className="section">
      <div className="section-title">{android ? "Android 手写输入" : "手写输入"}</div>
      <p>{handwritingPrivacyText(platform)}</p>
      {onOpenExternalUrl && !harmony && (
        <button
          type="button"
          className="secondary"
          onClick={() => onOpenExternalUrl(handwritingSdkPrivacyUrl)}
        >
          手写 SDK 隐私说明
        </button>
      )}
    </div>
  );
}
