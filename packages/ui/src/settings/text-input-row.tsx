import type { InputHTMLAttributes, ReactNode } from "react";
import { Row } from "../core/platform-controls";
import { textInput } from "../core/platform-controls-style";

export interface TextInputRowProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "aria-label" | "onChange" | "title" | "value"
> {
  title: ReactNode;
  description?: ReactNode;
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
  hidden,
  label,
  value,
  onChange,
  children,
  className = textInput,
  ...inputProps
}: TextInputRowProps) {
  return (
    <Row title={title} description={description} hidden={hidden} wideControl>
      <input
        {...inputProps}
        className={className}
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {children}
    </Row>
  );
}
