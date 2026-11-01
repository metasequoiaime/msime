import { SliderRow } from "./slider-row";

/** 本地识别「停顿时间」和「判停灵敏度」两行的文字（#10310），标题和说明与 iOS 原生语音页逐字相同；Android `VoicePage` 的标题和判停灵敏度说明也相同，停顿时间的说明多一句「锁定录音时由发送或取消结束」，因为只有 Android 能上滑锁定录音；说明段的第一句也与 iOS 相同，第二句讲鸿蒙上其他识别服务怎么结束：鸿蒙的系统识别由键盘按固定的 1500 毫秒停顿结束，iOS 的系统识别要点「说完了」，这一句不同。判停灵敏度按报告者给的语义：数值越高越容易判定说完、结束录音。 */
export const voicePauseText = {
  pauseTitle: "本地停顿时间",
  pauseDescription: "本地识别出字后停顿这么久就结束录音；按住说话时由松手结束",
  sensitivityTitle: "判停灵敏度",
  sensitivityDescription: "数值越高越容易判定说完、结束录音；调得太高，轻声说的字可能被当成停顿",
  footer:
    "这两项只用于点按开始的本地识别，只保存在本机，下次录音生效；判停灵敏度只对带语音活动检测的本地整段模型生效。系统识别仍在停顿 1.5 秒后结束，云服务由「说完了」或松开结束。",
} as const;

/** 与 client-core 的 `VOICE_PAUSE_MILLIS_*`、`VOICE_STOP_SENSITIVITY_*` 和 Android 本机设置相同的范围与步进；`fallback` 是缺键时的值，也就是改成可调之前写死的 1500 毫秒和 0.5 阈值对应的 50。 */
export const voicePauseLimits = {
  pause: { min: 500, max: 5000, step: 100, fallback: 1500 },
  sensitivity: { min: 10, max: 90, step: 1, fallback: 50 },
} as const;

export interface VoicePauseRowsProps {
  /** `voice_input.pause_millis`，缺键时传 `undefined`。 */
  pauseMillis: number | undefined;
  /** `voice_input.stop_sensitivity`，缺键时传 `undefined`。 */
  stopSensitivity: number | undefined;
  onPauseMillisChange: (pause_millis: number) => void;
  onStopSensitivityChange: (stop_sensitivity: number) => void;
}

/** 手机语音页「识别」组里的两条滑块：轨道铺满整行，数值在轨道右侧，与屏幕键盘页的尺寸滑块一样。 */
export function VoicePauseRows({
  pauseMillis,
  stopSensitivity,
  onPauseMillisChange,
  onStopSensitivityChange,
}: VoicePauseRowsProps) {
  const { pause, sensitivity } = voicePauseLimits;
  const pauseValue = pauseMillis ?? pause.fallback;
  const sensitivityValue = stopSensitivity ?? sensitivity.fallback;
  return (
    <>
      <SliderRow
        title={voicePauseText.pauseTitle}
        description={voicePauseText.pauseDescription}
        wide
        displayValue={`${pauseValue} 毫秒`}
        valueText={`${pauseValue} 毫秒`}
        min={pause.min}
        max={pause.max}
        step={pause.step}
        value={pauseValue}
        onChange={onPauseMillisChange}
      />
      <SliderRow
        title={voicePauseText.sensitivityTitle}
        description={voicePauseText.sensitivityDescription}
        wide
        displayValue={`${sensitivityValue}%`}
        valueText={`${sensitivityValue}%`}
        min={sensitivity.min}
        max={sensitivity.max}
        step={sensitivity.step}
        value={sensitivityValue}
        onChange={onStopSensitivityChange}
      />
    </>
  );
}
