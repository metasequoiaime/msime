import type { ComponentPropsWithRef } from "react";

export interface ControlledInputProps extends Omit<
  ComponentPropsWithRef<"input">,
  "value" | "onChange"
> {
  value: string;
  onChange: (value: string) => void;
}

/** Native input with a controlled string value. */
export function ControlledInput({ value, onChange, ...inputProps }: ControlledInputProps) {
  return <input {...inputProps} value={value} onChange={(event) => onChange(event.target.value)} />;
}
