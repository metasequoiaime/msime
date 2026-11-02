import type { ReactNode } from "react";
import { SettingsSubViewHeader } from "./settings-subview-header";

/** 从「我的插件」打开的子页标题栏；`data-plugin-back` 是重新进入时的焦点位置。 */
export function PluginViewHeader({
  title,
  note,
  onBack,
}: {
  title: string;
  /** 见 `SettingsSubViewHeader` 的 `note`。 */
  note?: ReactNode;
  onBack: () => void;
}) {
  return (
    <SettingsSubViewHeader
      title={title}
      note={note}
      back={{
        label: "我的插件",
        ariaLabel: "返回我的插件",
        onClick: onBack,
        marker: "data-plugin-back",
      }}
    />
  );
}
