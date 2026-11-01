import { SecretInput } from "../core/secret-input";
import { Row } from "../core/platform-controls";
import type { ReactNode } from "react";

export interface SecretSettingRowProps {
  title: string;
  description?: ReactNode;
  /** 状态或校验提示，常驻标题下方；解释性说明用 `description`，收进「?」。 */
  detail?: ReactNode;
  label: string;
  value: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

/** A settings row that presents a masked credential input. */
export function SecretSettingRow({
  title,
  description,
  detail,
  label,
  value,
  disabled,
  onChange,
}: SecretSettingRowProps) {
  return (
    <Row title={title} description={description} detail={detail} wideControl>
      <SecretInput label={label} value={value} disabled={disabled} onChange={onChange} />
    </Row>
  );
}
