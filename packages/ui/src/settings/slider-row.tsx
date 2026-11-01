import type { ReactNode } from "react";
import { Row, Slider } from "../core/platform-controls";
import * as settings from "./settings-style";

export interface SliderRowProps {
  title: ReactNode;
  description?: ReactNode;
  /** 状态、当前值或校验提示，常驻标题下方；解释性说明用 `description`，收进「?」。 */
  detail?: ReactNode;
  hidden?: boolean;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  valueText?: string;
  /** Optional value rendered after the track, while the slider remains the accessible control. */
  displayValue?: ReactNode;
  ticks?: boolean;
  /** 轨道铺满整行：HarmonyOS 手机上放到标题下方，数值在轨道右侧；其余平台与默认相同。 */
  wide?: boolean;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  onChange: (value: number) => void;
}

/** A settings row that presents a platform slider with the standard row width. */
export function SliderRow({
  title,
  description,
  detail,
  hidden,
  value,
  min,
  max,
  step,
  disabled,
  valueText,
  displayValue,
  ticks,
  wide,
  onChange,
  ...labels
}: SliderRowProps) {
  const control = wide ? settings.sliderControlWide : settings.sliderControl;
  const slider = (
    <Slider
      value={value}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      valueText={valueText}
      ticks={ticks}
      onChange={onChange}
      {...labels}
    />
  );
  return (
    <Row title={title} description={description} detail={detail} hidden={hidden} wideControl={wide}>
      {displayValue === undefined ? (
        <span className={control}>{slider}</span>
      ) : (
        <span className={wide ? settings.sliderWithValueWide : settings.sliderWithValue}>
          <span className={control}>{slider}</span>
          <span className={settings.sliderValue} aria-hidden="true">
            {displayValue}
          </span>
        </span>
      )}
    </Row>
  );
}
