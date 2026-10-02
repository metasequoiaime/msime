import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import {
  PreeditStyleSelect,
  type PreeditStyle,
  type PreeditStyleSelectMode,
} from "./preedit-style-select";

export interface PreeditStyleRowProps {
  title: ReactNode;
  description?: ReactNode;
  mode: PreeditStyleSelectMode;
  value: PreeditStyle;
  onChange: (value: PreeditStyle) => void;
}

/** A settings row for one of the preedit presentation selectors. */
export function PreeditStyleRow({
  title,
  description,
  mode,
  value,
  onChange,
}: PreeditStyleRowProps) {
  return (
    <Row title={title} description={description}>
      <PreeditStyleSelect mode={mode} value={value} onChange={onChange} />
    </Row>
  );
}
