import { SecretInput } from "../core/secret-input";
import { Row } from "../core/platform-controls";

export interface SecretSettingRowProps {
  title: string;
  label: string;
  value: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

/** A settings row that presents a masked credential input. */
export function SecretSettingRow({
  title,
  label,
  value,
  disabled,
  onChange,
}: SecretSettingRowProps) {
  return (
    <Row title={title}>
      <SecretInput label={label} value={value} disabled={disabled} onChange={onChange} />
    </Row>
  );
}
