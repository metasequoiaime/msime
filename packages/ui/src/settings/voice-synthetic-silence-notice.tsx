import * as settings from "./settings-style";

/** Explains the synthetic audio used when testing a voice recognition provider. */
export function VoiceSyntheticSilenceNotice() {
  return (
    <p className={settings.panelPreviewLabel}>
      测试会向当前服务发送一秒合成静音，不使用麦克风；服务可能计入 API 用量。
    </p>
  );
}
