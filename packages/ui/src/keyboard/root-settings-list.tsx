import { FluentIcon } from "../core/fluent-icons";
import { NavGroup, NavRow } from "../core/platform-controls";
import type { RootSettingsRow } from "./root-settings-model";
export { voiceLanguageTitle } from "../voice/voice-language-options";
export {
  rootPageValue,
  rootSettingsGroups,
  type RootPage,
  type RootSettingsRow,
} from "./root-settings-model";

/** 根页面的一行：带 Fluent 图标的 `NavRow`。 */
export function RootSettingsRowView({
  row,
  onOpenPage,
}: {
  row: RootSettingsRow;
  onOpenPage: (page: string) => void;
}) {
  return (
    <NavRow
      icon={<FluentIcon name={row.icon} size={20} />}
      title={row.title}
      value={row.value}
      onClick={() => onOpenPage(row.id)}
    />
  );
}

/** 「首页」根页面的分组行：每组一张无边框卡片，间距 20px，不显示组标题。 */
export function RootSettingsList({
  groups,
  onOpenPage,
}: {
  groups: readonly (readonly RootSettingsRow[])[];
  onOpenPage: (page: string) => void;
}) {
  return (
    <div className="flex flex-col gap-5">
      {groups.map((group) => (
        <NavGroup key={group[0].id}>
          {group.map((row) => (
            <RootSettingsRowView key={row.id} row={row} onOpenPage={onOpenPage} />
          ))}
        </NavGroup>
      ))}
    </div>
  );
}
