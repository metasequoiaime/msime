import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import { EndpointInput } from "./endpoint-input";

export interface EndpointSettingRowProps {
  title: ReactNode;
  inputLabel: string;
  description?: ReactNode;
  value: string;
  disabled?: boolean;
  placeholder?: string;
  onChange: (value: string) => void;
}

/** A settings row with the shared URL input control. */
export function EndpointSettingRow({
  title,
  inputLabel,
  description,
  value,
  disabled,
  placeholder,
  onChange,
}: EndpointSettingRowProps) {
  return (
    <Row title={title} description={description} wideControl>
      <EndpointInput
        label={inputLabel}
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        onChange={onChange}
      />
    </Row>
  );
}
