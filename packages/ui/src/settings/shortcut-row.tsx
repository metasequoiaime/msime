import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import * as settings from "./settings-style";

export interface ShortcutRowProps {
  title: ReactNode;
  description?: ReactNode;
  chord: ReactNode;
}

/** A settings row that displays one keyboard shortcut at the trailing edge. */
export function ShortcutRow({ title, description, chord }: ShortcutRowProps) {
  return (
    <Row title={title} description={description}>
      <kbd className={settings.shortcutKey}>{chord}</kbd>
    </Row>
  );
}
