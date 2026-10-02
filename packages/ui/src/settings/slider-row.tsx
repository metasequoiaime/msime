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
  ticks,
  onChange,
  ...labels
}: SliderRowProps) {
  return (
    <Row title={title} description={description} hidden={hidden}>
      <span className={settings.sliderControl}>
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
      </span>
    </Row>
  );
}
