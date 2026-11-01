import type { DictionaryClient } from "../dictionary/dictionary-contracts";
import { GroupList } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";
import { SummaryRow } from "./summary-row";
import { ActionRow } from "./action-row";
import { SettingsGroupNote } from "./settings-group-note";
import { useDictionaryUpdates } from "./use-dictionary-updates";

/** 开关只控制之后的自动检查，已下载的词库保留；手动检查由宿主启动后台任务。 */
export function DictionaryUpdates({
  request,
}: {
  request: NonNullable<DictionaryClient["updates"]>;
}) {
  const { status, running, message, action } = useDictionaryUpdates(request);
  return (
    <GroupList title="词库更新">
      <SwitchRow
        title="自动更新词库"
        checked={status?.enabled ?? true}
        disabled={!status}
        onChange={(enabled) => {
          void action("configure", enabled);
        }}
      />
      <SummaryRow title="词库版本">{status?.version ?? "读取中…"}</SummaryRow>
      <ActionRow
        title="检查词库更新"
        label={running ? "更新中…" : "检查更新"}
        disabled={running || !status}
        action={() => action("check")}
      />
      {status?.can_rollback && (
        <ActionRow
          title="恢复上一版词库"
          description="恢复后关闭自动更新，下次启动生效"
          label="恢复"
          disabled={running}
          action={() => action("rollback")}
        />
      )}
      <SettingsGroupNote inline>{message}</SettingsGroupNote>
    </GroupList>
  );
}
