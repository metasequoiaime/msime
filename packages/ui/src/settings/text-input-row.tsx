import type { InputHTMLAttributes, ReactNode } from "react";
import { ControlledInput } from "../core/controlled-input";
import { Row } from "../core/platform-controls";
import { textInput } from "../core/platform-controls-style";

export interface TextInputRowProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "aria-label" | "onChange" | "title" | "value"
> {
  title: ReactNode;
  description?: ReactNode;
  /** 状态、当前值或校验提示，常驻标题下方；解释性说明用 `description`，收进「?」。 */
  detail?: ReactNode;
  hidden?: boolean;
  label: string;
  value: string;
  children?: ReactNode;
  onChange: (value: string) => void;
}

/** A settings row that presents a controlled, labelled text input. */
export function TextInputRow({
  title,
  description,
  detail,
  hidden,
  label,
  value,
  onChange,
  children,
  className = textInput,
  ...inputProps
}: TextInputRowProps) {
  return (
    <Row title={title} description={description} detail={detail} hidden={hidden} wideControl>
      <ControlledInput
        {...inputProps}
        className={className}
        aria-label={label}
        value={value}
        onChange={onChange}
      />
      {children}
    </Row>
  );
}
