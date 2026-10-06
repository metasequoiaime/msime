import type { InputHTMLAttributes, ReactNode } from "react";
import { Row } from "../core/platform-controls";

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
  ...inputProps
}: TextInputRowProps) {
  return (
    <Row title={title} description={description} hidden={hidden}>
      <input
        {...inputProps}
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {children}
    </Row>
  );
}
