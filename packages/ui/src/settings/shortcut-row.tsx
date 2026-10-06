import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import { SettingsShortcutKey } from "./settings-shortcut-key";

export interface ShortcutRowProps {
  title: ReactNode;
  description?: ReactNode;
  chord: ReactNode;
}

/** A settings row that displays one keyboard shortcut at the trailing edge. */
export function ShortcutRow({ title, description, chord }: ShortcutRowProps) {
  return (
    <Row title={title} description={description}>
      <SettingsShortcutKey>{chord}</SettingsShortcutKey>
    </Row>
  );
}
