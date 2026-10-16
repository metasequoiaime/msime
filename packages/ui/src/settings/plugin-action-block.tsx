import type { ReactNode } from "react";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsManagerNote } from "./settings-manager-note";

export interface PluginActionBlockProps {
  note?: ReactNode;
  children?: ReactNode;
}

/** 插件详情里的说明与操作区域；没有操作时不创建空的操作容器。 */
export function PluginActionBlock({ note, children }: PluginActionBlockProps) {
  return (
    <SettingsManagerBlock>
      {note && <SettingsManagerNote>{note}</SettingsManagerNote>}
      {children && <SettingsManagerActions>{children}</SettingsManagerActions>}
    </SettingsManagerBlock>
  );
}
