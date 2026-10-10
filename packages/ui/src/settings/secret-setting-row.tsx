import { SecretInput } from "../core/secret-input";
import { Row } from "../core/platform-controls";
import type { ReactNode } from "react";

export interface SecretSettingRowProps {
  title: string;
  description?: ReactNode;
  label: string;
  value: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

/** A settings row that presents a masked credential input. */
export function SecretSettingRow({
  title,
  description,
  label,
  value,
  disabled,
  onChange,
}: SecretSettingRowProps) {
  return (
    <Row title={title} description={description} wideControl>
      <SecretInput label={label} value={value} disabled={disabled} onChange={onChange} />
    </Row>
  );
}
