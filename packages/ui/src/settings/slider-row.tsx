import type { ReactNode } from "react";
import { Row, Slider } from "../core/platform-controls";
import * as settings from "./settings-style";

export interface SliderRowProps {
  title: ReactNode;
  description?: ReactNode;
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
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  onChange: (value: number) => void;
}

/** A settings row that presents a platform slider with the standard row width. */
export function SliderRow({
  title,
  description,
  hidden,
  value,
  min,
  max,
  step,
  disabled,
  valueText,
  displayValue,
  ticks,
  onChange,
  ...labels
}: SliderRowProps) {
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
    <Row title={title} description={description} hidden={hidden}>
      {displayValue === undefined ? (
        <span className={settings.sliderControl}>{slider}</span>
      ) : (
        <span className={settings.sliderWithValue}>
          <span className={settings.sliderControl}>{slider}</span>
          <span className={settings.sliderValue} aria-hidden="true">
            {displayValue}
          </span>
        </span>
      )}
    </Row>
  );
}
