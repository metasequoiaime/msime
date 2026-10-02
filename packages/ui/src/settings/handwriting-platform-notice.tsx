export type HandwritingPlatform = "ios" | "harmony" | "android";

export interface HandwritingPlatformNoticeProps {
  platform: HandwritingPlatform;
  onOpenExternalUrl?: (url: string) => void;
}

/** Google ML Kit 的条款和数据披露；iOS 和 Android 用它识别手写。 */
export const handwritingSdkPrivacyUrl = "https://developers.google.com/ml-kit/terms";

/** 各平台手写隐私文案的唯一一份：旧的输入面板通过 `HandwritingPlatformNotice` 显示它，设置窗口的「手写输入」页则在 `HandwritingSettingsSection` 里把它放进该平台的组。 */
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
