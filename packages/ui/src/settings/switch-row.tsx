import type { ReactNode } from "react";
import { Row, Switch } from "../core/platform-controls";

export interface SwitchRowProps {
  title: ReactNode;
  description?: ReactNode;
  /** 状态、当前值或校验提示，常驻标题下方；解释性说明用 `description`，收进「?」。 */
  detail?: ReactNode;
  hidden?: boolean;
  checked: boolean;
  disabled?: boolean;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  onChange: (checked: boolean) => void;
}

/** A settings row that presents a platform switch control. */
export function SwitchRow({
  title,
  description,
  detail,
  hidden,
  checked,
  disabled,
  onChange,
  ...labels
}: SwitchRowProps) {
  return (
    <Row title={title} description={description} detail={detail} hidden={hidden}>
      <Switch checked={checked} disabled={disabled} onChange={onChange} {...labels} />
    </Row>
  );
}
