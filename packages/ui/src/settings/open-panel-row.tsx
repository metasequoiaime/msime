import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import { OpenPanelButton } from "./open-panel-button";

export interface OpenPanelRowProps {
  title: ReactNode;
  description?: ReactNode;
  action?: () => void | Promise<void>;
  label?: string;
  className?: string;
}

/** A settings row with the shared button for opening a host panel or native surface. */
export function OpenPanelRow({ title, description, action, label, className }: OpenPanelRowProps) {
  return (
    <Row title={title} description={description}>
      <OpenPanelButton action={action} label={label} className={className} />
    </Row>
  );
}
