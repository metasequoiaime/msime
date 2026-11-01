import { Row } from "../core/platform-controls";
import { ActionButton } from "./action-button";
import { SettingsRowStack } from "./settings-row-stack";
import { SettingsWarning } from "./settings-warning";
import { SwitchRow } from "./switch-row";
import { useVimModeApps, type UseVimModeAppsOptions } from "./use-vim-mode-apps";

export interface CrossCommitSpacingSectionProps {
  enabled?: boolean;
  excludedApps?: string[];
  onChange: (enabled: boolean) => void;
  onExcludedAppsChange: (apps: string[]) => void;
  pickApplication?: UseVimModeAppsOptions["pickApplication"];
  applicationNames?: UseVimModeAppsOptions["applicationNames"];
}

/** macOS 的跨次空格开关与应用排除名单，独立于单次文本内部的空格开关。 */
export function CrossCommitSpacingSection(props: CrossCommitSpacingSectionProps) {
  const apps = useVimModeApps({
    value: props.excludedApps,
    windows: false,
    listName: "跨次空格排除名单",
    pickApplication: props.pickApplication,
    applicationNames: props.applicationNames,
    onChange: props.onExcludedAppsChange,
  });
  return (
    <SettingsRowStack role="group" aria-label="跨次中英文空格">
      <SwitchRow
        title="跨次上屏自动加空格"
        description="分次输入汉字与英文字母、数字时，在相邻边界补一个半角空格，也支持 Shift 切换英文和临时英文。已有空白不重复添加；应用不提供文档或查询被新输入打断时不添加；不改右侧已有文字。补空格可能增加一次撤销步骤。"
        checked={props.enabled ?? false}
        onChange={props.onChange}
      />
      {props.enabled && (
        <>
          <Row
            title="排除应用"
            description="在编写代码、网址或邮箱的应用中保留原样，可将该应用加入排除名单。"
          />
          {apps.entries.map((id) => (
            <Row key={id} title={apps.names[id] ?? id} detail={apps.names[id] ? id : undefined}>
              <ActionButton
                action={() => apps.remove(id)}
                label="移除"
                ariaLabel={`移除跨次空格排除应用 ${id}`}
              />
            </Row>
          ))}
          <Row title="添加排除应用">
            {apps.pick ? (
              <ActionButton
                action={apps.pick}
                disabled={apps.full || apps.picking}
                label="选择应用…"
              />
            ) : (
              <>
                <input
                  aria-label="跨次空格排除应用 Bundle ID"
                  value={apps.draft}
                  onChange={(event) => apps.edit(event.target.value)}
                  spellCheck={false}
                  autoCapitalize="off"
                />
                <ActionButton action={apps.addTyped} disabled={apps.full} label="添加" />
              </>
            )}
          </Row>
          {apps.warning && <SettingsWarning role="alert">{apps.warning}</SettingsWarning>}
        </>
      )}
    </SettingsRowStack>
  );
}
