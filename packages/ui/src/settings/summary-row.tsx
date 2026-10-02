import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import * as controls from "../core/platform-controls-style";

export interface SummaryRowProps {
  title: ReactNode;
  description?: ReactNode;
  hidden?: boolean;
  breakAnywhere?: boolean;
  children: ReactNode;
}

/** A settings row with a read-only, right-aligned summary value. */
export function SummaryRow({
  title,
  description,
  hidden,
  breakAnywhere = false,
  children,
}: SummaryRowProps) {
  return (
    <Row title={title} description={description} hidden={hidden}>
      <span
        className={`${controls.rowDescription} text-right${breakAnywhere ? " break-anywhere" : ""}`}
      >
        {children}
      </span>
    </Row>
  );
}
