import type { ReactNode } from "react";
import { Row, Segmented } from "../core/platform-controls";

export interface SegmentedRowOption<T extends string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}

export interface SegmentedRowProps<T extends string> {
  title: ReactNode;
  description?: ReactNode;
  hidden?: boolean;
  options: readonly SegmentedRowOption<T>[];
  value: T;
  disabled?: boolean;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  onChange: (value: T) => void;
}

/** A settings row that presents a segmented radio control. */
export function SegmentedRow<T extends string>({
  title,
  description,
  hidden,
  options,
  value,
  disabled,
  onChange,
  ...labels
}: SegmentedRowProps<T>) {
  return (
    <Row title={title} description={description} hidden={hidden}>
      <Segmented
        options={options}
        value={value}
        disabled={disabled}
        onChange={onChange}
        {...labels}
      />
    </Row>
  );
}
