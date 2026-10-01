import type { ReactNode, SelectHTMLAttributes } from "react";
import { Row, Select } from "../core/platform-controls";

export interface SelectRowProps extends Omit<
  SelectHTMLAttributes<HTMLSelectElement>,
  "title" | "children"
> {
  title: ReactNode;
  description?: ReactNode;
  hidden?: boolean;
  children?: ReactNode;
}

/** A settings row that presents a native select control. */
export function SelectRow({
  title,
  description,
  hidden,
  children,
  ...selectProps
}: SelectRowProps) {
  return (
    <Row title={title} description={description} hidden={hidden}>
      <Select {...selectProps}>{children}</Select>
    </Row>
  );
}
