import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { SettingsGroupNote } from "./settings-group-note";
import { SummaryRow } from "./summary-row";
import type { PluginPackage } from "./plugin-types";

export interface PluginInfoGroupProps {
  pack: PluginPackage;
  kindLabel: string;
  children?: ReactNode;
}

/** Metadata shared by installed packs and script plugins; kind-specific rows go before the description. */
export function PluginInfoGroup({ pack, kindLabel, children }: PluginInfoGroupProps) {
  return (
    <GroupList title="信息">
      <SummaryRow title="类型" breakAnywhere>
        {kindLabel}
      </SummaryRow>
      <SummaryRow title="版本" breakAnywhere>
        {pack.version}
      </SummaryRow>
      {pack.author && (
        <SummaryRow title="作者" breakAnywhere>
          {pack.author}
        </SummaryRow>
      )}
      <SummaryRow title="许可证" breakAnywhere>
        {pack.license}
      </SummaryRow>
      {children}
      {pack.description && <SettingsGroupNote inline>{pack.description}</SettingsGroupNote>}
    </GroupList>
  );
}
