import type { InputHTMLAttributes, ReactNode } from "react";
import { Row } from "../core/platform-controls";

export interface TextInputRowProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "aria-label" | "onChange" | "title" | "value"
> {
  title: ReactNode;
  label: string;
  value: string;
  onChange: (value: string) => void;
}

/** A settings row that presents a controlled, labelled text input. */
export function TextInputRow({ title, label, value, onChange, ...inputProps }: TextInputRowProps) {
  return (
    <Row title={title}>
      <input
        {...inputProps}
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </Row>
  );
}
