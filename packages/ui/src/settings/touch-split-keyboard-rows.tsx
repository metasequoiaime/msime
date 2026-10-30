import { SliderRow } from "./slider-row";
import { SwitchRow } from "./switch-row";

/** 分离式键盘四项的当前值，对应共享偏好 `touch_keyboard_split`、`touch_keyboard_split_portrait`、`touch_keyboard_split_threshold` 和 `touch_keyboard_split_gap`。 */
export interface TouchSplitKeyboardSettings {
  enabled: boolean;
  portrait: boolean;
  /** 启用宽度（dp）。 */
  threshold: number;
  /** 中间空隙百分比。 */
  gap: number;
}

export type TouchSplitKeyboardChange = Partial<TouchSplitKeyboardSettings>;

/** 标题、说明和取值范围与 Android `KeyboardOptionsPage` 的「分离式键盘」几行逐字相同，iOS 原生设置页的开关标题也是「分离式键盘」。 */
export const touchSplitKeyboardText = {
  title: "分离式键盘",
  description:
    "默认仅在大屏横屏时生效：26 键和韩文键盘分成左右两半，方便双手握持时用拇指输入；右半边也有 G 和 V",
  portraitTitle: "竖屏也分离",
  portraitDescription: "平板或折叠屏内屏竖放时也分离；回到窄外屏后自动使用完整键盘",
  thresholdTitle: "分离启用宽度",
  gapTitle: "分离中间空隙",
} as const;

/** 与 Android `SplitKeyboardPolicy` 和 client-core 的校验相同的范围与步进。 */
export const touchSplitKeyboardLimits = {
  threshold: { min: 480, max: 1000, step: 20, fallback: 600 },
  gap: { min: 10, max: 40, step: 5, fallback: 25 },
} as const;

export interface TouchSplitKeyboardRowsProps {
  value: TouchSplitKeyboardSettings;
  onChange: (change: TouchSplitKeyboardChange) => void;
}

/** 屏幕键盘页「布局」组里的分离式键盘四行：主开关、竖屏也分离、启用宽度和中间空隙；主开关关着时后三行不可调，与 Android 相同。 */
export function TouchSplitKeyboardRows({ value, onChange }: TouchSplitKeyboardRowsProps) {
  const { threshold, gap } = touchSplitKeyboardLimits;
  return (
    <>
      <SwitchRow
        title={touchSplitKeyboardText.title}
        description={touchSplitKeyboardText.description}
        checked={value.enabled}
        onChange={(enabled) => onChange({ enabled })}
      />
      <SwitchRow
        title={touchSplitKeyboardText.portraitTitle}
        description={touchSplitKeyboardText.portraitDescription}
        disabled={!value.enabled}
        checked={value.portrait}
        onChange={(portrait) => onChange({ portrait })}
      />
      <SliderRow
        title={touchSplitKeyboardText.thresholdTitle}
        displayValue={`${value.threshold} dp`}
        valueText={`${value.threshold} dp`}
        min={threshold.min}
        max={threshold.max}
        step={threshold.step}
        disabled={!value.enabled}
        value={value.threshold}
        onChange={(next) => onChange({ threshold: next })}
      />
      <SliderRow
        title={touchSplitKeyboardText.gapTitle}
        displayValue={`${value.gap}%`}
        valueText={`${value.gap}%`}
        min={gap.min}
        max={gap.max}
        step={gap.step}
        disabled={!value.enabled}
        value={value.gap}
        onChange={(next) => onChange({ gap: next })}
      />
    </>
  );
}
